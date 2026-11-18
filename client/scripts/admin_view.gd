extends VBoxContainer
## Scheda «Admin» (solo amministratori): panoramica del server, utenti
## (sospensioni, monete, password, ruolo, disconnessione), connessioni aperte,
## partite, impostazioni di gioco, stagioni, backup e registro delle azioni.

const UI := preload("res://scripts/ui.gd")
const Stats := preload("res://scripts/stats_view.gd")

const SECTIONS := ["Panoramica", "Utenti", "Connessioni", "Partite", "Tornei", "Impostazioni", "Registro"]
const FILTERS := [["Tutti", ""], ["Online", "online"], ["Sospesi", "banned"], ["Admin", "admins"]]

var _section := "Panoramica"
var _body: VBoxContainer
var _msg: Label
var _query := ""
var _filter := ""
var _offset := 0


func _ready() -> void:
	add_theme_constant_override("separation", 10)
	var row := UI.hbox(8)
	for s in SECTIONS:
		row.add_child(UI.button(s, func() -> void: show_section(s)))
	add_child(row)
	_msg = UI.label("", 16, UI.MUTED)
	_msg.visible = false
	add_child(_msg)
	_body = UI.vbox(10)
	add_child(_body)


func refresh() -> void:
	await show_section(_section)


func _say(r: Dictionary, ok: String) -> bool:
	var good := int(r.status) == 200
	_msg.text = ok if good else Net.error_text(r)
	_msg.visible = _msg.text != ""
	_msg.add_theme_color_override("font_color", UI.GOOD if good else UI.BAD)
	if not good:
		Sfx.play("ui_error")
	return good


func show_section(s: String) -> void:
	_section = s
	UI.clear(_body)
	_body.add_child(UI.label(s, 26, UI.ACCENT))
	match s:
		"Panoramica":
			await _overview()
		"Utenti":
			await _users()
		"Connessioni":
			await _online()
		"Partite":
			await _matches()
		"Tornei":
			await _tournaments()
		"Impostazioni":
			await _settings()
		"Registro":
			await _log()


# ------------------------------------------------------------------ panoramica

func _overview() -> void:
	var r := await Net.get_json("/api/admin/overview")
	if int(r.status) != 200:
		_say(r, "")
		return
	var d: Dictionary = r.data
	var rows := [
		["Account", int(d["accounts"]), "admin %d · sospesi %d" % [int(d["admins"]), int(d["banned"])]],
		["Nuovi", "%d oggi" % int(d["new_24h"]), "%d in 7 giorni" % int(d["new_7d"])],
		["Attivi", "%d oggi" % int(d["active_24h"]), "%d in 7 giorni" % int(d["active_7d"])],
		["Partite", int(d["matches"]), "%d oggi · %d in 7 giorni" % [int(d["matches_24h"]), int(d["matches_7d"])]],
		["Online ora", "%d giocatori" % int(d["online_players"]), "%d connessioni · %d in partita · %d in coda PvP" % [
			int(d["online_connections"]), int(d["in_match"]), int(d["pvp_queue"])]],
		["Economia", "%d monete" % int(d["coins_total"]), "%d oggetti negli inventari" % int(d["items_total"])],
		["Clan", int(d["clans"]), ""],
		["Stagione", d["season"], "finisce il " + Stats.date(int(d["season_ends_at"]))],
	]
	_body.add_child(Stats.table(["", "", ""], rows))
	var st: Dictionary = d["settings"]
	var flags: Array[String] = []
	if bool(st["maintenance"]):
		flags.append("MANUTENZIONE ATTIVA")
	if not bool(st["registrations_open"]):
		flags.append("registrazioni chiuse")
	if not flags.is_empty():
		_body.add_child(UI.label(" · ".join(flags), 16, UI.BAD))
	var acts := UI.hbox(10)
	acts.add_child(UI.button("Chiudi la stagione ora", func() -> void:
		_confirm("Chiudere subito la stagione corrente? Premi e titoli vengono assegnati e ne parte una nuova.", func() -> void:
			var rr := await Net.post_json("/api/admin/seasons/close")
			if _say(rr, "Stagione chiusa: %s" % rr.data.get("closed", "")):
				await show_section("Panoramica"))))
	var b := UI.button("Backup del database", func() -> void:
		var rr := await Net.post_json("/api/admin/backup")
		_say(rr, "Backup salvato: %s" % (rr.data.get("path", "") if rr.data is Dictionary else "")))
	b.disabled = not bool(d["backups"])
	b.tooltip_text = "" if bool(d["backups"]) else "Imposta TACTICA_BACKUP_DIR sul server"
	acts.add_child(b)
	_body.add_child(acts)


## Conferma prima delle azioni pesanti.
func _confirm(text: String, action: Callable) -> void:
	var dlg := ConfirmationDialog.new()
	dlg.dialog_text = text
	dlg.ok_button_text = "Conferma"
	dlg.cancel_button_text = "Annulla"
	dlg.confirmed.connect(func() -> void: action.call())
	dlg.visibility_changed.connect(func() -> void:
		if not dlg.visible:
			dlg.queue_free())
	add_child(dlg)
	dlg.popup_centered()


# ------------------------------------------------------------------ utenti

func _users() -> void:
	var head := UI.hbox(10)
	var q := UI.line_edit("Cerca per nome", _query)
	var opt := UI.option(FILTERS, FILTERS.map(func(f: Array) -> String: return f[1]).find(_filter))
	var go := func() -> void:
		_query = q.text.strip_edges()
		_filter = str(opt.get_item_metadata(opt.selected))
		_offset = 0
		await show_section("Utenti")
	q.text_submitted.connect(func(_t: String) -> void: go.call())
	head.add_child(q)
	head.add_child(opt)
	head.add_child(UI.button("Cerca", go))
	_body.add_child(head)
	var r := await Net.get_json("/api/admin/users?q=%s&filter=%s&offset=%d&limit=25" % [_query.uri_encode(), _filter, _offset])
	if int(r.status) != 200:
		_say(r, "")
		return
	var total := int(r.data["total"])
	var rows: Array = []
	for u in r.data["users"]:
		var uid := int(u["id"])
		var state := "online" if bool(u["online"]) else ""
		var col := UI.GOOD
		if u["banned_until"] != null:
			state = "sospeso"
			col = UI.BAD
		var nm := str(u["username"]) + (" ★" if bool(u["is_admin"]) else "")
		var seen := "mai" if u["last_seen"] == null else Stats.date(int(u["last_seen"]))
		rows.append([nm, [state, col], int(u["elo"]), int(u["coins"]), int(u["matches"]), int(u["roster"]),
			str(u["clan"]) if u["clan"] != null else "", seen, UI.button("Gestisci", func() -> void: _user(uid))])
	_body.add_child(Stats.table(["Utente", "Stato", "ELO", "Monete", "Partite", "Unità", "Clan", "Ultimo accesso", ""], rows))
	var nav := UI.hbox(10)
	var prev := UI.button("«", func() -> void:
		_offset = maxi(0, _offset - 25)
		await show_section("Utenti"))
	prev.disabled = _offset == 0
	var next := UI.button("»", func() -> void:
		_offset += 25
		await show_section("Utenti"))
	next.disabled = _offset + 25 >= total
	nav.add_child(prev)
	nav.add_child(UI.label("%d utenti" % total, 16, UI.MUTED))
	nav.add_child(next)
	_body.add_child(nav)


func _user(id: int) -> void:
	var r := await Net.get_json("/api/admin/users/%d" % id)
	UI.clear(_body)
	var back := UI.hbox()
	back.add_child(UI.button("« Utenti", func() -> void: await show_section("Utenti")))
	_body.add_child(back)
	if int(r.status) != 200:
		_say(r, "")
		return
	var d: Dictionary = r.data
	var u: Dictionary = d["user"]
	_body.add_child(UI.label(str(u["username"]) + ("  (admin)" if bool(u["is_admin"]) else ""), 26, UI.ACCENT))
	var info := "ELO %d · Monete %d · Partite %d · Registrato il %s · Ultimo accesso %s · Sessioni attive %d%s" % [
		int(u["elo"]), int(u["coins"]), int(u["matches"]), Stats.date(int(u["created_at"])),
		"mai" if u["last_seen"] == null else Stats.date(int(u["last_seen"])), int(d["sessions"]),
		" · ONLINE" if bool(u["online"]) else ""]
	_body.add_child(UI.label(info, 16))
	if u["banned_until"] != null:
		var until := int(u["banned_until"])
		var span := "a tempo indeterminato" if until >= 253402300000 else "fino al " + Stats.date(until)
		_body.add_child(UI.label("Sospeso %s: %s" % [span, str(u["ban_reason"]) if u["ban_reason"] != null else ""], 16, UI.BAD))

	# Sospensione.
	var ban_row := UI.hbox(8)
	var days := UI.option([["1 giorno", 1], ["3 giorni", 3], ["7 giorni", 7], ["30 giorni", 30], ["Per sempre", 0]], 2)
	var reason := UI.line_edit("Motivo")
	ban_row.add_child(UI.label("Sospendi:", 16))
	ban_row.add_child(days)
	ban_row.add_child(reason)
	ban_row.add_child(UI.button("Sospendi", func() -> void:
		var dd := int(days.get_item_metadata(days.selected))
		var body := {"reason": reason.text}
		if dd > 0:
			body["days"] = dd
		_confirm("Sospendere %s? Verrà disconnesso subito." % u["username"], func() -> void:
			if _say(await Net.post_json("/api/admin/users/%d/ban" % id, body), "Utente sospeso"):
				await _user(id))))
	if u["banned_until"] != null:
		ban_row.add_child(UI.button("Riattiva", func() -> void:
			if _say(await Net.post_json("/api/admin/users/%d/unban" % id), "Utente riattivato"):
				await _user(id)))
	_body.add_child(ban_row)

	# Monete.
	var coin_row := UI.hbox(8)
	var amount := UI.line_edit("es. 500 o -200")
	amount.custom_minimum_size = Vector2(160, 0)
	var why := UI.line_edit("Motivo")
	coin_row.add_child(UI.label("Monete:", 16))
	coin_row.add_child(amount)
	coin_row.add_child(why)
	coin_row.add_child(UI.button("Applica", func() -> void:
		if not amount.text.strip_edges().is_valid_int():
			_msg.text = "Scrivi un numero intero"
			_msg.visible = true
			return
		var rr := await Net.post_json("/api/admin/users/%d/coins" % id, {"delta": int(amount.text), "reason": why.text})
		if _say(rr, "Monete aggiornate"):
			await _user(id)))
	_body.add_child(coin_row)

	# Password, disconnessione, ruolo.
	var acc_row := UI.hbox(8)
	var pw := UI.line_edit("Nuova password (min. 8)", "", true)
	acc_row.add_child(pw)
	acc_row.add_child(UI.button("Imposta password", func() -> void:
		_say(await Net.post_json("/api/admin/users/%d/password" % id, {"password": pw.text}), "Password cambiata, sessioni chiuse")))
	acc_row.add_child(UI.button("Disconnetti", func() -> void:
		var rr := await Net.post_json("/api/admin/users/%d/logout" % id)
		_say(rr, "Chiuse %d sessioni e %d connessioni" % [int(rr.data.get("sessions", 0)), int(rr.data.get("kicked", 0))] if rr.data is Dictionary else "")))
	var make := not bool(u["is_admin"])
	acc_row.add_child(UI.button("Rendi admin" if make else "Togli admin", func() -> void:
		_confirm(("Dare" if make else "Togliere") + " il ruolo di amministratore a %s?" % u["username"], func() -> void:
			if _say(await Net.post_json("/api/admin/users/%d/role" % id, {"admin": make}), "Ruolo aggiornato"):
				await _user(id))))
	_body.add_child(acc_row)

	_body.add_child(UI.label("Roster", 16, UI.ACCENT))
	var rows: Array = []
	for un in d["roster"]:
		rows.append([un["name"], "%s %s" % [Stats._people_one(str(un.get("people", "human"))), Stats.class_name_of(str(un["class"]))], int(un["level"])])
	_body.add_child(Stats.table(["Nome", "Popolo e classe", "Lv"], rows))
	_body.add_child(UI.label("Partite recenti", 16, UI.ACCENT))
	_body.add_child(_match_table(d["recent"]))
	if not (d["log"] as Array).is_empty():
		_body.add_child(UI.label("Azioni degli admin su questo utente", 16, UI.ACCENT))
		_body.add_child(_log_table(d["log"]))


# ------------------------------------------------------------------ connessioni

func _online() -> void:
	var r := await Net.get_json("/api/admin/online")
	if int(r.status) != 200:
		_say(r, "")
		return
	var rows: Array = []
	for o in r.data:
		var conn := int(o["conn"])
		var name := str(o["username"])
		rows.append([name, Stats.date(int(o["since"])), ["in partita", UI.ACCENT] if bool(o["in_match"]) else "nel menu",
			UI.button("Chiudi", func() -> void:
				if _say(await Net.post_json("/api/admin/online/%d/kick" % conn), "Connessione di %s chiusa" % name):
					await show_section("Connessioni"))])
	if rows.is_empty():
		_body.add_child(UI.label("Nessuna connessione aperta.", 16, UI.MUTED))
	else:
		_body.add_child(Stats.table(["Utente", "Connesso dal", "Dove", ""], rows))
	var again := UI.hbox()
	again.add_child(UI.button("Aggiorna", func() -> void: await show_section("Connessioni")))
	_body.add_child(again)


# ------------------------------------------------------------------ partite

func _match_table(list: Array) -> GridContainer:
	var rows: Array = []
	for m in list:
		var w := "pareggio" if m["winner_team"] == null else ("blu" if int(m["winner_team"]) == 0 else "rossa")
		rows.append([str(int(m["id"])), Stats.date(int(m["ended_at"])), Stats.MODES.get(m["mode"], m["mode"]), m["biome"],
			int(m["turns"]), w, ", ".join(m["players"])])
	return Stats.table(["#", "Fine", "Modalità", "Ambiente", "Turni", "Vince", "Giocatori (squadra, esito)"], rows)


func _matches() -> void:
	var r := await Net.get_json("/api/admin/matches?limit=50")
	if int(r.status) != 200:
		_say(r, "")
		return
	_body.add_child(_match_table(r.data))


# ------------------------------------------------------------------ impostazioni

func _settings() -> void:
	var r := await Net.get_json("/api/admin/settings")
	if int(r.status) != 200:
		_say(r, "")
		return
	var s: Dictionary = r.data
	var reg := CheckBox.new()
	reg.text = "Registrazioni aperte"
	reg.button_pressed = bool(s["registrations_open"])
	var maint := CheckBox.new()
	maint.text = "Manutenzione (solo gli admin avviano partite)"
	maint.button_pressed = bool(s["maintenance"])
	var ach := CheckBox.new()
	ach.text = "Monete in premio per i traguardi"
	ach.button_pressed = bool(s.get("achievement_coins", true))
	var motd := UI.line_edit("Messaggio per tutti i giocatori (vuoto = nessuno)", str(s["motd"]))
	motd.custom_minimum_size = Vector2(700, 0)
	var start := SpinBox.new()
	start.max_value = 1000000
	start.step = 50
	start.value = int(s["start_coins"])
	var coins := SpinBox.new()
	coins.max_value = 1000
	coins.step = 10
	coins.suffix = "%"
	coins.value = int(s["coins_pct"])
	var xp := SpinBox.new()
	xp.max_value = 1000
	xp.step = 10
	xp.suffix = "%"
	xp.value = int(s["xp_pct"])
	_body.add_child(reg)
	_body.add_child(maint)
	_body.add_child(ach)
	_body.add_child(UI.label("Messaggio del giorno", 16, UI.ACCENT))
	_body.add_child(motd)
	var grid := GridContainer.new()
	grid.columns = 2
	grid.add_theme_constant_override("h_separation", 16)
	for pair in [["Monete dei nuovi account", start], ["Monete di fine partita", coins], ["Esperienza di fine partita", xp]]:
		grid.add_child(UI.label(pair[0], 16))
		grid.add_child(pair[1])
	_body.add_child(grid)
	_body.add_child(UI.label("Le percentuali valgono dalla prossima partita conclusa (100% = normale; per eventi, ad esempio 200%).", 16, UI.MUTED))
	var save := UI.hbox()
	_body.add_child(save)
	save.add_child(UI.button("Salva impostazioni", func() -> void:
		var body := {"registrations_open": reg.button_pressed, "maintenance": maint.button_pressed, "motd": motd.text,
			"start_coins": int(start.value), "coins_pct": int(coins.value), "xp_pct": int(xp.value),
			"achievement_coins": ach.button_pressed}
		_say(await Net.put_json("/api/admin/settings", body), "Impostazioni salvate")))


# ------------------------------------------------------------------ registro

func _log_table(list: Array) -> GridContainer:
	var rows: Array = []
	for l in list:
		rows.append([Stats.date(int(l["at"])), l["admin"], l["action"], str(l["target"]) if l["target"] != null else "",
			str(l["detail"]) if l["detail"] != null else ""])
	return Stats.table(["Quando", "Admin", "Azione", "Su", "Dettagli"], rows)


func _log() -> void:
	var r := await Net.get_json("/api/admin/log")
	if int(r.status) != 200:
		_say(r, "")
		return
	if (r.data as Array).is_empty():
		_body.add_child(UI.label("Nessuna azione registrata.", 16, UI.MUTED))
		return
	_body.add_child(_log_table(r.data))


# ------------------------------------------------------------------ tornei

const T_STATUS := {"signup": "iscrizioni", "running": "in corso", "done": "concluso", "cancelled": "annullato"}


func _tournaments() -> void:
	var top := UI.hbox(10)
	top.add_child(UI.button("Applica le scadenze ora", func() -> void:
		var rr := await Net.post_json("/api/admin/tournaments/expire")
		if _say(rr, "Incontri assegnati a tavolino: %d" % int(rr.data.get("decided", 0)) if rr.data is Dictionary else ""):
			await show_section("Tornei")))
	_body.add_child(top)
	_body.add_child(UI.label("Alla scadenza un incontro non giocato va a tavolino: vince il clan che si è schierato; se non c'è nessuno, o ci sono entrambi, passa il clan con l'ELO più alto. Le guerre in corso non scadono.", 16, UI.MUTED))
	var r := await Net.get_json("/api/admin/tournaments")
	if int(r.status) != 200:
		_say(r, "")
		return
	if (r.data as Array).is_empty():
		_body.add_child(UI.label("Nessun torneo.", 16, UI.MUTED))
		return
	for t in r.data:
		var tid := int(t["id"])
		var box := UI.vbox(6)
		var head := "%s · %s · %d clan · incontri da %d ore" % [t["name"], T_STATUS.get(t["status"], t["status"]), int(t["clans"]), int(t["match_hours"])]
		if t["creator"] != null:
			head += " · organizza [%s]" % t["creator"]
		if t["winner"] != null:
			head += " · campioni [%s]" % t["winner"]
		box.add_child(UI.label(head, 16, UI.ACCENT))
		var open := str(t["status"]) in ["signup", "running"]
		if str(t["status"]) == "running":
			for m in t["pending"]:
				var row := UI.hbox(8)
				var a := "—" if m["a"] == null else str(m["a"])
				var b := "—" if m["b"] == null else str(m["b"])
				var when := "senza scadenza" if m["deadline"] == null else "scade il " + Stats.date(int(m["deadline"]))
				var war := "" if m["war_status"] == null else " (guerra: %s)" % m["war_status"]
				row.add_child(UI.label("Turno %d · [%s] contro [%s] · %s%s" % [int(m["round"]), a, b, when, war], 16))
				for tag in [m["a"], m["b"]]:
					if tag == null:
						continue
					var tg := str(tag)
					var rnd := int(m["round"])
					var slot := int(m["slot"])
					row.add_child(UI.button("Vince [%s]" % tg, func() -> void:
						_confirm("Assegnare l'incontro a [%s] senza giocarlo?" % tg, func() -> void:
							if _say(await Net.post_json("/api/admin/tournaments/%d/matches/%d/%d/winner" % [tid, rnd, slot], {"tag": tg}), "Incontro assegnato a [%s]" % tg):
								await show_section("Tornei"))))
				box.add_child(row)
			var ext := UI.hbox(8)
			ext.add_child(UI.button("Proroga di 24 ore", func() -> void:
				if _say(await Net.post_json("/api/admin/tournaments/%d/extend" % tid, {"hours": 24}), "Scadenze prorogate di 24 ore"):
					await show_section("Tornei")))
			box.add_child(ext)
		if open:
			var row2 := UI.hbox(8)
			var tname := str(t["name"])
			row2.add_child(UI.button("Annulla il torneo", func() -> void:
				_confirm("Annullare «%s»? Nessun premio verrà assegnato." % tname, func() -> void:
					if _say(await Net.post_json("/api/admin/tournaments/%d/cancel" % tid), "Torneo annullato"):
						await show_section("Tornei"))))
			box.add_child(row2)
		_body.add_child(UI.panel(box, 10))
