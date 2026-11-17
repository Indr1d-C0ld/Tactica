extends VBoxContainer
## Scheda «Statistiche»: profilo con andamento ELO, storico delle partite con
## dettaglio unità per unità, classifica e statistiche del mondo.

const UI := preload("res://scripts/ui.gd")
const Chart := preload("res://scripts/chart.gd")

const MODES := {"pve": "Contro l'IA", "pvp": "Classificata", "coop": "Missione di clan",
	"war": "Guerra fra clan", "campaign": "Campagna"}
const RESULTS := {"win": "Vittoria", "loss": "Sconfitta", "draw": "Pareggio"}
const PEOPLES := {"human": "Umani", "sylvan": "Silvani", "stonekin": "Rocciosi", "winged": "Alati", "undine": "Ondini"}
const SECTIONS := ["Profilo", "Le mie partite", "Classifica", "Mondo"]

var _section := "Profilo"
var _body: VBoxContainer
var _mode_filter := ""
var _offset := 0


func _ready() -> void:
	add_theme_constant_override("separation", 10)
	var row := UI.hbox(8)
	for s in SECTIONS:
		row.add_child(UI.button(s, func() -> void: show_section(s)))
	add_child(row)
	_body = UI.vbox(10)
	add_child(_body)


func refresh() -> void:
	await show_section(_section)


func show_section(s: String) -> void:
	_section = s
	UI.clear(_body)
	_body.add_child(UI.label(s, 26, UI.ACCENT))
	match s:
		"Profilo":
			await _profile()
		"Le mie partite":
			_offset = 0
			await _history()
		"Classifica":
			await _leaderboard()
		"Mondo":
			await _world()


# ------------------------------------------------------------------ utilità

static func date(t: int, with_time := true) -> String:
	var bias := int(Time.get_time_zone_from_system().get("bias", 0)) * 60
	var d := Time.get_datetime_dict_from_unix_time(t + bias)
	var s := "%02d/%02d/%d" % [d.day, d.month, d.year]
	return s + (" %02d:%02d" % [d.hour, d.minute] if with_time else "")


static func class_name_of(key: String) -> String:
	return str(Net.classes.get(key, {}).get("name", key))


static func pct(x: float) -> String:
	return "%d%%" % int(round(x * 100.0))


## Tabella: intestazioni e righe (ogni cella testo o [testo, colore]).
static func table(headers: Array, rows: Array) -> GridContainer:
	var g := GridContainer.new()
	g.columns = headers.size()
	g.add_theme_constant_override("h_separation", 22)
	g.add_theme_constant_override("v_separation", 4)
	for h in headers:
		g.add_child(UI.label(str(h), 16, UI.ACCENT))
	for r in rows:
		for c in r:
			if c is Control:
				g.add_child(c)
			elif c is Array:
				g.add_child(UI.label(str(c[0]), 16, c[1]))
			else:
				g.add_child(UI.label(str(c), 16))
	return g


static func result_cell(r: String) -> Array:
	var col := UI.GOOD if r == "win" else (UI.BAD if r == "loss" else UI.MUTED)
	return [RESULTS.get(r, r), col]


func _error(r: Dictionary) -> void:
	_body.add_child(UI.label(Net.error_text(r), 16, UI.BAD))


# ------------------------------------------------------------------ profilo

func _profile() -> void:
	var r := await Net.get_json("/api/me/stats")
	if int(r.status) != 200:
		return _error(r)
	var d: Dictionary = r.data
	var played := int(d["played"])
	var streak := int(d["current_streak"])
	var streak_txt := "nessuna" if streak == 0 else ("%d vittorie" % streak if streak > 0 else "%d sconfitte" % -streak)
	_body.add_child(UI.label("Partite %d · Vittorie %d · Sconfitte %d · Pareggi %d · %% vittorie %s" % [
		played, int(d["wins"]), int(d["losses"]), int(d["draws"]),
		pct(float(d["wins"]) / maxf(1.0, played))], 16))
	_body.add_child(UI.label("Miglior ELO %d · Serie attuale: %s · Serie migliore: %d vittorie · Precisione %s · Monete guadagnate %d" % [
		int(d["best_elo"]), streak_txt, int(d["best_streak"]), pct(float(d["accuracy"])), int(d["coins_earned"])], 16, UI.MUTED))
	var elo: Array = []
	var when: Array = []
	for p in d["elo"]:
		elo.append(int(p["elo"]))
		when.append(date(int(p["at"]), false))
	var ch := Chart.new(elo, false, UI.ACCENT, "Andamento ELO")
	ch.labels = when
	_body.add_child(ch)

	_body.add_child(UI.label("Per modalità", 16, UI.ACCENT))
	var rows: Array = []
	for m in d["modes"]:
		rows.append([MODES.get(m["mode"], m["mode"]), int(m["played"]), int(m["wins"]), int(m["losses"]), int(m["draws"]),
			"%.1f" % float(m["avg_turns"]), int(m["coins"])])
	_body.add_child(table(["Modalità", "Partite", "V", "S", "P", "Turni medi", "Monete"], rows))

	_body.add_child(UI.label("Per ambiente", 16, UI.ACCENT))
	rows = []
	for b in d["biomes"]:
		rows.append([b["biome"], int(b["played"]), pct(float(b["wins"]) / maxf(1.0, float(b["played"])))])
	_body.add_child(table(["Ambiente", "Partite", "% vittorie"], rows))

	_body.add_child(UI.label("Le tue unità", 16, UI.ACCENT))
	rows = []
	for u in d["units"]:
		rows.append([u["name"], "%s %s" % [_people_one(str(u["people"])), class_name_of(str(u["class"]))],
			int(u["level"]), int(u["fielded"]), int(u["kills"]), int(u["damage_dealt"]), int(u["healing_done"]),
			pct(float(u["survival_rate"])), int(u["xp_gained"])])
	_body.add_child(table(["Nome", "Popolo e classe", "Lv", "Schierata", "KO", "Danni", "Cure", "Sopravv.", "EXP"], rows))


static func _people_one(key: String) -> String:
	return {"human": "Umano", "sylvan": "Silvano", "stonekin": "Roccioso", "winged": "Alato", "undine": "Ondino"}.get(key, key)


# ------------------------------------------------------------------ storico

func _history() -> void:
	var filters := [["Tutte", ""]]
	for k in MODES:
		filters.append([MODES[k], k])
	var sel := 0
	for i in filters.size():
		if filters[i][1] == _mode_filter:
			sel = i
	var opt := UI.option(filters, sel)
	opt.item_selected.connect(func(i: int) -> void:
		_mode_filter = str(opt.get_item_metadata(i))
		_offset = 0
		_reload_history())
	var head := UI.hbox(10)
	head.add_child(UI.label("Modalità:", 16))
	head.add_child(opt)
	_body.add_child(head)
	var list := UI.vbox(6)
	list.name = "List"
	_body.add_child(list)
	await _fill_history(list)


func _reload_history() -> void:
	var list := _body.get_node_or_null("List") as VBoxContainer
	if list != null:
		await _fill_history(list)


func _fill_history(list: VBoxContainer) -> void:
	UI.clear(list)
	var r := await Net.get_json("/api/me/matches?limit=15&offset=%d&mode=%s" % [_offset, _mode_filter])
	if int(r.status) != 200:
		list.add_child(UI.label(Net.error_text(r), 16, UI.BAD))
		return
	var total := int(r.data["total"])
	if total == 0:
		list.add_child(UI.label("Nessuna partita giocata.", 16, UI.MUTED))
		return
	var rows: Array = []
	for m in r.data["matches"]:
		var delta := int(m["elo_after"]) - int(m["elo_before"])
		var who := "IA"
		if not (m["opponents"] as Array).is_empty():
			who = ", ".join(m["opponents"])
		var mid := int(m["id"])
		rows.append([date(int(m["ended_at"])), MODES.get(m["mode"], m["mode"]), result_cell(str(m["result"])),
			m["biome"], int(m["turns"]), ["%+d" % delta, UI.GOOD if delta > 0 else (UI.BAD if delta < 0 else UI.MUTED)],
			int(m["coins"]), who, UI.button("Dettagli", func() -> void: _detail(mid))])
	list.add_child(table(["Data", "Modalità", "Esito", "Ambiente", "Turni", "ELO", "Monete", "Avversari", ""], rows))
	var nav := UI.hbox(10)
	var prev := UI.button("« Più recenti", func() -> void:
		_offset = maxi(0, _offset - 15)
		_reload_history())
	prev.disabled = _offset == 0
	var next := UI.button("Meno recenti »", func() -> void:
		_offset += 15
		_reload_history())
	next.disabled = _offset + 15 >= total
	nav.add_child(prev)
	nav.add_child(UI.label("%d-%d di %d" % [_offset + 1, mini(_offset + 15, total), total], 16, UI.MUTED))
	nav.add_child(next)
	list.add_child(nav)


func _detail(id: int) -> void:
	var r := await Net.get_json("/api/matches/%d/summary" % id)
	UI.clear(_body)
	var back := UI.hbox()
	back.add_child(UI.button("« Torna allo storico", func() -> void:
		await show_section("Le mie partite")))
	_body.add_child(back)
	if int(r.status) != 200:
		return _error(r)
	var d: Dictionary = r.data
	_body.add_child(UI.label("Partita %d · %s · %s" % [id, MODES.get(d["mode"], d["mode"]), d["biome"]], 26, UI.ACCENT))
	var winner := "pareggio" if d["winner_team"] == null else ("vince la squadra %s" % ("blu" if int(d["winner_team"]) == 0 else "rossa"))
	var mins := (int(d["ended_at"]) - int(d["started_at"])) / 60
	_body.add_child(UI.label("%s · %d turni · %d minuti · %s" % [date(int(d["ended_at"])), int(d["turns"]), mins, winner], 16))
	if not (d["statutes"] as Array).is_empty():
		_body.add_child(UI.label("Statuti: " + ", ".join(d["statutes"]), 16, UI.MUTED))
	var rows: Array = []
	for p in d["players"]:
		var delta := int(p["elo_after"]) - int(p["elo_before"])
		rows.append([p["username"], "blu" if int(p["team"]) == 0 else "rossa", result_cell(str(p["result"])), "%+d" % delta, int(p["coins"])])
	_body.add_child(table(["Giocatore", "Squadra", "Esito", "ELO", "Monete"], rows))
	rows = []
	for u in d["units"]:
		var team_col: Color = UI.TEAM_COLORS[clampi(int(u["team"]), 0, 1)]
		var nm := str(u["name"]) if u["name"] != null else class_name_of(str(u["class"]))
		var owner := str(u["owner"]) if u["owner"] != null else "IA"
		var lv := "%d" % int(u["level_start"])
		if int(u["level_end"]) > int(u["level_start"]):
			lv += "→%d" % int(u["level_end"])
		rows.append([[nm, team_col], owner, class_name_of(str(u["class"])), lv,
			["sì", UI.GOOD] if bool(u["survived"]) else ["KO", UI.BAD], int(u["damage_dealt"]), int(u["damage_taken"]),
			int(u["healing_done"]), int(u["kills"]), "%d/%d" % [int(u["actions"]) - int(u["misses"]), int(u["actions"])],
			int(u["violations"])])
	_body.add_child(table(["Unità", "Di", "Classe", "Lv", "In piedi", "Danni", "Subiti", "Cure", "KO", "Colpi", "Violaz."], rows))


# ------------------------------------------------------------------ classifica

func _leaderboard() -> void:
	var r := await Net.get_json("/api/leaderboard?limit=50")
	if int(r.status) != 200:
		return _error(r)
	var rows: Array = []
	var i := 1
	for p in r.data:
		var mine := str(p["username"]) == Net.username
		rows.append([str(i), [p["username"], UI.ACCENT if mine else Color(0.92, 0.92, 0.95)], int(p["elo"]), int(p["wins"]), int(p["losses"])])
		i += 1
	_body.add_child(table(["#", "Giocatore", "ELO", "Vittorie PvP", "Sconfitte PvP"], rows))


# ------------------------------------------------------------------ mondo

func _world() -> void:
	var g := await Net.get_json("/api/stats")
	var t := await Net.get_json("/api/stats/trends")
	if int(g.status) != 200:
		return _error(g)
	var d: Dictionary = g.data
	var line := "Giocatori %d · Partite %d (PvP %d) · Pareggi %d" % [int(d["accounts"]), int(d["matches"]), int(d["pvp_matches"]), int(d["draws"])]
	if int(t.status) == 200:
		line += " · Turni medi %.1f · Durata media %.0f min" % [float(t.data["avg_turns"]), float(t.data["avg_match_minutes"])]
	_body.add_child(UI.label(line, 16))
	if int(t.status) == 200:
		var per_day: Array = []
		var new_acc: Array = []
		var days: Array = []
		for day in t.data["days"]:
			per_day.append(int(day["matches"]))
			new_acc.append(int(day["new_accounts"]))
			days.append(date(int(day["day"]), false).substr(0, 5))
		var c1 := Chart.new(per_day, true, Color(0.45, 0.7, 1.0), "Partite al giorno (30 giorni)")
		c1.labels = days
		_body.add_child(c1)
		var c2 := Chart.new(new_acc, true, UI.GOOD, "Nuovi giocatori al giorno")
		c2.labels = days
		_body.add_child(c2)
		var rows: Array = []
		for m in t.data["modes"]:
			rows.append([MODES.get(m["key"], m["key"]), int(m["count"]), pct(float(m["win_rate"]))])
		_body.add_child(UI.label("Modalità", 16, UI.ACCENT))
		_body.add_child(table(["Modalità", "Partite", "% vittorie dei giocatori"], rows))
		rows = []
		for p in t.data["peoples"]:
			rows.append([PEOPLES.get(p["key"], p["key"]), int(p["count"]), pct(float(p["win_rate"]))])
		_body.add_child(UI.label("Popoli", 16, UI.ACCENT))
		_body.add_child(table(["Popolo", "Schieramenti", "% vittorie"], rows))
	var rows2: Array = []
	for c in d["classes"]:
		rows2.append([class_name_of(str(c["class"])), int(c["fielded"]), pct(float(c["win_rate"])), int(c["avg_damage"]),
			int(c["avg_healing"]), "%.2f" % float(c["avg_kills"])])
	_body.add_child(UI.label("Classi", 16, UI.ACCENT))
	_body.add_child(table(["Classe", "Schieramenti", "% vittorie", "Danni medi", "Cure medie", "KO medi"], rows2))
	rows2 = []
	for b in d["biomes"]:
		rows2.append([b["biome"], int(b["matches"]), "%.1f" % float(b["avg_turns"])])
	_body.add_child(UI.label("Ambienti", 16, UI.ACCENT))
	_body.add_child(table(["Ambiente", "Partite", "Turni medi"], rows2))
	rows2 = []
	for st in d["statutes"]:
		rows2.append([st["statute"], int(st["matches"]), "%.2f" % float(st["violations_per_match"])])
	_body.add_child(UI.label("Statuti", 16, UI.ACCENT))
	_body.add_child(table(["Statuto", "Partite", "Violazioni a partita"], rows2))
