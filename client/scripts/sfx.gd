extends Node
## Audio: musica in loop ed effetti sonori originali (tools/audio/synth.py).

const FX_SOUND := {
	"slash": "slash", "impact": "hit", "fire": "fire", "ice": "ice", "thunder": "thunder", "holy": "holy",
	"heal": "heal", "buff": "buff", "debuff": "debuff", "poison": "poison", "drain": "drain",
}
const SETTINGS := "user://audio.cfg"

var enabled := true
var music_volume_db := -8.0
var _music: AudioStreamPlayer
var _current := ""
var _pool: Array[AudioStreamPlayer] = []
var _cache := {}


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	_music = AudioStreamPlayer.new()
	_music.volume_db = music_volume_db
	add_child(_music)
	for i in 8:
		var p := AudioStreamPlayer.new()
		p.volume_db = -4.0
		add_child(p)
		_pool.append(p)
	var cfg := ConfigFile.new()
	if cfg.load(SETTINGS) == OK:
		enabled = bool(cfg.get_value("audio", "enabled", true))
	if "--mute" in OS.get_cmdline_user_args() or DisplayServer.get_name() == "headless":
		enabled = false


func _stream(name: String) -> AudioStream:
	if not _cache.has(name):
		var path := "res://assets/audio/%s.wav" % name
		_cache[name] = load(path) if ResourceLoader.exists(path) else null
	return _cache[name]


## Musica di sottofondo in loop ("menu", "battle", "world").
func music(name: String) -> void:
	if name == _current:
		return
	_current = name
	_music.stop()
	if not enabled:
		return
	var s := _stream("music_" + name) as AudioStreamWAV
	if s == null:
		return
	s.loop_mode = AudioStreamWAV.LOOP_FORWARD
	s.loop_begin = 0
	s.loop_end = s.data.size() / 2
	_music.stream = s
	_music.play()


## Breve brano senza loop (vittoria, sconfitta) al posto della musica.
func jingle(name: String) -> void:
	_current = ""
	_music.stop()
	if not enabled:
		return
	var s := _stream("jingle_" + name)
	if s != null:
		_music.stream = s
		_music.play()


func play(name: String) -> void:
	if not enabled:
		return
	var s := _stream("sfx_" + name)
	if s == null:
		return
	for p in _pool:
		if not p.playing:
			p.stream = s
			p.play()
			return


func play_fx(fx: String) -> void:
	play(FX_SOUND.get(fx, "hit"))


func set_enabled(on: bool) -> void:
	enabled = on
	var cfg := ConfigFile.new()
	cfg.set_value("audio", "enabled", on)
	cfg.save(SETTINGS)
	if not on:
		_music.stop()
	else:
		var m := _current
		_current = ""
		if m != "":
			music(m)
