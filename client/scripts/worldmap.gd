extends Control
## Mappa del mondo della campagna: regioni, sentieri e tappe sulla pergamena.

const UI := preload("res://scripts/ui.gd")
const Art := preload("res://scripts/art.gd")
var MAP_TEX: Texture2D = Art.tex("worldmap")

var main: Node
var data: Dictionary = {}
var selected: Dictionary = {}
var _map: TextureRect
var _paths: Control
var _info: RichTextLabel
var _actions: HBoxContainer
var _status: Label


func _ready() -> void:
	Sfx.music("world")
	var bg := ColorRect.new()
	bg.color = Color(0.09, 0.1, 0.13)
	UI.full_rect(bg)
	add_child(bg)

	_map = TextureRect.new()
	_map.texture = MAP_TEX
	_map.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_map.stretch_mode = TextureRect.STRETCH_SCALE
	_map.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_map.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_map.offset_right = -380
	add_child(_map)
	_paths = Control.new()
	_paths.mouse_filter = Control.MOUSE_FILTER_IGNORE
	UI.full_rect(_paths)
	_paths.draw.connect(_draw_paths)
	_map.add_child(_paths)

	var side := UI.vbox(10)
	side.add_child(UI.label("Le Terre di Velmara", 24, UI.ACCENT))
	_info = RichTextLabel.new()
	_info.bbcode_enabled = true
	_info.fit_content = true
	_info.custom_minimum_size = Vector2(340, 0)
	side.add_child(_info)
	_actions = UI.hbox()
	side.add_child(_actions)
	_status = UI.label("", 0, UI.MUTED)
	side.add_child(_status)
	side.add_child(UI.button("Torna al menu", func() -> void: main.show_menu()))
	var sp := UI.panel(UI.scroll(side))
	sp.set_anchors_and_offsets_preset(Control.PRESET_RIGHT_WIDE)
	sp.offset_left = -380
	add_child(sp)

	Net.ws_message.connect(_on_ws)
	await reload()
	if main.autoplay() and not main.opts.get("autoplay_world_done", false):
		main.opts["autoplay_world_done"] = true
		_autoplay()


func reload() -> void:
	var r := await Net.get_json("/api/campaign")
	if int(r.status) != 200:
		_status.text = Net.error_text(r)
		return
	data = r.data
	_build_nodes()
	# Si seleziona la prima tappa sbloccata e non ancora vinta.
	for reg in data["regions"]:
		for n in reg["nodes"]:
			if bool(n["unlocked"]) and not bool(n["cleared"]) and selected.is_empty():
				_select(n, reg)
	if selected.is_empty():
		var last: Dictionary = data["regions"][-1]
		_select(last["nodes"][-1], last)


func _node_pos(n: Dictionary) -> Vector2:
	return Vector2(float(n["x"]), float(n["y"])) * _map.size


func _build_nodes() -> void:
	for c in _map.get_children():
		if c != _paths:
			c.queue_free()
	var regions: Array = data["regions"]
	for ri in regions.size():
		var reg: Dictionary = regions[ri]
		# Targa col nome in fondo alla fascia della regione (le regioni vanno da ovest a est).
		var band := _map.size.x / regions.size()
		var plate := PanelContainer.new()
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.1, 0.07, 0.05, 0.72)
		sb.border_color = Color(0.85, 0.7, 0.45)
		sb.set_border_width_all(1)
		sb.set_content_margin_all(4)
		plate.add_theme_stylebox_override("panel", sb)
		var title := UI.label(str(reg["name"]), 13, Color(1.0, 0.92, 0.75))
		title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		title.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		title.custom_minimum_size = Vector2(band - 34, 0)
		plate.add_child(title)
		plate.position = Vector2(band * ri + 12, _map.size.y * 0.84)
		plate.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_map.add_child(plate)
		for n in reg["nodes"]:
			var b := Button.new()
			var locked := not bool(n["unlocked"])
			b.text = ("★ " if bool(n["champion"]) else "") + ("✓" if bool(n["cleared"]) else ("·" if locked else "!"))
			b.custom_minimum_size = Vector2(44, 40)
			b.position = _node_pos(n) - Vector2(22, 20)
			b.disabled = locked
			b.tooltip_text = str(n["name"])
			var node: Dictionary = n
			var region: Dictionary = reg
			b.pressed.connect(func() -> void: _select(node, region))
			_map.add_child(b)
	_paths.queue_redraw()


func _draw_paths() -> void:
	if data.is_empty():
		return
	var prev := Vector2(-1, -1)
	for reg in data["regions"]:
		for n in reg["nodes"]:
			var p := _node_pos(n)
			if prev.x >= 0:
				var col := Color(0.45, 0.3, 0.16) if bool(n["unlocked"]) else Color(0.55, 0.45, 0.35, 0.6)
				# Sentiero tratteggiato a pixel.
				var steps := int(prev.distance_to(p) / 10.0)
				for k in steps:
					if k % 2 == 0:
						var a := prev.lerp(p, float(k) / steps)
						_paths.draw_rect(Rect2(a.round() - Vector2(3, 3), Vector2(6, 6)), col)
			prev = p


func _select(n: Dictionary, reg: Dictionary) -> void:
	selected = n
	var txt := "[b][color=#fc6]%s[/color][/b] — %s\n\n%s\n\n" % [reg["name"], reg["biome"], reg["intro"]]
	txt += "[b]%s[/b]%s — difficoltà %d\n%s" % [n["name"], "  ★ campione" if bool(n["champion"]) else "",
		int(n["difficulty"]), n["story"]]
	if bool(n["tutorial"]):
		txt += "\n\n[color=#9cf]Battaglia guidata: un riquadro ti spiegherà i comandi passo per passo.[/color]"
	if bool(n["cleared"]):
		txt += "\n\n[color=#7f7]Tappa già vinta: puoi rigiocarla.[/color]"
	if reg.get("outro") != null:
		txt += "\n\n[i]%s[/i]" % reg["outro"]
	_info.text = txt
	UI.clear(_actions)
	if bool(n["unlocked"]):
		_actions.add_child(UI.button("Combatti", _fight))


func _fight() -> void:
	var ids: Array = []
	var me := await Net.get_json("/api/me")
	if int(me.status) == 200:
		for u in me.data["roster"]:
			if ids.size() < 4:
				ids.append(int(u["id"]))
	if ids.is_empty():
		_status.text = "Recluta prima qualche unità nel Roster."
		return
	_status.text = "Preparazione della battaglia..."
	Net.ws_send({"type": "campaign", "node": str(selected["key"]), "units": ids})


func _on_ws(msg: Dictionary, raw: String) -> void:
	if not is_inside_tree():
		return
	match str(msg.get("type", "")):
		"matched":
			main.start_online_battle(msg, raw)
		"error":
			_status.text = str(msg.get("message", "errore"))


func _autoplay() -> void:
	await get_tree().create_timer(0.6).timeout
	await main.shot("mappa_mondo")
	_fight()
