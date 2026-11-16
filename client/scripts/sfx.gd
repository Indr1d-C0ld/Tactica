extends Node
## Audio: musiche in loop (OGG) ed effetti sonori (WAV), tutti originali e
## sintetizzati in stile FM/MIDI anni '90 (tools/audio/music.py e sfx.py).

## Effetto visivo di un'abilità -> suono (quando l'estensione non ne indica uno).
const FX_SOUND := {
	"slash": "slash", "impact": "hit", "fire": "fire", "ice": "ice", "thunder": "thunder", "holy": "holy",
	"heal": "heal", "buff": "buff", "debuff": "debuff", "poison": "poison", "drain": "drain",
}
## Passi secondo il terreno della casella d'arrivo.
const STEPS := {"Grass": "step_grass", "Stone": "step_stone", "Sand": "step_sand", "Water": "step_water",
	"Lava": "step_stone", "Thicket": "step_thicket"}
## Volume relativo per gli effetti più invadenti o più discreti (dB).
const GAIN := {"ui_hover": -14.0, "ui_click": -6.0, "ui_tab": -6.0, "step_grass": -8.0, "step_stone": -8.0,
	"step_sand": -8.0, "step_water": -6.0, "step_thicket": -8.0, "turn": -6.0, "statute": -2.0}
## Effetti a cui si varia un po' l'intonazione, per non sembrare tutti uguali.
const VARY := ["slash", "hit", "pierce", "blunt", "arrow", "miss", "step_grass", "step_stone", "step_sand",
	"step_water", "step_thicket", "ui_click", "block"]
const SETTINGS := "user://audio.cfg"

var enabled := true
var music_volume_db := -9.0
var _music: AudioStreamPlayer
var _current := ""
var _pool: Array[AudioStreamPlayer] = []
var _cache := {}
var _rng := RandomNumberGenerator.new()
var _last := {}


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	_music = AudioStreamPlayer.new()
	_music.volume_db = music_volume_db
	add_child(_music)
	for i in 12:
		var p := AudioStreamPlayer.new()
		add_child(p)
		_pool.append(p)
	var cfg := ConfigFile.new()
	if cfg.load(SETTINGS) == OK:
		enabled = bool(cfg.get_value("audio", "enabled", true))
	if "--mute" in OS.get_cmdline_user_args() or DisplayServer.get_name() == "headless":
		enabled = false


func _exit_tree() -> void:
	# Libera i flussi in cache: niente risorse "ancora in uso" alla chiusura.
	_music.stop()
	_music.stream = null
	for p in _pool:
		p.stop()
		p.stream = null
	_cache.clear()


func _stream(name: String) -> AudioStream:
	if not _cache.has(name):
		var s: AudioStream = null
		for ext in ["ogg", "wav"]:
			var path := "res://assets/audio/%s.%s" % [name, ext]
			if ResourceLoader.exists(path):
				s = load(path)
				break
		_cache[name] = s
	return _cache[name]


## Musica di sottofondo in loop: menu, world, battle, battle2, boss, shop.
func music(name: String) -> void:
	if name == _current:
		return
	_current = name
	if not enabled:
		_music.stop()
		return
	var s := _stream("music_" + name)
	if s == null:
		_music.stop()
		return
	if s is AudioStreamOggVorbis:
		(s as AudioStreamOggVorbis).loop = true
	elif s is AudioStreamWAV:
		var w := s as AudioStreamWAV
		w.loop_mode = AudioStreamWAV.LOOP_FORWARD
		w.loop_end = w.data.size() / 2
	# Breve dissolvenza fra un brano e l'altro.
	if _music.playing:
		var tw := create_tween()
		tw.tween_property(_music, "volume_db", -40.0, 0.35)
		await tw.finished
		if _current != name:
			return
	_music.stream = s
	_music.volume_db = music_volume_db
	_music.play()


## Breve brano senza loop (vittoria, sconfitta) al posto della musica.
func jingle(name: String) -> void:
	_current = ""
	_music.stop()
	if not enabled:
		return
	var s := _stream("jingle_" + name)
	if s is AudioStreamOggVorbis:
		(s as AudioStreamOggVorbis).loop = false
	if s != null:
		_music.stream = s
		_music.volume_db = music_volume_db + 3.0
		_music.play()


## Effetto sonoro; `volume_db` si somma al volume proprio dell'effetto.
func play(name: String, volume_db := 0.0) -> void:
	if not enabled or name == "":
		return
	# Lo stesso effetto non si ripete più di una volta ogni 40 ms (aree con molti bersagli).
	var now := Time.get_ticks_msec()
	if now - int(_last.get(name, -1000)) < 40:
		return
	_last[name] = now
	var s := _stream("sfx_" + name)
	if s == null:
		return
	for p in _pool:
		if not p.playing:
			p.stream = s
			p.volume_db = -3.0 + float(GAIN.get(name, 0.0)) + volume_db
			p.pitch_scale = _rng.randf_range(0.93, 1.07) if name in VARY else 1.0
			p.play()
			return


func play_fx(fx: String) -> void:
	play(FX_SOUND.get(fx, "hit"))


func step(terrain: String) -> void:
	play(STEPS.get(terrain, "step_grass"))


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
