extends Node
## Rete: richieste HTTP (JSON) e connessione WebSocket al server.

signal ws_message(msg: Dictionary, raw: String)
signal ws_closed()

var base_url := "http://127.0.0.1:7777"
var token := ""
var username := ""
## Cataloghi in cache (chiave -> dati), caricati dal menu.
var items := {}
var classes := {}

var _ws: WebSocketPeer = null
var _pending: Array[String] = []


func request(method: int, path: String, body: Variant = null) -> Dictionary:
	var http := HTTPRequest.new()
	add_child(http)
	var headers := PackedStringArray(["Content-Type: application/json"])
	if token != "":
		headers.append("Authorization: Bearer " + token)
	var data := "" if body == null else JSON.stringify(body)
	var err := http.request(base_url + path, headers, method, data)
	if err != OK:
		http.queue_free()
		return {"status": 0, "data": {"error": "richiesta non inviata"}}
	var res: Array = await http.request_completed
	http.queue_free()
	if res[0] != HTTPRequest.RESULT_SUCCESS:
		return {"status": 0, "data": {"error": "server non raggiungibile"}}
	var text: String = (res[3] as PackedByteArray).get_string_from_utf8()
	var parsed: Variant = JSON.parse_string(text) if text != "" else null
	return {"status": int(res[1]), "data": parsed}


func get_json(path: String) -> Dictionary:
	return await request(HTTPClient.METHOD_GET, path)


func post_json(path: String, body: Variant = null) -> Dictionary:
	return await request(HTTPClient.METHOD_POST, path, body if body != null else {})


func put_json(path: String, body: Variant) -> Dictionary:
	return await request(HTTPClient.METHOD_PUT, path, body)


func delete_json(path: String) -> Dictionary:
	return await request(HTTPClient.METHOD_DELETE, path)


static func error_text(r: Dictionary) -> String:
	if r.data is Dictionary and (r.data as Dictionary).has("error"):
		return str(r.data["error"])
	if int(r.status) == 0:
		return "server non raggiungibile"
	return "errore %d" % int(r.status)


## Accesso o registrazione. Restituisce "" se va a buon fine, altrimenti l'errore.
func auth(register: bool, user: String, password: String) -> String:
	var path := "/api/register" if register else "/api/login"
	var r := await post_json(path, {"username": user, "password": password})
	if int(r.status) == 200:
		token = r.data["token"]
		username = r.data["username"]
		return ""
	return error_text(r)


func logout() -> void:
	if token != "":
		await post_json("/api/logout")
	token = ""
	username = ""
	ws_close()


func ws_connect() -> void:
	if _ws != null:
		return
	var url := base_url.replace("https://", "wss://").replace("http://", "ws://")
	_ws = WebSocketPeer.new()
	_ws.inbound_buffer_size = 1 << 22
	_ws.outbound_buffer_size = 1 << 20
	_ws.max_queued_packets = 4096
	_ws.connect_to_url(url + "/ws?token=" + token.uri_encode())


func ws_close() -> void:
	if _ws != null:
		_ws.close()
		_ws = null
	_pending.clear()


func ws_send(msg: Dictionary) -> void:
	ws_send_raw(JSON.stringify(msg))


func ws_send_raw(text: String) -> void:
	if _ws == null:
		ws_connect()
	if _ws.get_ready_state() == WebSocketPeer.STATE_OPEN:
		_ws.send_text(text)
	else:
		_pending.append(text)


## Invia un'azione già serializzata dal motore (JSON di `Action`).
func ws_send_action(action_json: String) -> void:
	ws_send_raw('{"type":"action","action":%s}' % action_json)


func _process(_delta: float) -> void:
	if _ws == null:
		return
	_ws.poll()
	match _ws.get_ready_state():
		WebSocketPeer.STATE_OPEN:
			while not _pending.is_empty():
				_ws.send_text(_pending.pop_front())
			while _ws.get_available_packet_count() > 0:
				var raw := _ws.get_packet().get_string_from_utf8()
				var msg: Variant = JSON.parse_string(raw)
				if msg is Dictionary:
					ws_message.emit(msg, raw)
		WebSocketPeer.STATE_CLOSED:
			_ws = null
			_pending.clear()
			ws_closed.emit()
