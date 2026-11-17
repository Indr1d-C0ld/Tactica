extends Control
## Grafico semplice in pixel-art: linea (andamento ELO) o barre (partite al giorno).

var values: Array = []
## Etichette sotto l'asse (stesso numero dei valori, o vuoto).
var labels: Array = []
var bars := false
var color := Color(0.95, 0.78, 0.35)
var title := ""


func _init(vals: Array = [], as_bars := false, col := Color(0.95, 0.78, 0.35), heading := "") -> void:
	values = vals
	bars = as_bars
	color = col
	title = heading
	custom_minimum_size = Vector2(520, 170)
	size_flags_horizontal = Control.SIZE_EXPAND_FILL


func _draw() -> void:
	var font := get_theme_default_font()
	var fs := 22
	var area := Rect2(Vector2(70, 30), size - Vector2(84, 58))
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.06, 0.07, 0.11, 0.85))
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.95, 0.78, 0.35, 0.5), false, 2.0)
	if title != "":
		draw_string(font, Vector2(10, 22), title, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color(0.85, 0.87, 0.92))
	if values.is_empty():
		draw_string(font, area.position + Vector2(0, area.size.y / 2), "nessun dato", HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color(0.6, 0.62, 0.7))
		return
	var lo := float(values.min())
	var hi := float(values.max())
	if bars:
		lo = 0.0
	if bars:
		# Scala minima di 4, così le etichette (0, metà, massimo) restano distinte.
		hi = maxf(hi, 4.0)
	elif hi - lo < 4.0:
		# Valori quasi uguali: un margine attorno.
		var mid := (hi + lo) / 2.0
		lo = mid - 10.0
		hi = mid + 10.0
	# Griglia orizzontale e valori estremi.
	for k in 3:
		var y := area.position.y + area.size.y * k / 2.0
		draw_line(Vector2(area.position.x, y), Vector2(area.end.x, y), Color(1, 1, 1, 0.08), 1.0)
		var v := hi - (hi - lo) * k / 2.0
		draw_string(font, Vector2(6, y + 7), str(int(round(v))), HORIZONTAL_ALIGNMENT_LEFT, 60, fs, Color(0.6, 0.62, 0.7))
	var n := values.size()
	var pts := PackedVector2Array()
	for i in n:
		var t := float(values[i])
		var x: float = area.position.x + (area.size.x * (i + 0.5) / n if bars else (area.size.x * i / maxf(1.0, n - 1.0)))
		var y := area.end.y - (t - lo) / (hi - lo) * area.size.y
		if bars:
			var w := maxf(2.0, area.size.x / n - 3.0)
			draw_rect(Rect2(x - w / 2.0, y, w, area.end.y - y), color)
		else:
			pts.append(Vector2(x, y))
	if not bars:
		if pts.size() == 1:
			draw_circle(pts[0], 4.0, color)
		else:
			draw_polyline(pts, color, 3.0)
			draw_circle(pts[pts.size() - 1], 4.0, color)
	# Etichette: la prima, quella di mezzo e l'ultima.
	if labels.size() == n and n > 0:
		for i: int in [0, n / 2, n - 1]:
			var x: float = area.position.x + (area.size.x * (i + 0.5) / n if bars else area.size.x * i / maxf(1.0, n - 1.0))
			draw_string(font, Vector2(x - 30, size.y - 6), str(labels[i]), HORIZONTAL_ALIGNMENT_LEFT, 160, fs, Color(0.6, 0.62, 0.7))
