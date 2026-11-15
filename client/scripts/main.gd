extends Control
## Gestione delle schermate e modalità automatica (per test e dimostrazioni).
##
## Argomenti dopo `--` (es. `godot --path client -- --autoplay --offline`):
##   --autoplay            gioca da solo con i suggerimenti dell'IA
##   --offline             schermaglia locale, senza server
##   --server=URL          indirizzo del server (predefinito http://127.0.0.1:7777)
##   --user=NOME --pass=PW credenziali (registra l'account se non esiste)
##   --mode=pve|pvp        modalità della partita automatica
##   --shots=CARTELLA      salva schermate PNG dei momenti salienti

const UI := preload("res://scripts/ui.gd")
const LoginScreen := preload("res://scripts/login.gd")
const MenuScreen := preload("res://scripts/menu.gd")
const BattleScreen := preload("res://scripts/battle.gd")
const WorldScreen := preload("res://scripts/worldmap.gd")

var opts := {}
var current: Control = null


func _ready() -> void:
	_parse_args()
	theme = _make_theme()
	_read_default_server()
	if opts.has("server"):
		Net.base_url = str(opts["server"])
	if opts.get("offline", false):
		start_local_battle()
	elif opts.get("autoplay", false) and opts.has("user"):
		_autoplay_login()
	else:
		show_login()


## Tema dell'interfaccia: cornici in pixel-art generate da tools/pixelart.
func _make_theme() -> Theme:
	var t := Theme.new()
	# Font bitmap originale, ingrandito solo per multipli interi (pixel nitidi).
	var f: FontFile = load("res://assets/fonts/tactica.fnt")
	f.fixed_size_scale_mode = TextServer.FIXED_SIZE_SCALE_INTEGER_ONLY
	f.antialiasing = TextServer.FONT_ANTIALIASING_NONE
	t.default_font = f
	t.default_font_size = UI.FONT_M
	var win := _frame("ui_window", 8, 14)
	t.set_stylebox("panel", "PanelContainer", win)
	t.set_stylebox("panel", "TabContainer", win)
	var states := {"normal": "ui_button", "hover": "ui_button_hover", "pressed": "ui_button_pressed",
		"disabled": "ui_button_disabled", "hover_pressed": "ui_button_pressed"}
	for st in states:
		t.set_stylebox(st, "Button", _frame(states[st], 6, 9))
		t.set_stylebox(st, "OptionButton", _frame(states[st], 6, 9))
	t.set_stylebox("focus", "Button", StyleBoxEmpty.new())
	t.set_stylebox("focus", "OptionButton", StyleBoxEmpty.new())
	t.set_stylebox("tab_selected", "TabContainer", _frame("ui_button_hover", 6, 9))
	t.set_stylebox("tab_unselected", "TabContainer", _frame("ui_button", 6, 9))
	t.set_stylebox("tab_hovered", "TabContainer", _frame("ui_button_hover", 6, 9))
	t.set_stylebox("normal", "LineEdit", _frame("ui_field", 6, 9))
	t.set_stylebox("focus", "LineEdit", _frame("ui_button_hover", 6, 9))
	for type in ["Label", "RichTextLabel", "LineEdit", "Button", "OptionButton", "CheckBox", "TabContainer"]:
		t.set_color("font_outline_color", type, Color(0.05, 0.03, 0.1))
	for type in ["Button", "OptionButton"]:
		t.set_color("font_color", type, Color(0.98, 0.94, 0.84))
		t.set_color("font_hover_color", type, Color(1, 1, 0.9))
		t.set_color("font_disabled_color", type, Color(0.55, 0.55, 0.55))
	return t


func _frame(name: String, margin: int, content: int) -> StyleBoxTexture:
	var s := StyleBoxTexture.new()
	s.texture = preload("res://scripts/art.gd").tex(name)
	# La finestra PixelLab ha angoli decorati più ampi della cornice generata.
	if name == "ui_window" and s.texture.get_width() > 32:
		margin = 18
		content = 20
	elif name != "ui_window" and ResourceLoader.exists("res://assets/override/%s.png" % name):
		# Cornici PixelLab dei pulsanti: angoli arrotondati di 9 pixel.
		margin = 9
		content = 10
	for side in [SIDE_LEFT, SIDE_TOP, SIDE_RIGHT, SIDE_BOTTOM]:
		s.set_texture_margin(side, margin)
		s.set_content_margin(side, content)
	return s


func _parse_args() -> void:
	for a in OS.get_cmdline_user_args():
		if not a.begins_with("--"):
			continue
		var kv := a.substr(2).split("=", true, 1)
		opts[kv[0]] = kv[1] if kv.size() > 1 else true


func autoplay() -> bool:
	return opts.get("autoplay", false)


func _switch(screen: Control) -> void:
	if current != null:
		remove_child(current)
		current.queue_free()
	current = screen
	add_child(screen)
	UI.full_rect(screen)


func show_login() -> void:
	var s := LoginScreen.new()
	s.main = self
	_switch(s)


func show_menu() -> void:
	var s := MenuScreen.new()
	s.main = self
	_switch(s)


func show_world() -> void:
	var s := WorldScreen.new()
	s.main = self
	_switch(s)


func start_online_battle(matched: Dictionary, raw: String) -> void:
	var s := BattleScreen.new()
	s.main = self
	s.setup_online(matched, raw)
	_switch(s)


func start_local_battle() -> void:
	var s := BattleScreen.new()
	s.main = self
	s.setup_local(randi())
	_switch(s)


## Salva una schermata se richiesto da `--shots`.
func shot(name: String) -> void:
	if not opts.has("shots"):
		return
	await RenderingServer.frame_post_draw
	var dir := str(opts["shots"])
	DirAccess.make_dir_recursive_absolute(dir)
	var path := "%s/%s.png" % [dir, name]
	get_viewport().get_texture().get_image().save_png(path)
	printerr("SCHERMATA ", path)


## Fine della partita automatica: torna al menu, fotografa e chiude.
func autoplay_after_battle(summary: String) -> void:
	show_menu()
	await get_tree().create_timer(1.0).timeout
	if str(opts.get("mode", "")) in ["coop", "war"] and current != null:
		current._tabs.current_tab = 3
	if str(opts.get("mode", "")) == "campaign":
		show_world()
	await get_tree().create_timer(1.5).timeout
	await shot("menu_dopo_partita")
	finish(0, summary)


func finish(code: int, summary: String) -> void:
	printerr("AUTOPLAY ", summary)
	get_tree().quit(code)


## Nei pacchetti distribuiti, `server.txt` accanto all'eseguibile indica il
## server predefinito; `--server=URL` ha comunque la precedenza.
func _read_default_server() -> void:
	if not OS.has_feature("template"):
		return
	var path := OS.get_executable_path().get_base_dir().path_join("server.txt")
	if not FileAccess.file_exists(path):
		return
	var url := FileAccess.get_file_as_string(path).strip_edges().trim_suffix("/")
	if url.begins_with("http://") or url.begins_with("https://"):
		Net.base_url = url


func _autoplay_login() -> void:
	var user := str(opts["user"])
	var pw := str(opts.get("pass", "password-lunga"))
	var err := await Net.auth(true, user, pw)
	if err != "":
		err = await Net.auth(false, user, pw)
	if err != "":
		finish(1, "accesso fallito: " + err)
		return
	show_menu()
