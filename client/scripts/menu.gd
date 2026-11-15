extends Control
## Menu principale: partite, roster (equipaggiamento, classi), negozio, statistiche.

const UI := preload("res://scripts/ui.gd")
const Sprites := preload("res://scripts/sprites.gd")
const MAX_PARTY := 4

var main: Node
var me: Dictionary = {}
var inventory: Dictionary = {}
var _header: Label
var _tabs: TabContainer
var _play: VBoxContainer
var _roster: VBoxContainer
var _shop: VBoxContainer
var _stats: VBoxContainer
var _clan: VBoxContainer
var _season: VBoxContainer
var _status: Label
var _picked := {}
var _queued := false


func _ready() -> void:
	var root := UI.vbox(8)
	UI.full_rect(root)
	var m := MarginContainer.new()
	UI.full_rect(m)
	for side in ["left", "right", "top", "bottom"]:
		m.add_theme_constant_override("margin_" + side, 16)
	add_child(m)
	m.add_child(root)

	var head := UI.hbox(16)
	head.add_child(UI.label("Tactica", 26, UI.ACCENT))
	_header = UI.label("", 16)
	_header.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	head.add_child(_header)
	head.add_child(UI.button("Esci", _logout))
	root.add_child(head)

	_tabs = TabContainer.new()
	_tabs.size_flags_vertical = Control.SIZE_EXPAND_FILL
	root.add_child(_tabs)
	_play = _tab("Gioca")
	_roster = _tab("Roster")
	_shop = _tab("Negozio")
	_clan = _tab("Clan")
	_season = _tab("Stagione")
	_stats = _tab("Statistiche")
	_status = UI.label("", 14, UI.MUTED)
	root.add_child(_status)

	Net.ws_message.connect(_on_ws)
	await _load_catalogs()
	await reload()
	if main.autoplay() and not main.opts.get("autoplay_menu_done", false):
		main.opts["autoplay_menu_done"] = true
		_autoplay()


func _tab(title: String) -> VBoxContainer:
	var v := UI.vbox(8)
	var s := UI.scroll(v)
	s.name = title
	_tabs.add_child(s)
	return v


func _say(text: String, bad := false) -> void:
	_status.text = text
	_status.add_theme_color_override("font_color", UI.BAD if bad else UI.MUTED)


func _load_catalogs() -> void:
	if Net.items.is_empty():
		var r := await Net.get_json("/api/items")
		if int(r.status) == 200:
			for it in r.data:
				Net.items[str(it["key"])] = it
	if Net.classes.is_empty():
		var r := await Net.get_json("/api/classes")
		if int(r.status) == 200:
			for c in r.data:
				Net.classes[str(c["key"])] = c


func _ability_name(key: String) -> String:
	for c in Net.classes.values():
		for a in c["innate"] + c["learnable"]:
			if str(a["key"]) == key:
				return str(a["name"])
	return key


func _item_name(key: String) -> String:
	return str(Net.items.get(key, {}).get("name", key))


func reload() -> void:
	var r := await Net.get_json("/api/me")
	if int(r.status) != 200:
		_say(Net.error_text(r), true)
		return
	me = r.data
	var inv := await Net.get_json("/api/inventory")
	inventory = inv.data if int(inv.status) == 200 else {"coins": 0, "items": []}
	var tag := ""
	if me.get("clan") != null:
		tag = "[%s] " % me["clan"]["tag"]
	var title := ""
	if not (me.get("titles", []) as Array).is_empty():
		title = " · «%s»" % me["titles"][0]
	var srat := "—" if me["season"]["rating"] == null else str(int(me["season"]["rating"]))
	_header.text = "%s%s%s · ELO %d · %s: %s · Monete %d" % [tag, me["username"], title, int(me["elo"]),
		me["season"]["name"], srat, int(me["coins"])]
	_build_play()
	_build_roster()
	_build_shop()
	await _build_clan()
	await _build_season()
	await _build_stats()


func _logout() -> void:
	await Net.logout()
	main.show_login()


# ------------------------------------------------------------------ gioca

func _build_play() -> void:
	UI.clear(_play)
	_play.add_child(UI.label("Scegli fino a %d unità e una modalità." % MAX_PARTY, 16))
	var roster: Array = me["roster"]
	if roster.is_empty():
		_play.add_child(UI.label("Il roster è vuoto: recluta qualcuno nella scheda Roster.", 0, UI.ACCENT))
	for u in roster:
		var id := int(u["id"])
		var cb := CheckBox.new()
		cb.text = "%s — %s Lv %d" % [u["name"], u["class_name"], int(u["level"])]
		cb.button_pressed = _picked.has(id)
		cb.toggled.connect(func(on: bool) -> void:
			if on and _picked.size() >= MAX_PARTY:
				cb.set_pressed_no_signal(false)
				_say("Al massimo %d unità" % MAX_PARTY, true)
				return
			if on:
				_picked[id] = true
			else:
				_picked.erase(id))
		_play.add_child(cb)
	var row := UI.hbox()
	row.add_child(UI.button("Contro l'IA (PvE)", func() -> void: _queue("pve")))
	row.add_child(UI.button("Classificata (PvP)", func() -> void: _queue("pvp")))
	row.add_child(UI.button("Annulla coda", _cancel))
	_play.add_child(row)
	_play.add_child(HSeparator.new())
	_play.add_child(UI.button("Schermaglia offline (squadre dimostrative)", func() -> void: main.start_local_battle()))


func _queue(mode: String) -> void:
	var ids: Array = []
	for u in me["roster"]:
		if _picked.has(int(u["id"])):
			ids.append(int(u["id"]))
	if ids.is_empty():
		_say("Seleziona almeno un'unità", true)
		return
	_queued = true
	Net.ws_send({"type": "queue", "mode": mode, "units": ids})
	_say("Ricerca partita (%s)..." % mode.to_upper())


func _cancel() -> void:
	_queued = false
	Net.ws_send({"type": "cancel"})
	_say("Uscito dalla coda")


func _on_ws(msg: Dictionary, raw: String) -> void:
	if not is_inside_tree():
		return
	match str(msg.get("type", "")):
		"queued":
			_say("In coda: in attesa di un avversario...")
		"matched":
			_queued = false
			main.start_online_battle(msg, raw)
		"error":
			_say(str(msg.get("message", "errore")), true)


# ------------------------------------------------------------------ roster

func _build_roster() -> void:
	UI.clear(_roster)
	var rec := UI.hbox()
	var name_edit := UI.line_edit("Nome della recluta")
	var base := []
	for c in Net.classes.values():
		if bool(c["base"]):
			base.append([c["name"], c["key"]])
	var cls := UI.option(base)
	rec.add_child(name_edit)
	rec.add_child(cls)
	rec.add_child(UI.button("Recluta", func() -> void:
		_recruit(name_edit.text.strip_edges(), str(cls.get_selected_metadata()))))
	_roster.add_child(rec)
	for u in me["roster"]:
		_roster.add_child(_unit_panel(u))


func _unit_panel(u: Dictionary) -> Control:
	var id := int(u["id"])
	var outer := UI.hbox(14)
	outer.add_child(Sprites.portrait(str(u["class"]), 0, str(u["name"])))
	var box := UI.vbox(4)
	box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	outer.add_child(box)
	box.add_child(UI.label("%s — %s  Lv %d  (EXP %d/100)" % [u["name"], u["class_name"], int(u["level"]), int(u["xp"])], 17, UI.ACCENT))
	var eq: Array[String] = []
	for k in u["equipment"]:
		eq.append(_item_name(str(k)))
	box.add_child(UI.label("Equipaggiamento: " + (", ".join(eq) if not eq.is_empty() else "nessuno")))
	var us: Array[String] = []
	for k in u["usable"]:
		us.append(_ability_name(str(k)))
	box.add_child(UI.label("Abilità usabili: " + ", ".join(us), 0, UI.MUTED))
	for a in u["abilities"]:
		var state := "★ padroneggiata" if bool(a["mastered"]) else ("in addestramento" if bool(a["training"]) else "")
		box.add_child(UI.label("   %s  %d/%d RP  %s" % [a["name"], int(a["rp"]), int(a["master_rp"]), state],
			0, UI.GOOD if bool(a["mastered"]) else Color.WHITE))

	# Equipaggiamento: un menu per slot con oggetti equipaggiati o in inventario adatti alla classe.
	var row := UI.hbox()
	var pickers: Array[OptionButton] = []
	for slot in ["arma", "armatura", "accessorio"]:
		var opts := [["(%s: nessuno)" % slot, ""]]
		var sel := 0
		var seen := {}
		var candidates: Array = []
		for k in u["equipment"]:
			candidates.append(str(k))
		for it in inventory.get("items", []):
			candidates.append(str(it["key"]))
		for k in candidates:
			var info: Dictionary = Net.items.get(k, {})
			if info.is_empty() or str(info["slot"]) != slot or seen.has(k):
				continue
			if not (str(u["class"]) in info["classes"]):
				continue
			seen[k] = true
			opts.append([info["name"], k])
			if k in u["equipment"]:
				sel = opts.size() - 1
		var ob := UI.option(opts, sel)
		pickers.append(ob)
		row.add_child(ob)
	row.add_child(UI.button("Equipaggia", func() -> void:
		var keys: Array = []
		for ob in pickers:
			var k := str(ob.get_selected_metadata())
			if k != "":
				keys.append(k)
		_equip(id, keys)))
	box.add_child(row)

	var row2 := UI.hbox()
	var cls_opts := []
	var cur := 0
	for k in u["unlocked_classes"]:
		cls_opts.append([Net.classes.get(k, {}).get("name", k), k])
		if k == u["class"]:
			cur = cls_opts.size() - 1
	var co := UI.option(cls_opts, cur)
	row2.add_child(co)
	row2.add_child(UI.button("Cambia classe", func() -> void: _change_class(id, str(co.get_selected_metadata()))))
	row2.add_child(UI.button("Congeda", func() -> void: _dismiss(id)))
	var locked: Array[String] = []
	for c in Net.classes.values():
		if not (c["key"] in u["unlocked_classes"]):
			locked.append("%s (%s)" % [c["name"], c["requirement"]])
	box.add_child(row2)
	if not locked.is_empty():
		var lk := UI.label("Da sbloccare: " + "; ".join(locked), 12, UI.MUTED)
		lk.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		box.add_child(lk)
	return UI.panel(outer, 10)


func _result(r: Dictionary, ok_text: String) -> bool:
	if int(r.status) == 200:
		_say(ok_text)
		await reload()
		return true
	_say(Net.error_text(r), true)
	return false


func _recruit(name: String, cls: String) -> void:
	await _result(await Net.post_json("/api/roster", {"name": name, "class": cls}), "Recluta arruolata")


func _equip(id: int, keys: Array) -> void:
	await _result(await Net.put_json("/api/roster/%d/equipment" % id, {"items": keys}), "Equipaggiamento aggiornato")


func _change_class(id: int, cls: String) -> void:
	await _result(await Net.put_json("/api/roster/%d/class" % id, {"class": cls}), "Classe cambiata")


func _dismiss(id: int) -> void:
	_picked.erase(id)
	await _result(await Net.delete_json("/api/roster/%d" % id), "Unità congedata")


# ------------------------------------------------------------------ negozio

func _bonus_text(b: Dictionary) -> String:
	var names := {"atk": "ATK", "def": "DEF", "mag": "MAG", "res": "RES", "max_hp": "HP",
		"max_mp": "MP", "spd": "VEL", "mov": "MOV", "evade": "SCHIV", "jump": "SALTO"}
	var parts: Array[String] = []
	for k in names:
		var v := int(b.get(k, 0))
		if v != 0:
			parts.append("%s %+d" % [names[k], v])
	return " ".join(parts)


func _build_shop() -> void:
	UI.clear(_shop)
	_shop.add_child(UI.label("Monete: %d" % int(inventory.get("coins", 0)), 18, UI.ACCENT))
	var grid := GridContainer.new()
	grid.columns = 5
	grid.add_theme_constant_override("h_separation", 14)
	for h in ["Oggetto", "Slot", "Effetti", "Prezzo", ""]:
		grid.add_child(UI.label(h, 0, UI.MUTED))
	for it in Net.items.values():
		if not bool(it["in_shop"]):
			continue
		var key := str(it["key"])
		var fx := _bonus_text(it["bonus"])
		if it["teaches"] != null:
			fx += "  · insegna " + _ability_name(str(it["teaches"]))
		grid.add_child(UI.label(str(it["name"])))
		grid.add_child(UI.label(str(it["slot"])))
		grid.add_child(UI.label(fx, 13))
		grid.add_child(UI.label(str(int(it["price"]))))
		grid.add_child(UI.button("Compra", func() -> void: _trade("buy", key)))
	_shop.add_child(grid)
	_shop.add_child(HSeparator.new())
	_shop.add_child(UI.label("Inventario (oggetti non equipaggiati)", 16))
	for it in inventory.get("items", []):
		var key := str(it["key"])
		var row := UI.hbox()
		row.add_child(UI.label("%s ×%d" % [it["name"], int(it["qty"])]))
		var price := int(Net.items.get(key, {}).get("price", 0)) / 2
		row.add_child(UI.button("Vendi (%d)" % price, func() -> void: _trade("sell", key)))
		_shop.add_child(row)


func _trade(kind: String, key: String) -> void:
	await _result(await Net.post_json("/api/shop/" + kind, {"item": key}),
		"Acquistato" if kind == "buy" else "Venduto")


# ------------------------------------------------------------------ statistiche

func _build_stats() -> void:
	UI.clear(_stats)
	var p := await Net.get_json("/api/players/" + str(me["username"]).uri_encode())
	var lb := await Net.get_json("/api/leaderboard?limit=10")
	var g := await Net.get_json("/api/stats")
	var t := RichTextLabel.new()
	t.bbcode_enabled = true
	t.fit_content = true
	var s := ""
	if int(p.status) == 200:
		var d: Dictionary = p.data
		var tot: Dictionary = d["totals"]
		s += "[b]Profilo[/b]\nVittorie %d · Sconfitte %d · Pareggi %d · ELO %d\n" % [int(d["wins"]), int(d["losses"]), int(d["draws"]), int(d["elo"])]
		s += "Danni inflitti %d · subiti %d · cure %d · KO %d · violazioni %d\n" % [
			int(tot["damage_dealt"]), int(tot["damage_taken"]), int(tot["healing_done"]), int(tot["kills"]), int(tot["violations"])]
		for c in d["classes"]:
			s += "   %s: schierata %d volte, KO %d, sopravvivenza %d%%\n" % [
				Net.classes.get(c["class"], {}).get("name", c["class"]), int(c["fielded"]), int(c["kills"]), int(float(c["survival_rate"]) * 100)]
		s += "\n"
	if int(lb.status) == 200:
		s += "[b]Classifica[/b]\n"
		var i := 1
		for r in lb.data:
			s += "%d. %s — ELO %d (%d V / %d S)\n" % [i, r["username"], int(r["elo"]), int(r["wins"]), int(r["losses"])]
			i += 1
		s += "\n"
	if int(g.status) == 200:
		var d: Dictionary = g.data
		s += "[b]Statistiche globali[/b]\nGiocatori %d · Partite %d (PvP %d) · Pareggi %d\n" % [int(d["accounts"]), int(d["matches"]), int(d["pvp_matches"]), int(d["draws"])]
		for c in d["classes"]:
			s += "   %s: %d schieramenti, vittorie %d%%, danni medi %d, cure medie %d\n" % [
				Net.classes.get(c["class"], {}).get("name", c["class"]), int(c["fielded"]), int(float(c["win_rate"]) * 100),
				int(c["avg_damage"]), int(c["avg_healing"])]
		for st in d["statutes"]:
			s += "   Statuto «%s»: %d partite, %.2f violazioni a partita\n" % [st["statute"], int(st["matches"]), float(st["violations_per_match"])]
	t.text = s
	_stats.add_child(t)


# ------------------------------------------------------------------ automatico

func _autoplay() -> void:
	if (me["roster"] as Array).is_empty():
		var i := 0
		for c in ["vanguard", "scout", "arcanist", "mender"]:
			await Net.post_json("/api/roster", {"name": "%s%d" % [me["username"], i], "class": c})
			i += 1
		await reload()
	# Fonda un clan dimostrativo, se possibile.
	if me.get("clan") == null and int(me["coins"]) >= 200:
		await Net.post_json("/api/clans", {"name": "Compagnia di " + str(me["username"]), "tag": str(me["username"]).substr(0, 4).to_upper(), "motto": "Pixel e onore"})
		await reload()
	# Compra e equipaggia un oggetto che insegna un'abilità, se possibile.
	for u in me["roster"]:
		if str(u["class"]) == "arcanist" and not ("frost_rod" in u["equipment"]) and int(inventory.get("coins", 0)) >= 350:
			await Net.post_json("/api/shop/buy", {"item": "frost_rod"})
			await Net.put_json("/api/roster/%d/equipment" % int(u["id"]), {"items": ["frost_rod", "leather"]})
			await reload()
			break
	for tab in [[1, "menu_roster"], [2, "menu_negozio"], [3, "menu_clan"], [4, "menu_stagione"], [5, "menu_statistiche"]]:
		_tabs.current_tab = tab[0]
		await get_tree().create_timer(0.3).timeout
		await main.shot(tab[1])
	_tabs.current_tab = 0
	var n := 0
	for u in me["roster"]:
		if n < MAX_PARTY:
			_picked[int(u["id"])] = true
			n += 1
	_build_play()
	await main.shot("menu_gioca")
	_queue(str(main.opts.get("mode", "pve")))


# ------------------------------------------------------------------ clan

func _build_clan() -> void:
	UI.clear(_clan)
	if me.get("clan") != null:
		await _build_my_clan(str(me["clan"]["tag"]), str(me["clan"]["role"]))
		return
	_clan.add_child(UI.label("Non fai parte di un clan. I membri guadagnano punti stagionali per il clan e il 10% di monete in più.", 15))
	for tag in me.get("invites", []):
		var row := UI.hbox()
		row.add_child(UI.label("Invito dal clan [%s]" % tag, 0, UI.ACCENT))
		row.add_child(UI.button("Unisciti", func() -> void: _clan_post("/api/clans/%s/join" % tag, {}, "Benvenuto nel clan")))
		_clan.add_child(row)
	var form := UI.hbox()
	var name_e := UI.line_edit("Nome del clan")
	var tag_e := UI.line_edit("Sigla (2-5)")
	tag_e.custom_minimum_size = Vector2(110, 0)
	var motto_e := UI.line_edit("Motto (facoltativo)")
	var open_cb := CheckBox.new()
	open_cb.text = "Aperto"
	open_cb.button_pressed = true
	for c in [name_e, tag_e, motto_e, open_cb]:
		form.add_child(c)
	form.add_child(UI.button("Fonda (200 monete)", func() -> void:
		_clan_post("/api/clans", {"name": name_e.text, "tag": tag_e.text, "motto": motto_e.text, "open": open_cb.button_pressed}, "Clan fondato")))
	_clan.add_child(form)
	_clan.add_child(HSeparator.new())
	_clan.add_child(UI.label("Clan esistenti (per punti stagionali)", 16))
	var r := await Net.get_json("/api/clans")
	if int(r.status) != 200:
		return
	for c in r.data:
		var tag := str(c["tag"])
		var row := UI.hbox()
		row.add_child(UI.label("[%s] %s — %d membri — %d punti%s" % [tag, c["name"], int(c["members"]), int(c["season_points"]),
			"" if bool(c["open"]) else " — su invito"]))
		if bool(c["open"]):
			row.add_child(UI.button("Unisciti", func() -> void: _clan_post("/api/clans/%s/join" % tag, {}, "Benvenuto nel clan")))
		_clan.add_child(row)


func _build_my_clan(tag: String, role: String) -> void:
	var r := await Net.get_json("/api/clans/" + tag)
	if int(r.status) != 200:
		_clan.add_child(UI.label(Net.error_text(r), 0, UI.BAD))
		return
	var c: Dictionary = r.data
	_clan.add_child(UI.label("[%s] %s" % [c["tag"], c["name"]], 22, UI.ACCENT))
	if str(c["motto"]) != "":
		_clan.add_child(UI.label("«%s»" % c["motto"], 0, UI.MUTED))
	_clan.add_child(UI.label("Stagione in corso: %d punti, %d vittorie · %s" % [int(c["season_points"]), int(c["season_wins"]),
		"aperto a tutti" if bool(c["open"]) else "su invito"]))
	for h in c["history"]:
		_clan.add_child(UI.label("   Stagione %d: %d punti, posizione %s" % [int(h["number"]), int(h["points"]),
			"—" if h["rank"] == null else str(int(h["rank"]))], 13, UI.MUTED))
	_clan.add_child(HSeparator.new())
	var role_names := {"leader": "Capo", "officer": "Ufficiale", "member": "Membro"}
	for m in c["members"]:
		var row := UI.hbox()
		var uname := str(m["username"])
		row.add_child(UI.label("%s — %s — ELO %d" % [uname, role_names.get(m["role"], m["role"]), int(m["elo"])]))
		if uname != str(me["username"]):
			if role == "leader":
				if str(m["role"]) == "member":
					row.add_child(UI.button("Nomina ufficiale", func() -> void: _clan_post("/api/clans/%s/role" % tag, {"username": uname, "role": "officer"}, "Ruolo aggiornato")))
				elif str(m["role"]) == "officer":
					row.add_child(UI.button("Rendi membro", func() -> void: _clan_post("/api/clans/%s/role" % tag, {"username": uname, "role": "member"}, "Ruolo aggiornato")))
				row.add_child(UI.button("Cedi il comando", func() -> void: _clan_post("/api/clans/%s/role" % tag, {"username": uname, "role": "leader"}, "Comando ceduto")))
			if role == "leader" or (role == "officer" and str(m["role"]) == "member"):
				row.add_child(UI.button("Espelli", func() -> void: _clan_post("/api/clans/%s/kick" % tag, {"username": uname}, "Membro espulso")))
		_clan.add_child(row)
	_clan.add_child(HSeparator.new())
	if role != "member":
		var inv := UI.hbox()
		var who := UI.line_edit("Nome del giocatore da invitare")
		inv.add_child(who)
		inv.add_child(UI.button("Invita", func() -> void: _clan_post("/api/clans/%s/invite" % tag, {"username": who.text.strip_edges()}, "Invito inviato")))
		_clan.add_child(inv)
	if role == "leader":
		var set_row := UI.hbox()
		var motto_e := UI.line_edit("Nuovo motto", str(c["motto"]))
		set_row.add_child(motto_e)
		set_row.add_child(UI.button("Salva motto", func() -> void: _clan_put(tag, {"motto": motto_e.text})))
		set_row.add_child(UI.button("Rendi su invito" if bool(c["open"]) else "Apri a tutti", func() -> void: _clan_put(tag, {"open": not bool(c["open"])})))
		_clan.add_child(set_row)
	_clan.add_child(UI.button("Lascia il clan", func() -> void: _clan_post("/api/clans/leave", {}, "Hai lasciato il clan")))


func _clan_post(path: String, body: Dictionary, ok: String) -> void:
	await _result(await Net.post_json(path, body), ok)


func _clan_put(tag: String, body: Dictionary) -> void:
	await _result(await Net.put_json("/api/clans/" + tag, body), "Clan aggiornato")


# ------------------------------------------------------------------ stagione

func _build_season() -> void:
	UI.clear(_season)
	var r := await Net.get_json("/api/seasons/current")
	if int(r.status) != 200:
		return
	var s: Dictionary = r.data
	var left := maxi(0, int(s["ends_at"]) - int(Time.get_unix_time_from_system()))
	_season.add_child(UI.label(str(s["name"]), 22, UI.ACCENT))
	_season.add_child(UI.label("Termina tra %d giorni e %d ore. A fine stagione: titoli e monete ai migliori (1° 1000, 2° 600, 3° 400, primi 10: 200, partecipanti: 50) e 300 ai membri del clan vincitore." % [left / 86400, (left % 86400) / 3600]))
	var mine: Dictionary = me["season"]
	if mine["rating"] != null:
		_season.add_child(UI.label("Il tuo punteggio: %d (%d V / %d S)" % [int(mine["rating"]), int(mine["wins"]), int(mine["losses"])], 16, UI.GOOD))
	else:
		_season.add_child(UI.label("Gioca una partita classificata (PvP) per entrare in classifica.", 0, UI.MUTED))
	var t := RichTextLabel.new()
	t.bbcode_enabled = true
	t.fit_content = true
	var txt := "[b]Giocatori[/b]\n"
	for p in s["players"]:
		var ctag := "" if p["clan"] == null else "[%s] " % p["clan"]
		txt += "%d. %s%s — %d (picco %d) · %d V / %d S\n" % [int(p["rank"]), ctag, p["username"], int(p["rating"]), int(p["peak"]), int(p["wins"]), int(p["losses"])]
	txt += "\n[b]Clan[/b]\n"
	for c in s["clans"]:
		txt += "%d. [%s] %s — %d punti\n" % [int(c["rank"]), c["tag"], c["name"], int(c["points"])]
	var hist := await Net.get_json("/api/seasons")
	if int(hist.status) == 200 and (hist.data as Array).size() > 1:
		txt += "\n[b]Stagioni passate[/b]\n"
		for old in hist.data:
			if bool(old["closed"]):
				txt += "%s\n" % old["name"]
	t.text = txt
	_season.add_child(t)
