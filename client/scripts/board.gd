extends Node2D
## Mappa isometrica in pixel-art: caselle e sprite dagli atlanti generati da
## tools/pixelart. Coordinate native (rombo 32x16, 8 px per livello); il nodo
## viene scalato di un fattore intero per mantenere i pixel nitidi.

const UI := preload("res://scripts/ui.gd")
const Sprites := preload("res://scripts/sprites.gd")
const TERRAIN_TEX := preload("res://assets/terrain.png")
const UNITS_TEX := preload("res://assets/units.png")

const TW := 32.0
const TH := 16.0
const HS := 8.0
const CELL := 32
const TERRAIN_ROW := {"Grass": 0, "Stone": 1, "Sand": 2, "Water": 3, "Lava": 4, "Thicket": 5}
const OVERLAY_ROW := 6
const FRAME := {"front_idle0": 0, "front_idle1": 1, "back_idle0": 2, "back_idle1": 3,
	"front_act": 4, "back_act": 5, "ko": 6}

var w := 0
var h := 0
var tiles: Array = []
var units: Array = []
var active := -1
var highlight := {}
var area := {}
var hover := Vector2i(-1, -1)
var floats: Array = []
var time := 0.0
## Animazioni per unità: indice -> {"act": scadenza, "hurt": scadenza}
var anim := {}
var font: Font


func _ready() -> void:
	font = ThemeDB.fallback_font
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST


func set_map(map: Dictionary) -> void:
	w = int(map["width"])
	h = int(map["height"])
	tiles = map["tiles"]
	queue_redraw()


func in_bounds(p: Vector2i) -> bool:
	return p.x >= 0 and p.y >= 0 and p.x < w and p.y < h


func height_at(x: int, y: int) -> int:
	return int(tiles[y * w + x]["height"])


func iso(x: float, y: float, z: float) -> Vector2:
	return Vector2((x - y) * TW / 2.0, (x + y) * TH / 2.0 - z * HS)


func tile_center(p: Vector2i) -> Vector2:
	return iso(p.x, p.y, height_at(p.x, p.y))


func top_poly(x: int, y: int) -> PackedVector2Array:
	var c := iso(x, y, height_at(x, y))
	return PackedVector2Array([
		c + Vector2(0, -TH / 2), c + Vector2(TW / 2, 0),
		c + Vector2(0, TH / 2), c + Vector2(-TW / 2, 0),
	])


## Rettangolo (coordinate native) occupato dalla mappa, sprite compresi.
func extent() -> Rect2:
	var r := Rect2(iso(0, 0, 0), Vector2.ZERO)
	var maxh := 0
	for t in tiles:
		maxh = maxi(maxh, int(t["height"]))
	for p in [iso(0, h - 1, 0), iso(w - 1, 0, 0), iso(w - 1, h - 1, 0)]:
		r = r.expand(p)
	return r.grow_individual(TW / 2, TH / 2 + maxh * HS + 40, TW / 2, TH / 2 + HS)


func pick(local: Vector2) -> Vector2i:
	for s in range(w + h - 2, -1, -1):
		for x in range(w - 1, -1, -1):
			var y := s - x
			if y < 0 or y >= h:
				continue
			if Geometry2D.is_point_in_polygon(local, top_poly(x, y)):
				return Vector2i(x, y)
	return Vector2i(-1, -1)


func add_float(p: Vector2i, text: String, color: Color) -> void:
	if not in_bounds(p):
		return
	var stacked := floats.filter(func(f: Dictionary) -> bool: return f["tile"] == p and f["t"] < 0.8).size()
	floats.append({"tile": p, "pos": tile_center(p) + Vector2(0, -36 - 9 * stacked), "text": text, "color": color, "t": 0.0})


func play(unit: int, kind: String, duration := 0.45) -> void:
	if not anim.has(unit):
		anim[unit] = {}
	anim[unit][kind] = time + duration


func _anim_on(unit: int, kind: String) -> bool:
	return anim.has(unit) and anim[unit].get(kind, 0.0) > time


func _process(delta: float) -> void:
	time += delta
	for f in floats:
		f["t"] += delta
	floats = floats.filter(func(f: Dictionary) -> bool: return f["t"] < 1.4)
	queue_redraw()


# ------------------------------------------------------------------ disegno

func _region(col: int, row: int) -> Rect2:
	return Rect2(col * CELL, row * CELL, CELL, CELL)


func _draw() -> void:
	if w == 0:
		return
	var by_tile := {}
	for u in units:
		if bool(u["active"]) or str(u["condition"]) == "Defeated":
			by_tile[Vector2i(int(u["x"]), int(u["y"]))] = u
	for s in range(w + h - 1):
		for x in range(w):
			var y := s - x
			if y >= 0 and y < h:
				_draw_tile(x, y)
		for x in range(w):
			var y := s - x
			if y >= 0 and y < h and by_tile.has(Vector2i(x, y)):
				_draw_unit(by_tile[Vector2i(x, y)])
	_draw_floats()


func _draw_tile(x: int, y: int) -> void:
	var t: Dictionary = tiles[y * w + x]
	var z := int(t["height"])
	var row: int = TERRAIN_ROW.get(str(t["terrain"]), 0)
	for k in range(0, z + 1):
		var c := iso(x, y, k)
		draw_texture_rect_region(TERRAIN_TEX, Rect2(c - Vector2(16, 8), Vector2(CELL, CELL)), _region(3, row))
	var c := iso(x, y, z)
	var variant := (x * 7 + y * 13) % 3
	if str(t["terrain"]) in ["Water", "Lava"]:
		variant = (int(time * 2.0) + x + y) % 3
	var dst := Rect2(c - Vector2(16, 24), Vector2(CELL, CELL))
	draw_texture_rect_region(TERRAIN_TEX, dst, _region(variant, row))
	var v := Vector2i(x, y)
	if highlight.has(v):
		draw_texture_rect_region(TERRAIN_TEX, dst, _region(0, OVERLAY_ROW), highlight[v])
	if area.has(v):
		draw_texture_rect_region(TERRAIN_TEX, dst, _region(0, OVERLAY_ROW), Color(1, 0.62, 0.15, 0.9))
	if v == hover:
		var pulse := 0.75 + 0.25 * sin(time * 6.0)
		draw_texture_rect_region(TERRAIN_TEX, dst, _region(1, OVERLAY_ROW), Color(1, 1, 1, pulse))


func _unit_row(u: Dictionary) -> int:
	return Sprites.row(str(u["class"]), int(u["team"]), str(u["name"]))


func _draw_unit(u: Dictionary) -> void:
	var x := int(u["x"])
	var y := int(u["y"])
	var idx := int(u["index"])
	var c := iso(x, y, height_at(x, y))
	draw_texture_rect_region(TERRAIN_TEX, Rect2(c - Vector2(16, 24), Vector2(CELL, CELL)), _region(3, OVERLAY_ROW))
	var ko := str(u["condition"]) == "Defeated"
	var facing := str(u["facing"])
	var back := facing == "N" or facing == "W"
	var flip := facing == "S" or facing == "W"
	var frame: String
	if ko:
		frame = "ko"
		flip = false
	elif _anim_on(idx, "act"):
		frame = "back_act" if back else "front_act"
	else:
		var bob := int(time * 2.2 + idx * 0.37) % 2
		frame = ("back_idle" if back else "front_idle") + str(bob)
	var src := Rect2(FRAME[frame] * CELL, _unit_row(u) * CELL, CELL, CELL)
	var pos := c - Vector2(16, 29)
	if _anim_on(idx, "hurt"):
		pos.x += 1 if int(time * 30) % 2 == 0 else -1
	var mod := Color(1, 1, 1, 0.65 if ko else 1.0)
	if _anim_on(idx, "hurt"):
		mod = Color(1, 0.45, 0.45)
	if flip:
		draw_set_transform(pos + Vector2(CELL, 0), 0.0, Vector2(-1, 1))
		draw_texture_rect_region(UNITS_TEX, Rect2(Vector2.ZERO, Vector2(CELL, CELL)), src, mod)
		draw_set_transform(Vector2.ZERO, 0.0, Vector2.ONE)
	else:
		draw_texture_rect_region(UNITS_TEX, Rect2(pos, Vector2(CELL, CELL)), src, mod)
	if ko:
		return
	# Barra degli HP e indicatori.
	var ratio := clampf(float(u["hp"]) / maxf(1.0, float(u["max_hp"])), 0.0, 1.0)
	var bar := Rect2(c + Vector2(-8, -34), Vector2(16, 3))
	draw_rect(bar, Color(0.08, 0.06, 0.12))
	var hp_col := UI.GOOD if ratio > 0.5 else (UI.ACCENT if ratio > 0.25 else UI.BAD)
	draw_rect(Rect2(bar.position + Vector2(1, 1), Vector2(roundf(14 * ratio), 1)), hp_col)
	var team_col: Color = UI.TEAM_COLORS[int(u["team"])]
	draw_rect(Rect2(bar.position + Vector2(-3, 0), Vector2(2, 3)), team_col)
	if idx == active:
		var bob := roundf(sin(time * 4.0) * 1.5)
		draw_texture_rect_region(TERRAIN_TEX, Rect2(c - Vector2(16, 70 - bob), Vector2(CELL, CELL)), _region(2, OVERLAY_ROW))
	if int(u["suspended"]) > 0:
		draw_rect(Rect2(c + Vector2(9, -34), Vector2(3, 3)), UI.ACCENT)


func _draw_floats() -> void:
	# Testo a risoluzione di schermo: si annulla la scala del nodo.
	var s := scale.x if scale.x > 0 else 1.0
	for f in floats:
		var a := clampf(1.4 - float(f["t"]), 0.0, 1.0)
		var col: Color = f["color"]
		col.a = a
		var at: Vector2 = f["pos"] + Vector2(-8, -float(f["t"]) * 14)
		draw_set_transform(at, 0.0, Vector2(1.0 / s, 1.0 / s))
		draw_string_outline(font, Vector2.ZERO, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 18, 5, Color(0.05, 0.03, 0.1, a))
		draw_string(font, Vector2.ZERO, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 18, col)
	draw_set_transform(Vector2.ZERO, 0.0, Vector2.ONE)
