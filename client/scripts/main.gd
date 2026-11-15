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

var opts := {}
var current: Control = null


func _ready() -> void:
	_parse_args()
	var t := Theme.new()
	t.default_font_size = 15
	theme = t
	if opts.has("server"):
		Net.base_url = str(opts["server"])
	if opts.get("offline", false):
		start_local_battle()
	elif opts.get("autoplay", false) and opts.has("user"):
		_autoplay_login()
	else:
		show_login()


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
	await get_tree().create_timer(2.0).timeout
	await shot("menu_dopo_partita")
	finish(0, summary)


func finish(code: int, summary: String) -> void:
	printerr("AUTOPLAY ", summary)
	get_tree().quit(code)


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
