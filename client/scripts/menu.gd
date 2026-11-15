extends Control
## Menu principale: partite, roster (equipaggiamento, classi), negozio, statistiche.

const Items := preload("res://scripts/items.gd")
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
var _craft: VBoxContainer
var _stats: VBoxContainer
var _clan: VBoxContainer
var _season: VBoxContainer
var _tourn: VBoxContainer
var _open_tournament := -1
var _status: Label
var _picked := {}
var _room: Dictionary = {}
var _rooms: Array = []
var _coop: VBoxContainer
var _difficulty := 1
var _war_room: Dictionary = {}
var _wars_box: VBoxContainer
var _my_clan: Dictionary = {}
var _my_role := ""
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
	var audio := UI.button("Audio: sì" if Sfx.enabled else "Audio: no", func() -> void: pass)
	audio.pressed.connect(func() -> void:
		Sfx.set_enabled(not Sfx.enabled)
		audio.text = "Audio: sì" if Sfx.enabled else "Audio: no")
	head.add_child(audio)
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
	_tourn = _tab("Tornei")
	_stats = _tab("Statistiche")
	_craft = _tab("Bottega")
	_status = UI.label("", 14, UI.MUTED)
	root.add_child(_status)

	Net.ws_message.connect(_on_ws)
	Sfx.music("menu")
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
	return Items.name_of(key)


func reload() -> void:
	var r := await Net.get_json("/api/me")
	if int(r.status) != 200:
		_say(Net.error_text(r), true)
		return
	me = r.data
	var inv := await Net.get_json("/api/inventory")
	inventory = inv.data if int(inv.status) == 200 else {"coins": 0, "items": []}
	# Le schede dell'inventario includono gli oggetti generati.
	for it in inventory.get("items", []):
		Net.items[str(it["key"])] = it
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
	await _build_tournaments()
	await _build_stats()
	await _build_craft()


func _logout() -> void:
	await Net.logout()
	main.show_login()


# ------------------------------------------------------------------ gioca

func _build_play() -> void:
	UI.clear(_play)
	var world := UI.button("Campagna: le Terre di Velmara", func() -> void: main.show_world())
	world.custom_minimum_size = Vector2(0, 54)
	_play.add_child(world)
	_play.add_child(HSeparator.new())
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
		"room":
			_room = msg["room"] if msg.get("room") is Dictionary else {}
			_render_coop()
			if main.autoplay():
				_autoplay_room()
		"war_room":
			_war_room = msg["room"] if msg.get("room") is Dictionary else {}
			_render_war_room()
			if main.autoplay():
				_autoplay_war_room()
		"rooms":
			_rooms = msg.get("rooms", [])
			_render_coop()
			if main.autoplay():
				_autoplay_room()


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
		eq.append(Items.bb_name(str(k)))
	var eq_lbl := RichTextLabel.new()
	eq_lbl.bbcode_enabled = true
	eq_lbl.fit_content = true
	eq_lbl.text = "Equipaggiamento: " + (", ".join(eq) if not eq.is_empty() else "nessuno")
	eq_lbl.tooltip_text = "\n\n".join(Array(u["equipment"]).map(func(k: Variant) -> String:
		return Items.plain(Items.describe(str(k), _ability_name))))
	box.add_child(eq_lbl)
	var icons := UI.hbox(6)
	for k in u["equipment"]:
		var ib := Items.icon_box(str(k))
		ib.tooltip_text = Items.plain(Items.describe(str(k), _ability_name))
		icons.add_child(ib)
	box.add_child(icons)
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
	for slot in ["arma", "armatura", "copricapo", "accessorio"]:
		var opts := [["(%s: nessuno)" % slot, ""]]
		var sel := 0
		var seen := {}
		var candidates: Array = []
		for k in u["equipment"]:
			candidates.append(str(k))
		for it in inventory.get("items", []):
			candidates.append(str(it["key"]))
		for k in candidates:
			var info: Dictionary = Items.info(k)
			if info.is_empty() or str(info["slot"]) != slot or seen.has(k):
				continue
			if not (str(u["class"]) in info["classes"]):
				continue
			seen[k] = true
			var mark := {"common": "", "uncommon": "◆ ", "rare": "◆◆ ", "epic": "◆◆◆ ", "legendary": "★ "}
			opts.append([str(mark.get(str(info.get("rarity", "common")), "")) + str(info["name"]), k])
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

	# Borsa di battaglia: due consumabili scelti dall'inventario.
	var bag_row := UI.hbox()
	bag_row.add_child(UI.label("Borsa:", 0, UI.MUTED))
	var bag_pickers: Array[OptionButton] = []
	var current_bag: Array = u.get("bag", [])
	var stock := {}
	for it in inventory.get("items", []):
		if str(Items.info(str(it["key"])).get("slot_key", "")) == "bag":
			stock[str(it["key"])] = int(it["qty"])
	for k in current_bag:
		stock[str(k)] = int(stock.get(str(k), 0)) + 1
	var bag_size := 4 if str(u["class"]) == "alchemist" else 2
	for slot in bag_size:
		var opts := [["(vuoto)", ""]]
		var sel := 0
		for k in stock:
			opts.append([Items.name_of(k), k])
			if slot < current_bag.size() and str(current_bag[slot]) == k:
				sel = opts.size() - 1
		var ob := UI.option(opts, sel)
		bag_pickers.append(ob)
		bag_row.add_child(ob)
	bag_row.add_child(UI.button("Prepara borsa", func() -> void:
		var keys: Array = []
		for ob in bag_pickers:
			var k := str(ob.get_selected_metadata())
			if k != "":
				keys.append(k)
		_set_bag(id, keys)))
	box.add_child(bag_row)

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


func _set_bag(id: int, keys: Array) -> void:
	await _result(await Net.put_json("/api/roster/%d/bag" % id, {"items": keys}), "Borsa preparata")


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
	grid.columns = 6
	grid.add_theme_constant_override("h_separation", 14)
	for h in ["", "Oggetto", "Slot", "Effetti", "Prezzo", ""]:
		grid.add_child(UI.label(h, 0, UI.MUTED))
	for it in Net.items.values():
		if not bool(it["in_shop"]):
			continue
		var key := str(it["key"])
		var fx := _bonus_text(it["bonus"])
		if it["teaches"] != null:
			fx += "  · insegna " + _ability_name(str(it["teaches"]))
		var ic := TextureRect.new()
		ic.texture = Items.icon(key)
		ic.custom_minimum_size = Vector2(32, 32)
		ic.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		ic.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		grid.add_child(ic)
		grid.add_child(UI.label(str(it["name"])))
		grid.add_child(UI.label(str(it["slot"])))
		grid.add_child(UI.label(fx, 13))
		grid.add_child(UI.label(str(int(it["price"]))))
		grid.add_child(UI.button("Compra", func() -> void: _trade("buy", key)))
	_shop.add_child(grid)
	_shop.add_child(HSeparator.new())
	_shop.add_child(UI.label("Inventario (oggetti non equipaggiati)", 16))
	var sorted_inv: Array = inventory.get("items", []).duplicate()
	var order := {"legendary": 0, "epic": 1, "rare": 2, "uncommon": 3, "common": 4}
	sorted_inv.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
		return int(order.get(str(a.get("rarity", "common")), 5)) < int(order.get(str(b.get("rarity", "common")), 5)))
	for it in sorted_inv:
		var key := str(it["key"])
		var row := UI.hbox(12)
		row.add_child(Items.icon_box(key))
		var card := RichTextLabel.new()
		card.bbcode_enabled = true
		card.fit_content = true
		card.custom_minimum_size = Vector2(620, 0)
		card.text = Items.describe(key, _ability_name) + ("   ×%d" % int(it["qty"]) if int(it["qty"]) > 1 else "")
		row.add_child(card)
		var price := int(Items.info(key).get("price", 0)) / 2
		row.add_child(UI.button("Vendi (%d)" % price, func() -> void: _trade("sell", key)))
		_shop.add_child(row)
		_shop.add_child(HSeparator.new())


func _trade(kind: String, key: String) -> void:
	await _result(await Net.post_json("/api/shop/" + kind, {"item": key}),
		"Acquistato" if kind == "buy" else "Venduto")


# ------------------------------------------------------------------ bottega (mestieri)

func _build_craft() -> void:
	UI.clear(_craft)
	var r := await Net.get_json("/api/craft")
	if int(r.status) != 200:
		_craft.add_child(UI.label(Net.error_text(r), 0, UI.BAD))
		return
	var w: Dictionary = r.data
	_craft.add_child(UI.label("Bottega — monete: %d" % int(w["coins"]), 18, UI.ACCENT))
	var mats := UI.hbox(14)
	for m in w["materials"]:
		var box := UI.hbox(4)
		var ic := TextureRect.new()
		ic.texture = Items.icon("mat_" + str(m["key"]))
		ic.custom_minimum_size = Vector2(32, 32)
		ic.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
		ic.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		ic.tooltip_text = str(m["name"])
		box.add_child(ic)
		box.add_child(UI.label("%d" % int(m["qty"])))
		mats.add_child(box)
	_craft.add_child(mats)
	_craft.add_child(UI.label("Le erbe si ricevono a ogni partita; gli altri materiali smantellando gli oggetti.", 12, UI.MUTED))

	_craft.add_child(HSeparator.new())
	_craft.add_child(UI.label("Forgia: smantella, raffina (qualità +1) o incanta (rarità +1, un affisso in più)", 16))
	for it in inventory.get("items", []):
		var key := str(it["key"])
		var row := UI.hbox(10)
		row.add_child(Items.icon_box(key))
		var name := RichTextLabel.new()
		name.bbcode_enabled = true
		name.fit_content = true
		name.custom_minimum_size = Vector2(380, 0)
		name.text = Items.bb_name(key) + ("  ×%d" % int(it["qty"]) if int(it["qty"]) > 1 else "")
		row.add_child(name)
		row.add_child(UI.button("Smantella", func() -> void: _craft_op("salvage", key)))
		if str(Items.info(key).get("slot_key", "")) != "bag":
			row.add_child(UI.button("Raffina", func() -> void: _craft_op("refine", key)))
			row.add_child(UI.button("Incanta", func() -> void: _craft_op("enchant", key)))
		_craft.add_child(row)

	_craft.add_child(HSeparator.new())
	_craft.add_child(UI.label("Alchimia: ricette (qualità I-III)", 16))
	for rec in w["recipes"]:
		var code := str(rec["code"])
		var row := UI.hbox(10)
		var tiers: Array = rec["tiers"]
		var first: Dictionary = tiers[0][0]
		row.add_child(Items.icon_box(str(first["key"])))
		var col := UI.vbox(2)
		col.add_child(UI.label(str(first["name"]).trim_suffix(" I"), 0, Items.color(str(first["rarity"]))))
		var buttons := UI.hbox(6)
		for t in tiers.size():
			var cost: Dictionary = tiers[t][1]
			var parts: Array[String] = []
			for m in cost["materials"]:
				parts.append("%d %s" % [int(m["qty"]), str(m["name"]).to_lower()])
			var tier: int = t
			var b := UI.button(["I", "II", "III"][t], func() -> void: _brew(code, tier))
			b.tooltip_text = "%s, %d monete" % [", ".join(parts), int(cost["coins"])]
			buttons.add_child(b)
		col.add_child(buttons)
		var c0: Dictionary = tiers[0][1]
		var need: Array[String] = []
		for m in c0["materials"]:
			need.append("%d %s" % [int(m["qty"]), str(m["name"]).to_lower()])
		col.add_child(UI.label("Qualità I: %s, %d monete (II e III: doppio e triplo)" % [", ".join(need), int(c0["coins"])], 12, UI.MUTED))
		row.add_child(col)
		_craft.add_child(row)


func _craft_op(op: String, key: String) -> void:
	var r := await Net.post_json("/api/craft/" + op, {"item": key})
	if int(r.status) == 200:
		var made: Variant = r.data.get("item")
		_say("Creato: " + str(made["name"]) if made != null else "Smantellato")
		await reload()
	else:
		_say(Net.error_text(r), true)


func _brew(code: String, tier: int) -> void:
	var r := await Net.post_json("/api/craft/brew", {"recipe": code, "tier": tier})
	if int(r.status) == 200:
		_say("Creato: " + str(r.data["item"]["name"]))
		await reload()
	else:
		_say(Net.error_text(r), true)


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
	if me.get("clan") == null and int(me["coins"]) >= 200 and not (str(main.opts.get("mode", "")) in ["coop", "war"]):
		await Net.post_json("/api/clans", {"name": "Compagnia di " + str(me["username"]), "tag": str(me["username"]).substr(0, 4).to_upper(), "motto": "Pixel e onore"})
		await reload()
	# Compra e equipaggia un oggetto che insegna un'abilità, se possibile.
	for u in me["roster"]:
		if str(u["class"]) == "arcanist" and not ("frost_rod" in u["equipment"]) and int(inventory.get("coins", 0)) >= 350 \
				and not (str(main.opts.get("mode", "")) in ["coop", "war"]):
			await Net.post_json("/api/shop/buy", {"item": "frost_rod"})
			await Net.put_json("/api/roster/%d/equipment" % int(u["id"]), {"items": ["frost_rod", "leather"]})
			await reload()
			break
	# Borsa: una pozione e una bomba per la prima unità, se c'è da spendere.
	var first: Dictionary = me["roster"][0]
	if (first.get("bag", []) as Array).is_empty() and int(inventory.get("coins", 0)) >= 130:
		await Net.post_json("/api/shop/buy", {"item": "c.pozione.0"})
		await Net.post_json("/api/shop/buy", {"item": "c.bomba.0"})
		await Net.put_json("/api/roster/%d/bag" % int(first["id"]), {"items": ["c.pozione.0", "c.bomba.0"]})
		await reload()
	for tab in [[1, "menu_roster"], [2, "menu_negozio"], [3, "menu_clan"], [4, "menu_stagione"], [5, "menu_tornei"], [6, "menu_statistiche"], [7, "menu_bottega"]]:
		_tabs.current_tab = tab[0]
		await get_tree().create_timer(0.3).timeout
		await main.shot(tab[1])
	if str(main.opts.get("mode", "")) == "coop":
		await _autoplay_coop_setup()
		return
	if str(main.opts.get("mode", "")) == "war":
		await _autoplay_war_setup()
		return
	if str(main.opts.get("mode", "")) == "campaign":
		main.show_world()
		return
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


# ------------------------------------------------------------------ missioni cooperative

func _coop_section() -> void:
	_coop = UI.vbox(6)
	_clan.add_child(HSeparator.new())
	_clan.add_child(UI.label("Missioni cooperative", 18, UI.ACCENT))
	_clan.add_child(UI.label("Da 2 a 6 membri del clan contro l'IA: 6 posti divisi fra i giocatori (le unità scelte nella scheda Gioca). Obiettivi: sbaragliare i nemici o abbattere il campione.", 13, UI.MUTED))
	_clan.add_child(_coop)
	_render_coop()
	Net.ws_send({"type": "room_list"})


func _render_coop() -> void:
	if _coop == null or not is_instance_valid(_coop):
		return
	UI.clear(_coop)
	if not _room.is_empty():
		var host := str(_room["host"]) == str(me["username"])
		_coop.add_child(UI.label("Stanza #%d — difficoltà %s — capostanza %s" % [int(_room["id"]), _room["difficulty_name"], _room["host"]], 16))
		for m in _room["members"]:
			_coop.add_child(UI.label("   %s (%d unità)" % [m["username"], int(m["units"])]))
		var row := UI.hbox()
		if host:
			var b := UI.button("Avvia la missione", func() -> void: Net.ws_send({"type": "room_start"}))
			b.disabled = (_room["members"] as Array).size() < int(_room["min_players"])
			row.add_child(b)
		row.add_child(UI.button("Esci dalla stanza", func() -> void: Net.ws_send({"type": "room_leave"})))
		_coop.add_child(row)
		if host and (_room["members"] as Array).size() < int(_room["min_players"]):
			_coop.add_child(UI.label("In attesa di altri membri del clan...", 0, UI.MUTED))
		return
	var row := UI.hbox()
	var diff := UI.option([["Normale", 1], ["Ardua", 2], ["Leggendaria", 3]], _difficulty - 1)
	diff.item_selected.connect(func(i: int) -> void: _difficulty = i + 1)
	row.add_child(diff)
	row.add_child(UI.button("Apri una stanza", func() -> void:
		Net.ws_send({"type": "room_create", "difficulty": _difficulty, "units": _picked_ids()})))
	row.add_child(UI.button("Aggiorna", func() -> void: Net.ws_send({"type": "room_list"})))
	_coop.add_child(row)
	if _rooms.is_empty():
		_coop.add_child(UI.label("Nessuna stanza aperta nel clan.", 0, UI.MUTED))
	for r in _rooms:
		var rid := int(r["id"])
		var rr := UI.hbox()
		rr.add_child(UI.label("Stanza #%d di %s — %s — %d/%d giocatori" % [rid, r["host"], r["difficulty_name"], (r["members"] as Array).size(), int(r["max_players"])]))
		rr.add_child(UI.button("Entra", func() -> void: Net.ws_send({"type": "room_join", "room": rid, "units": _picked_ids()})))
		_coop.add_child(rr)


func _picked_ids() -> Array:
	var ids: Array = []
	for u in me["roster"]:
		if _picked.has(int(u["id"])):
			ids.append(int(u["id"]))
	if ids.is_empty():
		for u in me["roster"]:
			if ids.size() < MAX_PARTY:
				ids.append(int(u["id"]))
	return ids


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
	_my_clan = c
	_my_role = role
	_coop_section()
	await _war_section()


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


func _autoplay_coop_setup() -> void:
	var tag := str(main.opts.get("clan", "COOP"))
	if me.get("clan") == null:
		if str(main.opts.get("role", "host")) == "host":
			await Net.post_json("/api/clans", {"name": "Compagnia " + tag, "tag": tag, "motto": "Insieme"})
		else:
			for i in 40:
				var r := await Net.post_json("/api/clans/%s/join" % tag, {})
				if int(r.status) == 200:
					break
				await get_tree().create_timer(0.5).timeout
		await reload()
	_tabs.current_tab = 3
	if str(main.opts.get("role", "host")) == "host":
		Net.ws_send({"type": "room_create", "difficulty": 1, "units": _picked_ids()})
	else:
		Net.ws_send({"type": "room_list"})


var _autoplay_shot_room := false


func _autoplay_room() -> void:
	var host := str(main.opts.get("role", "host")) == "host"
	if host and not _room.is_empty() and (_room["members"] as Array).size() >= int(_room["min_players"]):
		if not _autoplay_shot_room:
			_autoplay_shot_room = true
			await get_tree().create_timer(0.4).timeout
			await main.shot("menu_stanza")
		Net.ws_send({"type": "room_start"})
	elif not host and _room.is_empty():
		if _rooms.is_empty():
			await get_tree().create_timer(0.5).timeout
			Net.ws_send({"type": "room_list"})
		else:
			Net.ws_send({"type": "room_join", "room": int(_rooms[0]["id"]), "units": _picked_ids()})


# ------------------------------------------------------------------ guerre fra clan

func _war_section() -> void:
	_clan.add_child(HSeparator.new())
	_clan.add_child(UI.label("Guerre fra clan", 18, UI.ACCENT))
	_clan.add_child(UI.label("ELO del clan %d · guerre vinte %d, perse %d, pari %d. Le guerre cambiano l'ELO dei clan, non quello personale; il vincitore prende 5 punti stagionali e ogni partecipante monete (200 se vince, 60 se perde)." % [
		int(_my_clan.get("rating", 1000)), int(_my_clan.get("war_wins", 0)), int(_my_clan.get("war_losses", 0)), int(_my_clan.get("war_draws", 0))], 13, UI.MUTED))
	var captain := _my_role != "member"
	if captain:
		var r := await Net.get_json("/api/clans")
		var targets := []
		if int(r.status) == 200:
			for c in r.data:
				if str(c["tag"]) != str(_my_clan["tag"]):
					targets.append(["[%s] %s — ELO %d" % [c["tag"], c["name"], int(c["rating"])], str(c["tag"])])
		if not targets.is_empty():
			var row := UI.hbox()
			var ob := UI.option(targets)
			row.add_child(ob)
			row.add_child(UI.button("Dichiara guerra", func() -> void:
				_clan_post("/api/wars", {"tag": str(ob.get_selected_metadata())}, "Guerra dichiarata")))
			_clan.add_child(row)
	_wars_box = UI.vbox(4)
	_clan.add_child(_wars_box)
	await _render_wars()


func _render_wars() -> void:
	if _wars_box == null or not is_instance_valid(_wars_box):
		return
	UI.clear(_wars_box)
	var r := await Net.get_json("/api/wars")
	if int(r.status) != 200:
		return
	var mine := str(_my_clan.get("tag", ""))
	var captain := _my_role != "member"
	var status_names := {"pending": "in attesa di risposta", "accepted": "accettata: schieratevi", "live": "in corso",
		"done": "conclusa", "declined": "rifiutata", "cancelled": "annullata"}
	for w in r.data:
		var id := int(w["id"])
		var st := str(w["status"])
		var defending := str(w["defender_tag"]) == mine
		var row := UI.hbox()
		var txt := "[%s] contro [%s] — %s" % [w["challenger_tag"], w["defender_tag"], status_names.get(st, st)]
		if st == "done":
			var mine_before: Variant = w["defender_before"] if defending else w["challenger_before"]
			var mine_after: Variant = w["defender_after"] if defending else w["challenger_after"]
			var who := "pareggio" if w["winner_tag"] == null else "vince [%s]" % w["winner_tag"]
			txt += " — %s — ELO clan %d → %d" % [who, int(mine_before), int(mine_after)]
		row.add_child(UI.label(txt))
		if st == "pending" and defending and captain:
			row.add_child(UI.button("Accetta", func() -> void: _clan_post("/api/wars/%d/accept" % id, {}, "Guerra accettata")))
			row.add_child(UI.button("Rifiuta", func() -> void: _clan_post("/api/wars/%d/decline" % id, {}, "Guerra rifiutata")))
		if st == "accepted":
			row.add_child(UI.button("Schierati", func() -> void: Net.ws_send({"type": "war_join", "war": id, "units": _picked_ids()})))
		if st in ["pending", "accepted"] and captain and not (st == "pending" and defending):
			row.add_child(UI.button("Annulla", func() -> void: _clan_post("/api/wars/%d/cancel" % id, {}, "Guerra annullata")))
		_wars_box.add_child(row)
	_render_war_room()


func _render_war_room() -> void:
	if _wars_box == null or not is_instance_valid(_wars_box):
		return
	var old := _wars_box.get_node_or_null("WarRoom")
	if old != null:
		_wars_box.remove_child(old)
		old.queue_free()
	if _war_room.is_empty():
		return
	var box := UI.vbox(4)
	box.name = "WarRoom"
	box.add_child(UI.label("Schieramento della guerra #%d" % int(_war_room["war"]), 16, UI.ACCENT))
	for side in _war_room["sides"]:
		var names: Array[String] = []
		for m in side["members"]:
			names.append("%s (%d)" % [m["username"], int(m["units"])])
		box.add_child(UI.label("[%s] %s — %s — %s" % [side["tag"], side["name"],
			", ".join(names) if not names.is_empty() else "nessuno", "PRONTI" if bool(side["ready"]) else "non pronti"],
			0, UI.GOOD if bool(side["ready"]) else Color.WHITE))
	var row := UI.hbox()
	if _my_role != "member":
		var my_side: Dictionary = {}
		for side in _war_room["sides"]:
			if str(side["tag"]) == str(_my_clan.get("tag", "")):
				my_side = side
		var ready := bool(my_side.get("ready", false))
		row.add_child(UI.button("Non pronti" if ready else "Pronti!", func() -> void: Net.ws_send({"type": "war_ready", "ready": not ready})))
	row.add_child(UI.button("Lascia lo schieramento", func() -> void: Net.ws_send({"type": "war_leave"})))
	box.add_child(row)
	_wars_box.add_child(box)


# Modalità automatica: --mode=war --role=host|guest --clan=SIGLA --enemy=SIGLA
func _autoplay_war_setup() -> void:
	var tag := str(main.opts.get("clan", "LUPI"))
	var enemy := str(main.opts.get("enemy", "CORV"))
	var host := str(main.opts.get("role", "host")) == "host"
	if me.get("clan") == null:
		await Net.post_json("/api/clans", {"name": "Clan " + tag, "tag": tag, "motto": "Alla carica"})
		await reload()
	var war_id := -1
	for i in 120:
		var r := await Net.get_json("/api/wars")
		var wars: Array = r.data if int(r.status) == 200 else []
		var open := wars.filter(func(w: Dictionary) -> bool: return str(w["status"]) in ["pending", "accepted"])
		if host and open.is_empty():
			await Net.post_json("/api/wars", {"tag": enemy})
		elif not host and not open.is_empty() and str(open[0]["status"]) == "pending":
			await Net.post_json("/api/wars/%d/accept" % int(open[0]["id"]), {})
		elif not open.is_empty() and str(open[0]["status"]) == "accepted":
			war_id = int(open[0]["id"])
			break
		await get_tree().create_timer(0.5).timeout
	if war_id < 0:
		main.finish(1, "guerra non avviata")
		return
	await reload()
	_tabs.current_tab = 3
	Net.ws_send({"type": "war_join", "war": war_id, "units": _picked_ids()})


var _autoplay_war_shot := false


func _autoplay_war_room() -> void:
	if _war_room.is_empty():
		return
	var both := true
	for side in _war_room["sides"]:
		if (side["members"] as Array).is_empty():
			both = false
	if not both:
		return
	for side in _war_room["sides"]:
		if str(side["tag"]) == str(main.opts.get("clan", "")) and bool(side["ready"]):
			return
	if not _autoplay_war_shot and str(main.opts.get("role", "host")) == "host":
		_autoplay_war_shot = true
		await get_tree().create_timer(0.5).timeout
		await main.shot("menu_guerra")
	Net.ws_send({"type": "war_ready", "ready": true})


# ------------------------------------------------------------------ tornei

func _build_tournaments() -> void:
	UI.clear(_tourn)
	_tourn.add_child(UI.label("Tornei fra clan", 20, UI.ACCENT))
	_tourn.add_child(UI.label("Eliminazione diretta: teste di serie per ELO di clan, ogni incontro è una guerra (in caso di pareggio passa il clan con ELO più alto). Campioni: titolo, 500 monete a ogni membro e 15 punti stagionali; finalisti: titolo e 200 monete.", 13, UI.MUTED))
	var clan: Variant = me.get("clan")
	var captain: bool = clan != null and str(clan["role"]) != "member"
	if captain:
		var row := UI.hbox()
		var name_e := UI.line_edit("Nome del torneo")
		var size := UI.option([["4 clan", 4], ["8 clan", 8], ["16 clan", 16]], 1)
		row.add_child(name_e)
		row.add_child(size)
		row.add_child(UI.button("Organizza", func() -> void:
			_tourn_post("/api/tournaments", {"name": name_e.text, "max_clans": int(size.get_selected_metadata())}, "Torneo creato")))
		_tourn.add_child(row)
	var r := await Net.get_json("/api/tournaments")
	if int(r.status) != 200:
		return
	if (r.data as Array).is_empty():
		_tourn.add_child(UI.label("Nessun torneo per ora.", 0, UI.MUTED))
	var names := {"signup": "iscrizioni aperte", "running": "in corso", "done": "concluso", "cancelled": "annullato"}
	for t in r.data:
		var id := int(t["id"])
		var row := UI.hbox()
		var txt := "«%s» — %s — %d/%d clan" % [t["name"], names.get(t["status"], t["status"]), int(t["clans"]), int(t["max_clans"])]
		if t["winner"] != null:
			txt += " — campioni [%s]" % t["winner"]
		row.add_child(UI.label(txt))
		row.add_child(UI.button("Tabellone", func() -> void:
			_open_tournament = id
			_build_tournaments()))
		if captain and str(t["status"]) == "signup":
			row.add_child(UI.button("Iscrivi il clan", func() -> void: _tourn_post("/api/tournaments/%d/join" % id, {}, "Clan iscritto")))
			row.add_child(UI.button("Avvia", func() -> void: _tourn_post("/api/tournaments/%d/start" % id, {}, "Torneo avviato")))
		_tourn.add_child(row)
	if main.autoplay() and _open_tournament < 0 and not (r.data as Array).is_empty():
		_open_tournament = int(r.data[0]["id"])
	if _open_tournament >= 0:
		await _render_bracket(_open_tournament)


func _render_bracket(id: int) -> void:
	var r := await Net.get_json("/api/tournaments/%d" % id)
	if int(r.status) != 200:
		return
	var t: Dictionary = r.data
	_tourn.add_child(HSeparator.new())
	_tourn.add_child(UI.label("Tabellone — «%s»" % t["name"], 18, UI.ACCENT))
	var seeds: Array[String] = []
	for e in t["entries"]:
		seeds.append("%s[%s] (ELO %d)" % ["" if e["seed"] == null else "%d. " % int(e["seed"]), e["tag"], int(e["rating"])])
	_tourn.add_child(UI.label("Iscritti: " + ", ".join(seeds), 13))
	var my_tag := "" if me.get("clan") == null else str(me["clan"]["tag"])
	var cols := UI.hbox(16)
	for round in range(1, int(t["rounds"]) + 1):
		var col := UI.vbox(10)
		var title := "Finale" if round == int(t["rounds"]) else ("Semifinali" if round == int(t["rounds"]) - 1 else "Turno %d" % round)
		col.add_child(UI.label(title, 16, UI.ACCENT))
		for m in t["matches"]:
			if int(m["round"]) != round:
				continue
			var box := UI.vbox(2)
			var a := "—" if m["a"] == null else "[%s]" % m["a"]
			var b := "turno libero" if bool(m["bye"]) else ("—" if m["b"] == null else "[%s]" % m["b"])
			for side in [[a, m["a"]], [b, m["b"]]]:
				var won: bool = m["winner"] != null and side[1] != null and str(side[1]) == str(m["winner"])
				box.add_child(UI.label(str(side[0]), 0, UI.GOOD if won else Color.WHITE))
			if m["war_status"] != null and str(m["war_status"]) == "accepted" and my_tag in [str(m["a"]), str(m["b"])]:
				var war_id := int(m["war_id"])
				box.add_child(UI.button("Schierati", func() -> void: Net.ws_send({"type": "war_join", "war": war_id, "units": _picked_ids()})))
			col.add_child(UI.panel(box, 6))
		cols.add_child(col)
	_tourn.add_child(cols)
	if t["winner"] != null:
		_tourn.add_child(UI.label("Campioni: [%s]" % t["winner"], 20, UI.GOOD))


func _tourn_post(path: String, body: Dictionary, ok: String) -> void:
	await _result(await Net.post_json(path, body), ok)
