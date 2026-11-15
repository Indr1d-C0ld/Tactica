extends Node2D
## Mappa isometrica disegnata da codice: caselle con altezza, unità, evidenziazioni.
## Grafica segnaposto originale (forme e colori), in attesa degli asset definitivi.

const UI := preload("res://scripts/ui.gd")

const TW := 64.0
const TH := 32.0
const HS := 14.0
const TERRAIN := {
	"Grass": Color(0.37, 0.6, 0.3),
	"Stone": Color(0.55, 0.55, 0.58),
	"Sand": Color(0.84, 0.75, 0.48),
	"Water": Color(0.25, 0.5, 0.78),
	"Lava": Color(0.85, 0.33, 0.12),
	"Thicket": Color(0.22, 0.42, 0.2),
}
const CLASS_LETTER := {
	"vanguard": "A", "scout": "E", "arcanist": "R",
	"mender": "G", "runeblade": "S", "warden": "C",
}
const DIRS := {"N": Vector2i(0, -1), "E": Vector2i(1, 0), "S": Vector2i(0, 1), "W": Vector2i(-1, 0)}

var w := 0
var h := 0
var tiles: Array = []
var units: Array = []
var active := -1
var highlight := {}
var area := {}
var hover := Vector2i(-1, -1)
var floats: Array = []
var font: Font


func _ready() -> void:
	font = ThemeDB.fallback_font


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


## Rettangolo (in coordinate locali) occupato dalla mappa.
func extent() -> Rect2:
	var r := Rect2(iso(0, 0, 0), Vector2.ZERO)
	for p in [iso(0, h - 1, 0), iso(w - 1, 0, 0), iso(w - 1, h - 1, 0)]:
		r = r.expand(p)
	r = r.grow_individual(TW / 2, TH / 2 + 10 * HS, TW / 2, TH / 2 + HS)
	return r


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
	floats.append({"tile": p, "pos": tile_center(p) + Vector2(0, -18 * stacked), "text": text, "color": color, "t": 0.0})
	queue_redraw()


func _process(delta: float) -> void:
	if floats.is_empty():
		return
	for f in floats:
		f["t"] += delta
	floats = floats.filter(func(f: Dictionary) -> bool: return f["t"] < 1.4)
	queue_redraw()


func _draw() -> void:
	if w == 0:
		return
	for s in range(w + h - 1):
		for x in range(w):
			var y := s - x
			if y >= 0 and y < h:
				_draw_tile(x, y)
	var alive := units.filter(func(u: Dictionary) -> bool: return u["active"])
	alive.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
		return int(a["x"]) + int(a["y"]) < int(b["x"]) + int(b["y"]))
	for u in alive:
		_draw_unit(u)
	for f in floats:
		var a := clampf(1.4 - float(f["t"]), 0.0, 1.0)
		var col: Color = f["color"]
		col.a = a
		var at: Vector2 = f["pos"] + Vector2(-24, -44 - float(f["t"]) * 28)
		draw_string_outline(font, at, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 17, 4, Color(0, 0, 0, a))
		draw_string(font, at, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 17, col)


func _draw_tile(x: int, y: int) -> void:
	var t: Dictionary = tiles[y * w + x]
	var z := int(t["height"])
	var col: Color = TERRAIN.get(str(t["terrain"]), Color.MAGENTA)
	# Variazione leggera per casella, deterministica.
	col = col.lightened(float((x * 7 + y * 13) % 5) * 0.015)
	var c := iso(x, y, z)
	var drop := Vector2(0, (z + 0.6) * HS)
	var l := c + Vector2(-TW / 2, 0)
	var b := c + Vector2(0, TH / 2)
	var r := c + Vector2(TW / 2, 0)
	draw_colored_polygon(PackedVector2Array([l, b, b + drop, l + drop]), col.darkened(0.35))
	draw_colored_polygon(PackedVector2Array([b, r, r + drop, b + drop]), col.darkened(0.5))
	var top := top_poly(x, y)
	draw_colored_polygon(top, col)
	if str(t["terrain"]) == "Water" or str(t["terrain"]) == "Lava":
		draw_line(c + Vector2(-12, -2), c + Vector2(6, -2), col.lightened(0.3), 1.5)
		draw_line(c + Vector2(-4, 5), c + Vector2(12, 5), col.lightened(0.3), 1.5)
	elif str(t["terrain"]) == "Thicket":
		for o in [Vector2(-10, 0), Vector2(6, -4), Vector2(2, 6)]:
			draw_circle(c + o, 4, col.darkened(0.25))
	var closed := top.duplicate()
	closed.append(top[0])
	draw_polyline(closed, col.darkened(0.3), 1.0)
	var v := Vector2i(x, y)
	if highlight.has(v):
		draw_colored_polygon(top, highlight[v])
	if area.has(v):
		draw_colored_polygon(top, Color(1, 0.6, 0.1, 0.45))
	if v == hover:
		draw_polyline(closed, Color.WHITE, 2.0)


func _draw_unit(u: Dictionary) -> void:
	var x := int(u["x"])
	var y := int(u["y"])
	var c := iso(x, y, height_at(x, y))
	var team := int(u["team"])
	var col: Color = UI.TEAM_COLORS[team]
	var shadow := PackedVector2Array()
	for i in 16:
		var a := TAU * i / 16.0
		shadow.append(c + Vector2(cos(a) * 15, sin(a) * 7))
	draw_colored_polygon(shadow, Color(0, 0, 0, 0.35))
	var body := c + Vector2(0, -16)
	draw_rect(Rect2(body + Vector2(-9, 0), Vector2(18, 14)), col.darkened(0.35))
	draw_circle(body, 13, col.darkened(0.3))
	draw_circle(body, 11, col)
	if int(u["index"]) == active:
		draw_arc(body, 17, 0, TAU, 32, UI.ACCENT, 2.5)
	var letter: String = CLASS_LETTER.get(str(u["class"]), "?")
	draw_string(font, body + Vector2(-5, 5), letter, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color.WHITE)
	var d: Vector2i = DIRS.get(str(u["facing"]), Vector2i(1, 0))
	var dv := iso(d.x, d.y, 0).normalized()
	var tip := body + dv * 19
	var side := Vector2(-dv.y, dv.x) * 5
	draw_colored_polygon(PackedVector2Array([tip, body + dv * 12 + side, body + dv * 12 - side]), Color.WHITE)
	var ratio := clampf(float(u["hp"]) / maxf(1.0, float(u["max_hp"])), 0.0, 1.0)
	var bar := Rect2(body + Vector2(-16, -24), Vector2(32, 4))
	draw_rect(bar, Color(0, 0, 0, 0.7))
	var hp_col := UI.GOOD if ratio > 0.5 else (UI.ACCENT if ratio > 0.25 else UI.BAD)
	draw_rect(Rect2(bar.position, Vector2(32 * ratio, 4)), hp_col)
	if int(u["suspended"]) > 0:
		draw_string(font, body + Vector2(12, -14), "sosp.", HORIZONTAL_ALIGNMENT_LEFT, -1, 11, UI.ACCENT)
