extends RefCounted
## Accesso all'atlante degli sprite generato da tools/pixelart (units.png).

const Art := preload("res://scripts/art.gd")
static var UNITS_TEX: Texture2D = Art.tex("units")
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


static func region(cls: String, team: int, name: String, frame: String) -> Rect2:
	return Rect2(FRAME[frame] * CELL, row(cls, team, name) * CELL, CELL, CELL)


## Ritratto per l'interfaccia (TextureRect ingrandito senza sfocatura).
static func portrait(cls: String, team: int, name: String, scale := 3) -> TextureRect:
	var at := AtlasTexture.new()
	at.atlas = UNITS_TEX
	at.region = region(cls, team, name, "front_idle0")
	var tr := TextureRect.new()
	tr.texture = at
	tr.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	tr.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	tr.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	tr.custom_minimum_size = Vector2(CELL * scale, CELL * scale)
	return tr
