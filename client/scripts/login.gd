extends Control
## Schermata di accesso.

const UI := preload("res://scripts/ui.gd")

var main: Node
var _server: LineEdit
var _user: LineEdit
var _pass: LineEdit
var _status: Label


func _ready() -> void:
	var center := CenterContainer.new()
	UI.full_rect(center)
	add_child(center)

	var box := UI.vbox(10)
	box.add_child(UI.label("Tactica", 40, UI.ACCENT))
	box.add_child(UI.label("Gioco tattico a turni · statuti, risonanza, mappe procedurali", 14, UI.MUTED))
	_server = UI.line_edit("Server", Net.base_url)
	_user = UI.line_edit("Nome utente", Net.username)
	_pass = UI.line_edit("Password", "", true)
	_pass.text_submitted.connect(func(_t: String) -> void: _auth(false))
	for e in [_server, _user, _pass]:
		box.add_child(e)
	var row := UI.hbox()
	row.add_child(UI.button("Accedi", func() -> void: _auth(false)))
	row.add_child(UI.button("Registrati", func() -> void: _auth(true)))
	box.add_child(row)
	box.add_child(HSeparator.new())
	box.add_child(UI.button("Schermaglia offline contro l'IA", func() -> void: main.start_local_battle()))
	_status = UI.label("", 14, UI.BAD)
	box.add_child(_status)
	center.add_child(UI.panel(box, 24))


func _auth(register: bool) -> void:
	Net.base_url = _server.text.strip_edges().trim_suffix("/")
	_status.text = "Connessione..."
	var err: String = await Net.auth(register, _user.text.strip_edges(), _pass.text)
	if err == "":
		main.show_menu()
	else:
		_status.text = err
