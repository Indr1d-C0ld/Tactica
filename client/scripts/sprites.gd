extends RefCounted
## Accesso all'atlante degli sprite generato da tools/pixelart (units.png).

const Art := preload("res://scripts/art.gd")
static var UNITS_TEX: Texture2D = Art.tex("units")
## Atlante PixelLab con le varianti dei popoli: riga ((classe * 5 + popolo) * 2 + squadra).
static var PEOPLE_TEX: Texture2D = _people_tex()
const PEOPLE_ORDER := ["human", "sylvan", "stonekin", "winged", "undine"]
const CELL := 32
const CLASS_ORDER := ["vanguard", "scout", "arcanist", "mender", "runeblade", "warden",
	"shadowblade", "dragoon", "bard", "alchemist", "geomancer", "summoner"]
const HAIR_COUNT := 5
const SKIN_COUNT := 3
const FRAME := {"front_idle0": 0, "front_idle1": 1, "back_idle0": 2, "back_idle1": 3,
	"front_act": 4, "back_act": 5, "ko": 6,
	"front_walk0": 7, "front_walk1": 8, "back_walk0": 9, "back_walk1": 10}


## Riga dell'atlante: classe, squadra e aspetto (capelli e carnagione derivati dal nome).
static func row(cls: String, team: int, name: String) -> int:
	var c := maxi(0, CLASS_ORDER.find(cls))
	var hsh := absi(name.hash())
	var hair := hsh % HAIR_COUNT
	var skin := (hsh / HAIR_COUNT) % SKIN_COUNT
	return ((c * 2 + clampi(team, 0, 1)) * HAIR_COUNT + hair) * SKIN_COUNT + skin


static func _people_tex() -> Texture2D:
	if ResourceLoader.exists("res://assets/override/units_people.png") or FileAccess.file_exists("user://override/units_people.png"):
		return Art.tex("units_people")
	return null


## Atlante da usare: quello dei popoli se c'è, altrimenti quello classico.
static func atlas() -> Texture2D:
	return PEOPLE_TEX if PEOPLE_TEX != null else UNITS_TEX


static func region(cls: String, team: int, name: String, frame: String, people := "human") -> Rect2:
	if PEOPLE_TEX != null:
		var c := maxi(0, CLASS_ORDER.find(cls))
		var p := maxi(0, PEOPLE_ORDER.find(people))
		return Rect2(FRAME[frame] * CELL, ((c * PEOPLE_ORDER.size() + p) * 2 + clampi(team, 0, 1)) * CELL, CELL, CELL)
	return Rect2(FRAME[frame] * CELL, row(cls, team, name) * CELL, CELL, CELL)


## Ritratto per l'interfaccia (TextureRect ingrandito senza sfocatura).
static func portrait(cls: String, team: int, name: String, scale := 3, people := "human") -> TextureRect:
	var at := AtlasTexture.new()
	at.atlas = atlas()
	at.region = region(cls, team, name, "front_idle0", people)
	var tr := TextureRect.new()
	var face := face_texture(cls, team, people)
	tr.texture = face if face != null else at
	tr.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	tr.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	tr.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	tr.custom_minimum_size = Vector2(CELL * scale, CELL * scale)
	return tr


## Ritratto PixelLab 64x64 (atlante portraits.png: colonna classe, riga popolo * 2 + squadra), o null.
static func face_texture(cls: String, team: int, people := "human") -> Texture2D:
	if not ResourceLoader.exists("res://assets/override/portraits.png"):
		return null
	var at := AtlasTexture.new()
	at.atlas = Art.tex("portraits")
	var c := maxi(0, CLASS_ORDER.find(cls))
	var p := maxi(0, PEOPLE_ORDER.find(people))
	at.region = Rect2(c * 64, (p * 2 + clampi(team, 0, 1)) * 64, 64, 64)
	return at
