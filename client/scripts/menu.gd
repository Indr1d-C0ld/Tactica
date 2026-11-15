extends Control
## Menu principale: partite, roster (equipaggiamento, classi), negozio, statistiche.

const UI := preload("res://scripts/ui.gd")
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
	_header.text = "%s · ELO %d · Monete %d" % [me["username"], int(me["elo"]), int(me["coins"])]
	_build_play()
	_build_roster()
	_build_shop()
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
	var box := UI.vbox(4)
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
		box.add_child(UI.label("Da sbloccare: " + "; ".join(locked), 12, UI.MUTED))
	return UI.panel(box, 10)


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
	# Compra e equipaggia un oggetto che insegna un'abilità, se possibile.
	for u in me["roster"]:
		if str(u["class"]) == "arcanist" and not ("frost_rod" in u["equipment"]) and int(inventory.get("coins", 0)) >= 350:
			await Net.post_json("/api/shop/buy", {"item": "frost_rod"})
			await Net.put_json("/api/roster/%d/equipment" % int(u["id"]), {"items": ["frost_rod", "leather"]})
			await reload()
			break
	for tab in [[1, "menu_roster"], [2, "menu_negozio"], [3, "menu_statistiche"]]:
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
