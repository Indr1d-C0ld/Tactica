extends Control
## Schermata di battaglia: online (il server decide) o schermaglia locale.
##
## Il motore Rust (`TacticaEngine`) fornisce caselle raggiungibili, bersagli,
## anteprime e suggerimenti; in locale risolve anche le azioni.

const UI := preload("res://scripts/ui.gd")
const Board3D := preload("res://scripts/board3d.gd")
const VGA_SHADER := preload("res://shaders/vga.gdshader")
## Stili grafici: fattore di ingrandimento della risoluzione interna e livelli di colore.
const STYLES := [
	{"name": "VGA", "scale": 3, "levels": 6.0, "dither": 0.5},
	{"name": "SVGA", "scale": 2, "levels": 10.0, "dither": 0.8},
	{"name": "Nitido", "scale": 2, "levels": 0.0, "dither": 0.0},
]
const DIR_NAMES := [["N", "Nord"], ["E", "Est"], ["S", "Sud"], ["W", "Ovest"]]
const ANNOUNCE := {
	"Damaged": [Color(1, 0.45, 0.4), "-%d"],
	"Healed": [Color(0.5, 1, 0.55), "+%d"],
	"TerrainDamage": [Color(1, 0.6, 0.3), "-%d"],
}

var main: Node
var online := false
var my_team := 0
var opponent := "IA"
## Unità comandate da questo giocatore (in cooperativa solo una parte della squadra).
var mine := {}
var allies: Array = []
var mission: Dictionary = {}
var war: Dictionary = {}
var tutorial_step := -1
var _tutorial: PanelContainer
var _tutorial_text: RichTextLabel
const TUTORIAL := [
	"1/4 · Tocca a una tua unità (la freccia dorata). Premi «Muovi» e scegli una casella blu: sono quelle che può raggiungere.",
	"2/4 · Scegli un'abilità e passa il mouse sulle caselle rosse: vedi danni, probabilità di colpire ed eventuali statuti violati. Clicca per agire.",
	"3/4 · Premi «Fine turno» e scegli dove guardare: chi attacca alle spalle o di fianco colpisce più facilmente.",
	"4/4 · In alto ci sono gli Statuti di questa battaglia: se li violi l'unità viene sospesa, alla seconda volta espulsa. Ora tocca a te: buona fortuna!",
]
var engine: TacticaEngine
var snapshot: Dictionary = {}
var units: Array = []

var mode := "idle"  # idle | move | ability | facing
var sel_ability := ""
var abilities: Array = []
var reach := {}
var targets := {}
var busy := false
var ended := false
var my_actions := 0
var shots_taken := {}

var board: Board3D
var _vp: SubViewport
var _view: TextureRect
var _hud: Control
var _style := 0
var _top: Label
var _card: RichTextLabel
var _actions: VBoxContainer
var _log: RichTextLabel
var _hint: RichTextLabel
var _overlay: Control


func setup_online(matched: Dictionary, raw: String) -> void:
	online = true
	my_team = int(matched["team"])
	opponent = str(matched.get("opponent", "?"))
	for i in matched.get("mine", []):
		mine[int(i)] = true
	allies = matched.get("allies", [])
	if matched.get("mission") is Dictionary:
		mission = matched["mission"]
		if bool(mission.get("tutorial", false)):
			tutorial_step = 0
	if matched.get("war") is Dictionary:
		war = matched["war"]
	engine = TacticaEngine.new()
	engine.set_snapshot_from_message(raw)


func setup_local(seed: int) -> void:
	online = false
	my_team = 0
	opponent = "IA locale"
	var a := TacticaEngine.demo_party_json(0, 3)
	var b := TacticaEngine.demo_party_json(1, 3)
	var both := a.substr(0, a.length() - 1) + "," + b.substr(1)
	engine = TacticaEngine.new_local(seed, both, 2)


func _ready() -> void:
	Sfx.music("battle")
	_build_ui()
	if online:
		Net.ws_message.connect(_on_ws)
		Net.ws_closed.connect(func() -> void: _log_line("[color=#f66]Connessione chiusa dal server[/color]"))
	refresh()
	board.set_map(snapshot["map"])
	_fit_board()
	resized.connect(_fit_board)
	_log_line("[b]Statuti:[/b] " + ", ".join(engine.statute_names()))
	if tutorial_step >= 0:
		_tutorial_text = RichTextLabel.new()
		_tutorial_text.bbcode_enabled = true
		_tutorial_text.fit_content = true
		_tutorial_text.custom_minimum_size = Vector2(560, 0)
		_tutorial = UI.panel(_tutorial_text, 10)
		_tutorial.set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
		_tutorial.offset_top = 56
		_tutorial.offset_left = -290
		_tutorial.offset_right = 290
		_tutorial.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(_tutorial)
		_tutorial_update()
	_after_change()


# ------------------------------------------------------------------ interfaccia

func _build_ui() -> void:
	var bg := ColorRect.new()
	bg.color = Color(0.09, 0.1, 0.13)
	bg.mouse_filter = Control.MOUSE_FILTER_IGNORE
	UI.full_rect(bg)
	add_child(bg)
	var backdrop := TextureRect.new()
	backdrop.texture = preload("res://scripts/art.gd").tex("backdrop")
	backdrop.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	backdrop.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
	backdrop.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	backdrop.mouse_filter = Control.MOUSE_FILTER_IGNORE
	UI.full_rect(backdrop)
	add_child(backdrop)
	_vp = SubViewport.new()
	_vp.transparent_bg = true
	_vp.own_world_3d = true
	_vp.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	_vp.canvas_item_default_texture_filter = Viewport.DEFAULT_CANVAS_ITEM_TEXTURE_FILTER_NEAREST
	add_child(_vp)
	board = Board3D.new()
	_vp.add_child(board)
	_view = TextureRect.new()
	_view.texture = _vp.get_texture()
	_view.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_view.stretch_mode = TextureRect.STRETCH_SCALE
	_view.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	_view.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var mat := ShaderMaterial.new()
	mat.shader = VGA_SHADER
	_view.material = mat
	add_child(_view)
	_hud = Control.new()
	_hud.mouse_filter = Control.MOUSE_FILTER_IGNORE
	UI.full_rect(_hud)
	_hud.draw.connect(func() -> void: board.draw_hud(_hud, float(STYLES[_style]["scale"]), _hud.get_theme_default_font()))
	add_child(_hud)

	var top := UI.panel(UI.label(""), 8)
	_top = top.get_child(0).get_child(0)
	top.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	add_child(top)

	var left := UI.vbox(8)
	_card = RichTextLabel.new()
	_card.bbcode_enabled = true
	_card.fit_content = true
	_card.custom_minimum_size = Vector2(260, 0)
	left.add_child(_card)
	_actions = UI.vbox(4)
	left.add_child(_actions)
	var lp := UI.panel(left)
	lp.set_anchors_and_offsets_preset(Control.PRESET_LEFT_WIDE)
	lp.offset_top = 48
	lp.offset_right = 290
	lp.offset_bottom = -8
	lp.self_modulate = Color(1, 1, 1, 0.9)
	add_child(lp)

	_log = RichTextLabel.new()
	_log.bbcode_enabled = true
	_log.scroll_following = true
	_log.custom_minimum_size = Vector2(250, 0)
	_log.size_flags_vertical = Control.SIZE_EXPAND_FILL
	var rp := UI.panel(_log)
	rp.set_anchors_and_offsets_preset(Control.PRESET_RIGHT_WIDE)
	rp.offset_top = 48
	rp.offset_left = -270
	rp.offset_bottom = -8
	rp.self_modulate = Color(1, 1, 1, 0.9)
	add_child(rp)

	_hint = RichTextLabel.new()
	_hint.bbcode_enabled = true
	_hint.fit_content = true
	_hint.custom_minimum_size = Vector2(560, 0)
	var hp := UI.panel(_hint, 8)
	hp.set_anchors_and_offsets_preset(Control.PRESET_CENTER_BOTTOM)
	hp.offset_left = -290
	hp.offset_right = 290
	hp.offset_top = -70
	hp.offset_bottom = -8
	hp.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(hp)


var _dragging := false


## Risoluzione interna = finestra / fattore dello stile; l'immagine viene
## ingrandita senza sfocatura e passata nel filtro dei colori.
func _fit_board() -> void:
	if _vp == null:
		return
	var st: Dictionary = STYLES[_style]
	var k := int(st["scale"])
	var vs := Vector2i(maxi(1, ceili(size.x / k)), maxi(1, ceili(size.y / k)))
	_vp.size = vs
	_view.position = Vector2.ZERO
	_view.size = Vector2(vs * k)
	var mat := _view.material as ShaderMaterial
	mat.set_shader_parameter("source_size", Vector2(vs))
	mat.set_shader_parameter("levels", float(st["levels"]))
	mat.set_shader_parameter("dither", float(st["dither"]))
	mat.set_shader_parameter("enabled", float(st["levels"]) > 0.0)


func cycle_style() -> void:
	_style = (_style + 1) % STYLES.size()
	_fit_board()
	_rebuild_actions()


func _vp_mouse() -> Vector2:
	return get_local_mouse_position() / float(STYLES[_style]["scale"])


func _process(_delta: float) -> void:
	if _hud != null:
		_hud.queue_redraw()


func refresh() -> void:
	snapshot = JSON.parse_string(engine.snapshot_json())
	units = JSON.parse_string(engine.units_view_json())
	board.active = _active()
	var obj: Variant = snapshot.get("objective")
	board.champion = int(obj["Champion"]) if obj is Dictionary and obj.has("Champion") else -1
	board.units = units
	var turn_txt := "Turno %d" % int(snapshot["turn"])
	if not war.is_empty():
		var t: Array = war["tags"]
		var n: Array = war["names"]
		var cup := "" if war.get("tournament") == null else "Torneo «%s» · " % war["tournament"]
		_top.text = "%s · %sGuerra: [%s] %s contro [%s] %s · Tu: %s · %s · Statuti: %s" % [
			turn_txt, cup, t[0], n[0], t[1], n[1], "[%s]" % t[my_team], engine.biome_name(),
			", ".join(engine.statute_names())]
	elif not mission.is_empty():
		var goal := "abbatti il campione" if bool(mission.get("hunt", false)) else "sconfiggi tutti i nemici"
		_top.text = "%s · %s (%s) · Obiettivo: %s · %s · Statuti: %s · Con: %s" % [
			turn_txt, mission["name"], mission["difficulty_name"], goal, engine.biome_name(),
			", ".join(engine.statute_names()), ", ".join(allies) if not allies.is_empty() else "—"]
	else:
		_top.text = "%s · %s · Statuti: %s · Avversario: %s · Tu: squadra %s" % [
			turn_txt, engine.biome_name(), ", ".join(engine.statute_names()), opponent,
			"blu" if my_team == 0 else "rossa"]
	_show_card(_active())
	_rebuild_actions()


func _active() -> int:
	var a: Variant = snapshot.get("active")
	return -1 if a == null else int(a)


func my_turn() -> bool:
	var a := _active()
	if ended or bool(snapshot.get("finished", false)) or a < 0:
		return false
	if not mine.is_empty():
		return mine.has(a)
	return int(units[a]["team"]) == my_team


func _tutorial_update() -> void:
	if _tutorial == null:
		return
	if tutorial_step >= TUTORIAL.size():
		_tutorial.visible = false
		return
	_tutorial_text.text = "[color=#fc6]Tutorial[/color]  " + TUTORIAL[tutorial_step]


## Avanza il tutorial in base agli eventi delle proprie unità.
func _tutorial_events(msg: Dictionary) -> void:
	if tutorial_step < 0:
		return
	for e in msg.get("events", []):
		if not (e is Dictionary) or e.is_empty():
			continue
		var kind: String = e.keys()[0]
		var v: Variant = e[kind]
		var own: bool = v is Dictionary and v.has("unit") and mine.has(int(v["unit"]))
		if tutorial_step == 0 and kind == "Moved" and own:
			tutorial_step = 1
		elif tutorial_step == 1 and kind == "AbilityUsed" and own:
			tutorial_step = 2
		elif tutorial_step == 2 and kind == "TurnStarted" and not own:
			tutorial_step = 3
		elif tutorial_step == 3 and kind == "TurnStarted" and own:
			tutorial_step = 4
	_tutorial_update()


func _show_card(i: int) -> void:
	if i < 0 or i >= units.size():
		_card.text = ""
		return
	var u: Dictionary = units[i]
	var st: Dictionary = u["stats"]
	var team_col := "#6b9cff" if int(u["team"]) == 0 else "#ff7b6b"
	_card.text = ("[b][color=%s]%s[/color][/b]\n%s · Lv %d\nHP %d/%d   MP %d/%d\n" +
		"ATK %d DEF %d MAG %d RES %d\nVEL %d MOV %d SALTO %d SCHIVATA %d") % [
		team_col, u["name"], u["class_name"], int(u["level"]), int(u["hp"]), int(u["max_hp"]),
		int(u["mp"]), int(u["max_mp"]), int(st["atk"]), int(st["def"]), int(st["mag"]),
		int(st["res"]), int(st["spd"]), int(st["mov"]), int(st["jump"]), int(st["evade"])]
	var sts: Array[String] = []
	for status in u.get("statuses", []):
		sts.append("[color=%s]%s (%d)[/color]" % ["#f9a" if bool(status["negative"]) else "#9fc", status["name"], int(status["turns"])])
	if not sts.is_empty():
		_card.text += "\n" + ", ".join(sts)
	if not mine.is_empty() and int(u["team"]) == my_team:
		_card.text += "\n[color=#9cf]%s[/color]" % ("Tua unità" if mine.has(i) else "Unità di un alleato")
	if int(u["penalties"]) > 0:
		_card.text += "\n[color=#f96]Sanzioni: %d[/color]" % int(u["penalties"])


func _rebuild_actions() -> void:
	UI.clear(_actions)
	var view := UI.hbox(4)
	view.add_child(UI.button("<", func() -> void: board.rotate_by(-45.0)))
	view.add_child(UI.button(">", func() -> void: board.rotate_by(45.0)))
	view.add_child(UI.button("Vista", func() -> void: board.reset_view()))
	view.add_child(UI.button(str(STYLES[_style]["name"]), cycle_style))
	_actions.add_child(view)
	var help := UI.label("Q/E ruota · rotella zoom · tasto centrale: orbita (Maiusc: sposta) · F2 stile", 11, UI.MUTED)
	help.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	help.custom_minimum_size = Vector2(220, 0)
	_actions.add_child(help)
	if ended:
		return
	if not my_turn():
		var who := "Turno dell'avversario..."
		var a := _active()
		if not mine.is_empty() and a >= 0 and int(units[a]["team"]) == my_team:
			who = "Turno di un alleato..."
		_actions.add_child(UI.label(who if not busy else "In attesa...", 0, UI.MUTED))
		if mode != "idle":
			_set_mode("idle")
		return
	var a := _active()
	if not bool(snapshot["moved"]):
		_actions.add_child(UI.button("Muovi", func() -> void: _set_mode("move")))
	if not bool(snapshot["acted"]):
		abilities = JSON.parse_string(engine.abilities_json(a))
		for ab in abilities:
			var key := str(ab["key"])
			var cost := int(ab["mp_cost"])
			var txt := str(ab["name"]) + ("" if cost == 0 else "  (%d MP)" % cost)
			if ab.get("status") != null:
				txt += " · " + str(ab["status"])
			var b := UI.button(txt, func() -> void: _set_mode("ability", key))
			b.disabled = not bool(ab["usable"])
			_actions.add_child(b)
	_actions.add_child(UI.button("Fine turno", func() -> void:
		_set_mode("facing")
		_rebuild_actions()))
	_actions.add_child(HSeparator.new())
	_actions.add_child(UI.button("Suggerimento IA", _ai_step))
	if online:
		_actions.add_child(UI.button("Abbandona", func() -> void: Net.ws_send({"type": "leave"})))
	if mode == "facing":
		var row := UI.hbox(4)
		for d in DIR_NAMES:
			var key: String = d[0]
			row.add_child(UI.button(d[1], func() -> void: _send(TacticaEngine.make_end_turn(key))))
		_actions.add_child(UI.label("Direzione di fine turno:", 0, UI.ACCENT))
		_actions.add_child(row)


func _set_mode(m: String, ability := "") -> void:
	mode = m
	sel_ability = ability
	reach.clear()
	targets.clear()
	board.highlight = {}
	board.area = {}
	var a := _active()
	match m:
		"move":
			for p in engine.reachable(a):
				reach[p] = true
				board.highlight[p] = Color(0.4, 0.7, 1.0, 0.85)
			_hint.text = "Scegli dove muovere (clic destro per annullare)"
		"ability":
			for p in engine.targets(a, ability):
				targets[p] = true
				board.highlight[p] = Color(1.0, 0.35, 0.3, 0.8)
			_hint.text = "Scegli il bersaglio"
		"facing":
			_hint.text = "Scegli la direzione: gli attacchi alle spalle colpiscono più facilmente"
		_:
			_hint.text = ""


func _preview(t: Vector2i) -> void:
	var p: Dictionary = JSON.parse_string(engine.preview_json(_active(), sel_ability, t))
	board.area = {}
	for c in p.get("area", []):
		board.area[Vector2i(int(c[0]), int(c[1]))] = true
	var parts: Array[String] = []
	for o in p.get("outcomes", []):
		var who := str(o["name"])
		var extra := ""
		if o.get("status") != null:
			extra = " [color=#c9f]%s %d%%[/color]" % [o["status"], int(o["status_chance"])]
		if bool(o.get("cleanse", false)):
			extra += " [color=#9fc]purifica[/color]"
		if int(o["amount"]) == 0 and not bool(o["heal"]):
			parts.append("%s%s" % [who, extra])
			continue
		if bool(o["heal"]):
			parts.append("[color=#7f7]%s +%d[/color]" % [who, int(o["amount"])])
		else:
			var col := "#fc6" if bool(o["ally"]) else "#f88"
			var ko := " KO" if bool(o["lethal"]) else ""
			parts.append("[color=%s]%s -%d (%d%%)%s[/color]%s" % [col, who, int(o["amount"]), int(o["hit"]), ko, extra])
	var txt := "  ".join(parts) if not parts.is_empty() else "[color=#999]nessun bersaglio[/color]"
	var v: Array = p.get("violations", [])
	if not v.is_empty():
		txt += "\n[color=#f63][b]Viola lo statuto:[/b] %s → sanzione[/color]" % ", ".join(v)
	_hint.text = txt


func _unhandled_input(event: InputEvent) -> void:
	if ended:
		return
	if event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_Q:
				board.rotate_by(-45.0)
			KEY_E:
				board.rotate_by(45.0)
			KEY_R:
				board.reset_view()
			KEY_F2:
				cycle_style()
		return
	if event is InputEventMouseButton and event.pressed and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		board.zoom_by(1.0 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1.0)
		return
	if event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_MIDDLE:
		_dragging = event.pressed
		return
	if event is InputEventMouseMotion and _dragging:
		if event.shift_pressed:
			board.pan_by(event.relative / float(STYLES[_style]["scale"]))
		else:
			board.rotate_by(-event.relative.x * 0.4, false)
			board.tilt_by(event.relative.y * 0.3)
		return
	if event is InputEventMouseMotion:
		var t := board.pick(_vp_mouse())
		if t != board.hover:
			board.hover = t
			var under := _unit_at(t)
			if under >= 0:
				_show_card(under)
			else:
				_show_card(_active())
			if mode == "ability" and targets.has(t):
				_preview(t)
	elif event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_RIGHT:
			_set_mode("idle")
			_rebuild_actions()
			return
		if event.button_index != MOUSE_BUTTON_LEFT or not my_turn() or busy:
			return
		var t := board.pick(_vp_mouse())
		if mode == "move" and reach.has(t):
			_send(TacticaEngine.make_move(t.x, t.y))
		elif mode == "ability" and targets.has(t):
			_send(TacticaEngine.make_act(sel_ability, t.x, t.y))


func _unit_at(t: Vector2i) -> int:
	for u in units:
		if bool(u["active"]) and int(u["x"]) == t.x and int(u["y"]) == t.y:
			return int(u["index"])
	return -1


func _log_line(s: String) -> void:
	_log.append_text(s + "\n")


# ------------------------------------------------------------------ azioni

func _send(action_json: String) -> void:
	if action_json == "" or busy:
		return
	_set_mode("idle")
	if my_turn():
		my_actions += 1
	if online:
		busy = true
		Net.ws_send_action(action_json)
		_rebuild_actions()
		return
	var raw := engine.apply_json(action_json)
	var res: Dictionary = JSON.parse_string(raw)
	if res.has("error"):
		_log_line("[color=#f66]%s[/color]" % res["error"])
		push_warning("azione rifiutata: %s -> %s" % [action_json, res["error"]])
		return
	refresh()
	_announce(res, raw)
	_after_change()


func _ai_step() -> void:
	_send(engine.ai_next_json())


func _announce(msg: Dictionary, raw: String) -> void:
	for line in engine.describe_message_events(raw):
		_log_line(line)
	for e in msg.get("events", []):
		if not (e is Dictionary) or e.is_empty():
			continue
		var kind: String = e.keys()[0]
		var v: Variant = e[kind]
		if not (v is Dictionary) or not v.has("unit"):
			continue
		var u: Dictionary = units[int(v["unit"])]
		var at := Vector2i(int(u["x"]), int(u["y"]))
		if kind == "AbilityUsed":
			board.play(int(v["unit"]), "act")
			var fx: Dictionary = JSON.parse_string(TacticaEngine.ability_fx(str(v["ability"])))
			if not fx.is_empty():
				var t := Vector2i(int(v["target"]["x"]), int(v["target"]["y"]))
				board.cast(int(v["unit"]), str(fx["fx"]), bool(fx["projectile"]), t, int(fx["aoe"]))
				Sfx.play_fx(str(fx["fx"]))
		elif kind == "Damaged" or kind == "TerrainDamage":
			board.play(int(v["unit"]), "hurt", 0.3)
		if kind == "Damaged":
			Sfx.play("hit")
		elif kind == "Moved":
			Sfx.play("step")
		if ANNOUNCE.has(kind) and int(v.get("amount", 1)) == 0:
			pass
		elif ANNOUNCE.has(kind):
			board.add_float(at, ANNOUNCE[kind][1] % int(v["amount"]), ANNOUNCE[kind][0])
		elif kind == "Missed":
			board.add_float(at, "mancato", Color(0.8, 0.8, 0.8))
			Sfx.play("miss")
		elif kind == "StatuteViolated":
			board.add_float(at, "Sanzione!", Color(1, 0.55, 0.2))
		elif kind == "LevelUp":
			board.add_float(at, "Livello %d!" % int(v["level"]), UI.ACCENT)
			Sfx.play("levelup")
		elif kind == "Defeated":
			board.add_float(at, "KO", UI.BAD)
			Sfx.play("ko")
		elif kind == "StatusApplied":
			var neg := str(v["status"]) in ["Poison", "Sleep", "Silence", "Slow", "Root"]
			board.effect("debuff" if neg else "buff", at)
			board.add_float(at, _status_name(str(v["status"])), Color(0.85, 0.55, 1.0) if neg else Color(0.5, 1.0, 0.85))
		elif kind == "StatusResisted":
			board.add_float(at, "resiste", Color(0.8, 0.8, 0.8))
		elif kind == "Asleep":
			board.add_float(at, "Zzz", Color(0.6, 0.75, 1.0))
		elif kind == "StatusDamage":
			board.add_float(at, "-%d" % int(v["amount"]), Color(0.6, 0.95, 0.4))
			board.effect("poison", at)


func _after_change() -> void:
	if bool(snapshot.get("finished", false)):
		if not online:
			_show_end_local()
		return
	if my_turn():
		_autoplay_tick()
	elif not online:
		await get_tree().create_timer(0.3 if not main.autoplay() else 0.02).timeout
		if not ended and not my_turn():
			_ai_step()
	elif not my_turn():
		_rebuild_actions()


func _autoplay_tick() -> void:
	if not main.autoplay() or ended:
		return
	if not shots_taken.has("inizio"):
		shots_taken["inizio"] = true
		await get_tree().create_timer(0.3).timeout
		await main.shot("battaglia_inizio")
		if main.opts.has("shots"):
			# Dimostrazione della telecamera: rotazione, inclinazione, zoom, stili.
			board.rotate_by(135.0, false)
			board.tilt_by(18.0)
			await get_tree().create_timer(0.2).timeout
			await main.shot("battaglia_ruotata")
			board.zoom_by(4.0)
			await get_tree().create_timer(0.6).timeout
			await main.shot("battaglia_zoom")
			cycle_style()
			await get_tree().create_timer(0.2).timeout
			await main.shot("battaglia_svga")
			cycle_style()
			cycle_style()
			board.reset_view()
			await get_tree().create_timer(0.6).timeout
	await get_tree().create_timer(0.05).timeout
	if not my_turn() or busy:
		return
	var next := engine.ai_next_json()
	var parsed: Variant = JSON.parse_string(next)
	# Mostra una volta l'anteprima di un'abilità prima di usarla.
	if not shots_taken.has("anteprima") and parsed is Dictionary and parsed.has("Act"):
		shots_taken["anteprima"] = true
		var act: Dictionary = parsed["Act"]
		var key := _ability_key(str(act["ability"]))
		var t := Vector2i(int(act["target"]["x"]), int(act["target"]["y"]))
		_set_mode("ability", key)
		board.hover = t
		_preview(t)
		await get_tree().process_frame
		await get_tree().process_frame
		await main.shot("battaglia_anteprima")
	if my_actions == 30 and not shots_taken.has("meta"):
		shots_taken["meta"] = true
		await main.shot("battaglia_meta")
	_send(next)


## Le azioni JSON usano i nomi delle varianti Rust ("AimedShot"); i pulsanti le chiavi ("aimed_shot").
const STATUS_NAMES := {"Poison": "Veleno", "Sleep": "Sonno", "Silence": "Silenzio", "Slow": "Lentezza",
	"Root": "Radici", "Haste": "Rapidità", "Aegis": "Egida", "Regen": "Rigenerazione"}


func _status_name(variant: String) -> String:
	return STATUS_NAMES.get(variant, variant)


func _ability_key(variant: String) -> String:
	for ab in JSON.parse_string(engine.abilities_json(_active())):
		if str(ab["key"]).replace("_", "") == variant.to_lower():
			return str(ab["key"])
	return ""


# ------------------------------------------------------------------ server

func _on_ws(msg: Dictionary, raw: String) -> void:
	match str(msg.get("type", "")):
		"update":
			engine.set_snapshot_from_message(raw)
			busy = false
			refresh()
			_announce(msg, raw)
			_tutorial_events(msg)
			_after_change()
		"error":
			busy = false
			_log_line("[color=#f66]%s[/color]" % msg.get("message", "errore"))
			_rebuild_actions()
			if main.autoplay():
				_autoplay_tick()
		"ended":
			_show_end_online(msg)


func _show_end_online(msg: Dictionary) -> void:
	ended = true
	Sfx.jingle("victory" if str(msg["result"]) == "win" else "defeat")
	_rebuild_actions()
	var result := str(msg["result"])
	var title: String = str({"win": "Vittoria", "loss": "Sconfitta", "draw": "Pareggio"}.get(result, result))
	var lines: Array[String] = []
	lines.append("[font_size=33][b]%s[/b][/font_size]" % title)
	if int(msg["elo_before"]) != int(msg["elo_after"]):
		lines.append("ELO %d → %d" % [int(msg["elo_before"]), int(msg["elo_after"])])
	if msg.get("season_after") != null:
		lines.append("Punteggio stagionale %d → %d" % [int(msg["season_before"]), int(msg["season_after"])])
	lines.append("Monete +%d" % int(msg["coins"]))
	if int(msg.get("clan_points", 0)) > 0:
		lines.append("[color=#8cf]Punti clan +%d[/color]" % int(msg["clan_points"]))
	if msg.get("loot") != null:
		var key := str(msg["loot"])
		var name: String = Net.items.get(key, {}).get("name", key)
		lines.append("[color=#fc6]Bottino: %s[/color]" % name)
	lines.append("")
	for r in msg["rewards"]:
		var lv := ""
		if int(r["level_after"]) > int(r["level_before"]):
			lv = "  [color=#fc6]Lv %d → %d[/color]" % [int(r["level_before"]), int(r["level_after"])]
		lines.append("[b]%s[/b]  EXP +%d%s" % [r["name"], int(r["xp"]), lv])
		for g in r["rp"]:
			var star := "  [color=#fc6]★ padroneggiata![/color]" if bool(g["mastered_now"]) else ""
			lines.append("    %s  +%d RP (%d/%d)%s" % [g["name"], int(g["gained"]), int(g["rp"]), int(g["master_rp"]), star])
	_show_overlay("\n".join(lines), [["Torna al menu", func() -> void: main.show_menu()]])
	if main.autoplay():
		await main.shot("battaglia_fine")
		var summary := "risultato=%s monete=%d bottino=%s premi=%d" % [
			result, int(msg["coins"]), str(msg.get("loot")), msg["rewards"].size()]
		main.autoplay_after_battle(summary)


func _show_end_local() -> void:
	if ended:
		return
	ended = true
	var lw: Variant = snapshot.get("winner")
	Sfx.jingle("victory" if lw != null and int(lw) == my_team else "defeat")
	_rebuild_actions()
	var w: Variant = snapshot.get("winner")
	var title := "Pareggio" if w == null else ("Vittoria" if int(w) == my_team else "Sconfitta")
	var buttons := [["Nuova schermaglia", func() -> void: main.start_local_battle()]]
	if Net.token != "":
		buttons.append(["Torna al menu", func() -> void: main.show_menu()])
	else:
		buttons.append(["Accesso", func() -> void: main.show_login()])
	_show_overlay("[font_size=33][b]%s[/b][/font_size]\nTurni: %d" % [title, int(snapshot["turn"])], buttons)
	if main.autoplay():
		await main.shot("schermaglia_fine")
		main.finish(0, "schermaglia %s turni=%d hash=%d" % [title, int(snapshot["turn"]), engine.state_hash()])


func _show_overlay(bbcode: String, buttons: Array) -> void:
	var dim := ColorRect.new()
	dim.color = Color(0, 0, 0, 0.55)
	UI.full_rect(dim)
	add_child(dim)
	var center := CenterContainer.new()
	UI.full_rect(center)
	add_child(center)
	var box := UI.vbox(10)
	var t := RichTextLabel.new()
	t.bbcode_enabled = true
	t.fit_content = true
	t.custom_minimum_size = Vector2(460, 0)
	t.text = bbcode
	box.add_child(t)
	var row := UI.hbox()
	for b in buttons:
		row.add_child(UI.button(b[0], b[1]))
	box.add_child(row)
	center.add_child(UI.panel(box, 20))
	_overlay = center
