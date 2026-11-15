extends Node3D
## Mappa 3D ruotabile: colonne di caselle con texture pixel-art (atlante
## terrain3d.png), personaggi come sprite verticali che cambiano vista in base
## all'angolo della telecamera, telecamera ortogonale orbitale con zoom limitato.
## Pensata per essere renderizzata a bassa risoluzione (SubViewport) e
## ingrandita con filtro VGA.

const UI := preload("res://scripts/ui.gd")
const Sprites := preload("res://scripts/sprites.gd")
const Art := preload("res://scripts/art.gd")
var TEX: Texture2D = Art.tex("terrain3d")
var OVERLAY_TEX: Texture2D = Art.tex("terrain")
const ANIM_SHADER := preload("res://shaders/animated_tile.gdshader")
var VFX_TEX: Texture2D = Art.tex("vfx")
const VFX_ROWS := ["slash", "arrow", "impact", "fire", "ice", "thunder", "holy", "heal", "buff", "debuff", "poison", "drain"]
const VFX_LEN := {"slash": 4, "arrow": 1, "impact": 4, "fire": 6, "ice": 5, "thunder": 4, "holy": 5, "heal": 6,
	"buff": 5, "debuff": 5, "poison": 6, "drain": 5}

const C := 16.0
const LEVEL := 0.5
const PX := 0.05  # dimensione di un pixel degli sprite in unità di mondo
const TERRAIN_ROW := {"Grass": 0, "Stone": 1, "Sand": 2, "Water": 3, "Lava": 4, "Thicket": 5}
const DIR_VEC := {"N": Vector3(0, 0, -1), "E": Vector3(1, 0, 0), "S": Vector3(0, 0, 1), "W": Vector3(-1, 0, 0)}
const FACE_SHADE := {Vector2i(1, 0): 0.8, Vector2i(0, 1): 0.66, Vector2i(-1, 0): 0.9, Vector2i(0, -1): 0.74}

const STATUS_COLORS := {
	"poison": Color(0.45, 0.85, 0.3), "sleep": Color(0.55, 0.65, 1.0), "silence": Color(0.95, 0.95, 0.95),
	"slow": Color(0.7, 0.5, 0.3), "root": Color(0.25, 0.55, 0.25), "haste": Color(1.0, 0.85, 0.2),
	"aegis": Color(0.4, 0.9, 1.0), "regen": Color(1.0, 0.55, 0.75),
}
const PITCH_MIN := 20.0
const PITCH_MAX := 70.0
const ZOOM_MIN := 5.0

var w := 0
var h := 0
var tiles: Array = []
var active := -1
## Indice del campione nemico (missioni di caccia), -1 se assente.
var champion := -1
var highlight := {}
var area := {}
var hover := Vector2i(-1, -1)
var time := 0.0
var anim := {}
var floats: Array = []
var _fx: Array = []

var yaw := 45.0
var yaw_target := 45.0
var pitch := 34.0
var zoom := 12.0
var zoom_target := 12.0
var pan := Vector3.ZERO
var zoom_fit := 12.0
var zoom_max := 18.0

var cam: Camera3D
var _terrain: MeshInstance3D
var _animated: MeshInstance3D
var _overlay: MeshInstance3D
var _overlay_mesh := ImmediateMesh.new()
var _overlay_mat := StandardMaterial3D.new()
var _decor := Node3D.new()
var _arrow: Sprite3D
var _nodes := {}  # indice unità -> {sprite, shadow, pos}
var _shadow_mesh: QuadMesh
var _shadow_mat := StandardMaterial3D.new()

var units: Array = []:
	set(value):
		units = value
		_sync_units()


func _ready() -> void:
	cam = Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = zoom
	cam.near = 0.1
	cam.far = 300.0
	var env := Environment.new()
	# Sfondo trasparente: dietro la scena c'è il fondale in pixel-art.
	env.background_mode = Environment.BG_CLEAR_COLOR
	cam.environment = env
	add_child(cam)

	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.vertex_color_use_as_albedo = true
	mat.albedo_texture = TEX
	mat.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	_terrain = MeshInstance3D.new()
	_terrain.material_override = mat
	add_child(_terrain)

	var amat := ShaderMaterial.new()
	amat.shader = ANIM_SHADER
	amat.set_shader_parameter("atlas", TEX)
	amat.set_shader_parameter("cell_u", C / TEX.get_width())
	_animated = MeshInstance3D.new()
	_animated.material_override = amat
	add_child(_animated)

	_overlay_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_overlay_mat.vertex_color_use_as_albedo = true
	_overlay_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_overlay_mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	_overlay_mat.render_priority = 1
	_overlay = MeshInstance3D.new()
	_overlay.mesh = _overlay_mesh
	_overlay.material_override = _overlay_mat
	add_child(_overlay)
	add_child(_decor)

	_shadow_mesh = QuadMesh.new()
	_shadow_mesh.size = Vector2(0.7, 0.42)
	_shadow_mesh.orientation = PlaneMesh.FACE_Y
	_shadow_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_shadow_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_shadow_mat.albedo_color = Color(0.03, 0.02, 0.08, 0.45)

	_arrow = _sprite(OVERLAY_TEX, Rect2(64, 192, 32, 32))
	_arrow.visible = false
	add_child(_arrow)


func _sprite(tex: Texture2D, region: Rect2) -> Sprite3D:
	var s := Sprite3D.new()
	s.texture = tex
	s.region_enabled = true
	s.region_rect = region
	s.pixel_size = PX
	s.billboard = BaseMaterial3D.BILLBOARD_FIXED_Y
	s.shaded = false
	s.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	s.alpha_cut = SpriteBase3D.ALPHA_CUT_DISCARD
	return s


# ------------------------------------------------------------------ mappa

func set_map(map: Dictionary) -> void:
	w = int(map["width"])
	h = int(map["height"])
	tiles = map["tiles"]
	_build_terrain()
	# Inquadratura iniziale: tutta la mappa; lo zoom indietro non va oltre.
	zoom_fit = (w + h) * 0.62
	zoom_max = zoom_fit * 1.3
	zoom = zoom_fit
	zoom_target = zoom_fit
	_place_camera()
	_sync_units()


func in_bounds(p: Vector2i) -> bool:
	return p.x >= 0 and p.y >= 0 and p.x < w and p.y < h


func height_at(x: int, y: int) -> int:
	return int(tiles[y * w + x]["height"])


func top_y(x: int, y: int) -> float:
	return (height_at(x, y) + 1) * LEVEL


func tile_top(p: Vector2i) -> Vector3:
	return Vector3(p.x + 0.5, top_y(p.x, p.y), p.y + 0.5)


func _quad(st: SurfaceTool, v: Array, uv0: Vector2, uv1: Vector2, col: Color) -> void:
	var uvs := [uv0, Vector2(uv1.x, uv0.y), uv1, Vector2(uv0.x, uv1.y)]
	for i in [0, 1, 2, 0, 2, 3]:
		st.set_color(col)
		st.set_uv(uvs[i])
		st.add_vertex(v[i])


func _build_terrain() -> void:
	var tw := float(TEX.get_width())
	var th := float(TEX.get_height())
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var sa := SurfaceTool.new()
	sa.begin(Mesh.PRIMITIVE_TRIANGLES)
	var animated_count := 0
	for c in _decor.get_children():
		c.queue_free()
	for y in h:
		for x in w:
			var t: Dictionary = tiles[y * w + x]
			var terrain := str(t["terrain"])
			var row: int = TERRAIN_ROW.get(terrain, 0)
			var top := top_y(x, y)
			var animated := terrain == "Water" or terrain == "Lava"
			var variant := 0 if animated else (x * 7 + y * 13) % 3
			var uv0 := Vector2(variant * C, row * C) / Vector2(tw, th)
			var uv1 := uv0 + Vector2(C, C) / Vector2(tw, th)
			var corners := [Vector3(x, top, y), Vector3(x + 1, top, y), Vector3(x + 1, top, y + 1), Vector3(x, top, y + 1)]
			if animated:
				# L'alfa del colore sfasa l'animazione fra caselle vicine.
				_quad(sa, corners, uv0, uv1, Color(1, 1, 1, float((x + y) % 3) / 3.0))
				animated_count += 1
			else:
				_quad(st, corners, uv0, uv1, Color.WHITE)
			for d in FACE_SHADE:
				var n: Vector2i = Vector2i(x, y) + d
				var ntop := top_y(n.x, n.y) if in_bounds(n) else 0.0
				if ntop >= top:
					continue
				var shade: float = FACE_SHADE[d]
				var yb := top
				while yb > ntop + 0.001:
					var ya := maxf(yb - LEVEL, ntop)
					# Il segmento più alto usa la variante col bordo della sommità che ricade.
					var col := 4 if yb == top else 3
					var su0 := Vector2(col * C, row * C) / Vector2(tw, th)
					var su1 := su0 + Vector2(C, 8.0 * (yb - ya) / LEVEL) / Vector2(tw, th)
					_quad(st, _side(x, y, d, ya, yb), su0, su1, Color(shade, shade, shade))
					yb = ya
			_decorate(x, y, terrain, top)
	_terrain.mesh = st.commit()
	_animated.mesh = sa.commit() if animated_count > 0 else null


func _side(x: int, y: int, d: Vector2i, ya: float, yb: float) -> Array:
	match d:
		Vector2i(1, 0):
			return [Vector3(x + 1, yb, y), Vector3(x + 1, yb, y + 1), Vector3(x + 1, ya, y + 1), Vector3(x + 1, ya, y)]
		Vector2i(-1, 0):
			return [Vector3(x, yb, y + 1), Vector3(x, yb, y), Vector3(x, ya, y), Vector3(x, ya, y + 1)]
		Vector2i(0, 1):
			return [Vector3(x + 1, yb, y + 1), Vector3(x, yb, y + 1), Vector3(x, ya, y + 1), Vector3(x + 1, ya, y + 1)]
		_:
			return [Vector3(x, yb, y), Vector3(x + 1, yb, y), Vector3(x + 1, ya, y), Vector3(x, ya, y)]


func _decorate(x: int, y: int, terrain: String, top: float) -> void:
	var hsh := (x * 92821 + y * 68917) % 97
	var cells := []
	if terrain == "Thicket":
		cells = [[hsh % 2, Vector2(0.28, 0.3)], [(hsh + 1) % 2, Vector2(0.7, 0.68)]]
	elif terrain == "Stone" and hsh % 4 == 0:
		cells = [[2, Vector2(0.65, 0.4)]]
	elif terrain == "Grass" and hsh % 3 == 0:
		cells = [[3, Vector2(0.3 + (hsh % 5) * 0.1, 0.35 + (hsh % 7) * 0.05)]]
	for c in cells:
		var s := _sprite(TEX, Rect2(int(c[0]) * C, 6 * C, C, C))
		var off: Vector2 = c[1]
		s.position = Vector3(x + off.x, top + C * PX / 2.0 - PX, y + off.y)
		_decor.add_child(s)


# ------------------------------------------------------------------ telecamera

func center() -> Vector3:
	var maxh := 0
	for t in tiles:
		maxh = maxi(maxh, int(t["height"]))
	return Vector3(w / 2.0, maxh * LEVEL * 0.5, h / 2.0)


func _place_camera() -> void:
	if cam == null:
		return
	var yr := deg_to_rad(yaw)
	var pr := deg_to_rad(pitch)
	var dir := Vector3(cos(pr) * sin(yr), sin(pr), cos(pr) * cos(yr))
	var target := center() + pan
	cam.position = target + dir * 60.0
	cam.look_at(target, Vector3.UP)
	cam.size = zoom


## Rotazione a scatti di 45° (tasti) oppure libera (trascinamento).
func rotate_by(deg: float, smooth := true) -> void:
	yaw_target += deg
	if not smooth:
		yaw = yaw_target


func tilt_by(deg: float) -> void:
	pitch = clampf(pitch + deg, PITCH_MIN, PITCH_MAX)


func zoom_by(steps: float) -> void:
	zoom_target = clampf(zoom_target * pow(0.85, steps), ZOOM_MIN, zoom_max)


func pan_by(screen_delta: Vector2) -> void:
	var right := cam.global_transform.basis.x
	var fwd := -cam.global_transform.basis.z
	right.y = 0
	fwd.y = 0
	var k := zoom / 240.0
	pan += (-right.normalized() * screen_delta.x + fwd.normalized() * screen_delta.y) * k
	pan.x = clampf(pan.x, -w * 0.6, w * 0.6)
	pan.z = clampf(pan.z, -h * 0.6, h * 0.6)


func reset_view() -> void:
	yaw_target = 45.0
	pitch = 34.0
	zoom_target = zoom_fit
	pan = Vector3.ZERO


# ------------------------------------------------------------------ selezione

## Casella sotto il punto (coordinate della SubViewport), con raggio contro le colonne.
func pick(vp_pos: Vector2) -> Vector2i:
	if cam == null or w == 0:
		return Vector2i(-1, -1)
	var o := cam.project_ray_origin(vp_pos)
	var n := cam.project_ray_normal(vp_pos)
	var best := INF
	var hit := Vector2i(-1, -1)
	for y in h:
		for x in w:
			var t := _ray_box(o, n, Vector3(x, 0, y), Vector3(x + 1, top_y(x, y), y + 1))
			if t < best:
				best = t
				hit = Vector2i(x, y)
	return hit


func _ray_box(o: Vector3, n: Vector3, lo: Vector3, hi: Vector3) -> float:
	var tmin := -INF
	var tmax := INF
	for a in 3:
		if absf(n[a]) < 1e-9:
			if o[a] < lo[a] or o[a] > hi[a]:
				return INF
			continue
		var t1 := (lo[a] - o[a]) / n[a]
		var t2 := (hi[a] - o[a]) / n[a]
		tmin = maxf(tmin, minf(t1, t2))
		tmax = minf(tmax, maxf(t1, t2))
	if tmax < maxf(tmin, 0.0):
		return INF
	return tmin


func screen_of(world: Vector3) -> Vector2:
	return cam.unproject_position(world)


# ------------------------------------------------------------------ unità

func _sync_units() -> void:
	if not is_inside_tree() or tiles.is_empty():
		return
	var seen := {}
	for u in units:
		var i := int(u["index"])
		var visible_unit := bool(u["active"]) or str(u["condition"]) == "Defeated"
		seen[i] = true
		if not _nodes.has(i):
			var s := _sprite(Sprites.UNITS_TEX, Rect2(0, 0, 32, 32))
			var sh := MeshInstance3D.new()
			sh.mesh = _shadow_mesh
			sh.material_override = _shadow_mat
			add_child(s)
			add_child(sh)
			var start := tile_top(Vector2i(int(u["x"]), int(u["y"])))
			_nodes[i] = {"sprite": s, "shadow": sh, "pos": start}
		var node: Dictionary = _nodes[i]
		var tile := Vector2i(int(u["x"]), int(u["y"]))
		if node.has("tile") and node["tile"] != tile and bool(u["active"]):
			# Nuova casella: si cammina lungo il percorso, casella per casella.
			var jump := int(u["stats"]["jump"]) if u.has("stats") else 3
			var waypoints: Array = []
			for p in _grid_path(node["tile"], tile, jump):
				waypoints.append(tile_top(p))
			node["path"] = waypoints
			node["seg_from"] = node["pos"]
			node["seg_t"] = 0.0
		node["tile"] = tile
		node["target"] = tile_top(tile)
		node["unit"] = u
		(node["sprite"] as Sprite3D).visible = visible_unit
		(node["shadow"] as MeshInstance3D).visible = visible_unit and bool(u["active"])
	for i in _nodes.keys():
		if not seen.has(i):
			(_nodes[i]["sprite"] as Node).queue_free()
			(_nodes[i]["shadow"] as Node).queue_free()
			_nodes.erase(i)


# ------------------------------------------------------------------ effetti

func _fx_sprite(kind: String) -> Sprite3D:
	var s := _sprite(VFX_TEX, Rect2(0, VFX_ROWS.find(kind) * 32, 32, 32))
	s.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	s.render_priority = 2
	s.no_depth_test = true
	add_child(s)
	return s


## Effetto animato su una casella (`delay` in secondi).
func effect(kind: String, tile: Vector2i, delay := 0.0) -> void:
	if not in_bounds(tile) or not (kind in VFX_ROWS):
		return
	var s := _fx_sprite(kind)
	s.visible = false
	s.position = tile_top(tile) + Vector3(0, 0.75, 0)
	_fx.append({"sprite": s, "kind": kind, "t": -delay, "fps": 12.0})


## Abilità: proiettile da chi agisce (se a distanza) ed effetto su bersaglio e area.
func cast(user: int, fx: String, projectile: bool, target: Vector2i, aoe: int) -> void:
	var delay := 0.0
	if projectile and _nodes.has(user):
		var from: Vector3 = (_nodes[user]["pos"] as Vector3) + Vector3(0, 0.9, 0)
		var to := tile_top(target) + Vector3(0, 0.75, 0)
		var s := _fx_sprite("arrow")
		s.position = from
		var screen_dx := screen_of(to).x - screen_of(from).x
		s.flip_h = screen_dx < 0
		_fx.append({"sprite": s, "kind": "arrow", "t": 0.0, "from": from, "to": to, "dur": 0.28})
		delay = 0.28
	for y in range(target.y - aoe, target.y + aoe + 1):
		for x in range(target.x - aoe, target.x + aoe + 1):
			var p := Vector2i(x, y)
			if absi(p.x - target.x) + absi(p.y - target.y) <= aoe:
				effect(fx, p, delay + 0.05 * (absi(p.x - target.x) + absi(p.y - target.y)))


func _update_fx(delta: float) -> void:
	for e in _fx:
		e["t"] += delta
		var s: Sprite3D = e["sprite"]
		if e["kind"] == "arrow":
			var k := clampf(float(e["t"]) / float(e["dur"]), 0.0, 1.0)
			var from: Vector3 = e["from"]
			var to: Vector3 = e["to"]
			s.position = from.lerp(to, k) + Vector3(0, sin(PI * k) * 0.6, 0)
			e["done"] = k >= 1.0
			continue
		if float(e["t"]) < 0.0:
			continue
		s.visible = true
		var f := int(float(e["t"]) * float(e["fps"]))
		var n: int = VFX_LEN[e["kind"]]
		if f >= n:
			e["done"] = true
			continue
		s.region_rect = Rect2(f * 32, VFX_ROWS.find(e["kind"]) * 32, 32, 32)
	for e in _fx:
		if e.get("done", false):
			(e["sprite"] as Node).queue_free()
	_fx = _fx.filter(func(e: Dictionary) -> bool: return not e.get("done", false))


## Percorso a passi di una casella (BFS sull'altezza superabile); se non c'è, salto diretto.
func _grid_path(a: Vector2i, b: Vector2i, jump: int) -> Array[Vector2i]:
	var prev := {a: a}
	var queue: Array[Vector2i] = [a]
	while not queue.is_empty():
		var p: Vector2i = queue.pop_front()
		if p == b:
			break
		for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
			var n: Vector2i = p + d
			if in_bounds(n) and not prev.has(n) and absi(height_at(n.x, n.y) - height_at(p.x, p.y)) <= jump:
				prev[n] = p
				queue.append(n)
	if not prev.has(b):
		return [b]
	var out: Array[Vector2i] = []
	var c := b
	while c != a:
		out.push_front(c)
		c = prev[c]
	return out


func play(unit: int, kind: String, duration := 0.45) -> void:
	if not anim.has(unit):
		anim[unit] = {}
	anim[unit][kind] = time + duration


func _anim_on(unit: int, kind: String) -> bool:
	return anim.has(unit) and anim[unit].get(kind, 0.0) > time


func add_float(p: Vector2i, text: String, color: Color) -> void:
	if not in_bounds(p):
		return
	var stacked := floats.filter(func(f: Dictionary) -> bool: return f["tile"] == p and f["t"] < 0.8).size()
	floats.append({"tile": p, "world": tile_top(p) + Vector3(0, 1.9 + 0.35 * stacked, 0), "text": text, "color": color, "t": 0.0})


func _process(delta: float) -> void:
	time += delta
	yaw = lerpf(yaw, yaw_target, minf(1.0, delta * 10.0))
	zoom = lerpf(zoom, zoom_target, minf(1.0, delta * 10.0))
	_place_camera()
	var right := cam.global_transform.basis.x
	var toward := cam.global_transform.basis.z
	right.y = 0
	toward.y = 0
	for i in _nodes:
		var node: Dictionary = _nodes[i]
		var u: Dictionary = node["unit"]
		var target: Vector3 = node["target"]
		var pos: Vector3 = node["pos"]
		var walking := false
		var walk_dir := Vector3.ZERO
		var path: Array = node.get("path", [])
		if not path.is_empty():
			walking = true
			var from: Vector3 = node["seg_from"]
			var to: Vector3 = path[0]
			walk_dir = Vector3(to.x - from.x, 0, to.z - from.z).normalized()
			var t: float = float(node["seg_t"]) + delta * 5.0
			if t >= 1.0:
				pos = to
				path.pop_front()
				node["seg_from"] = to
				node["seg_t"] = 0.0
			else:
				pos = from.lerp(to, t)
				# Piccolo salto, più alto se si cambia livello.
				pos.y = lerpf(from.y, to.y, t) + sin(PI * t) * (0.12 + 0.6 * absf(to.y - from.y))
				node["seg_t"] = t
		else:
			pos = pos.move_toward(target, delta * 6.0)
		node["pos"] = pos
		var s: Sprite3D = node["sprite"]
		var ko := str(u["condition"]) == "Defeated"
		var d: Vector3 = walk_dir if walking else DIR_VEC.get(str(u["facing"]), Vector3(1, 0, 0))
		var front := d.dot(toward) >= 0.0
		var frame: String
		if ko:
			frame = "ko"
		elif _anim_on(i, "act"):
			frame = "front_act" if front else "back_act"
		elif walking:
			frame = ("front_walk" if front else "back_walk") + str(int(time * 8.0) % 2)
		else:
			frame = ("front_idle" if front else "back_idle") + str(int(time * 2.2 + i * 0.37) % 2)
		s.region_rect = Sprites.region(str(u["class"]), int(u["team"]), str(u["name"]), frame)
		s.flip_h = (not ko) and d.dot(right) < 0.0
		var shake := Vector3.ZERO
		if _anim_on(i, "hurt"):
			shake = right.normalized() * (PX if int(time * 30) % 2 == 0 else -PX)
			s.modulate = Color(1, 0.45, 0.45)
		else:
			s.modulate = Color(1, 1, 1, 0.7 if ko else 1.0)
		s.position = pos + Vector3(0, 32 * PX / 2.0 - 2 * PX, 0) + shake
		(node["shadow"] as MeshInstance3D).position = pos + Vector3(0, 0.01, 0)
	if active >= 0 and _nodes.has(active) and bool(_nodes[active]["unit"]["active"]):
		_arrow.visible = true
		_arrow.position = (_nodes[active]["pos"] as Vector3) + Vector3(0, 2.15 + sin(time * 4.0) * 0.06, 0)
	else:
		_arrow.visible = false
	for f in floats:
		f["t"] += delta
	floats = floats.filter(func(f: Dictionary) -> bool: return f["t"] < 1.4)
	_update_fx(delta)
	_draw_overlay()


func _draw_overlay() -> void:
	_overlay_mesh.clear_surfaces()
	if w == 0:
		return
	_overlay_mesh.surface_begin(Mesh.PRIMITIVE_TRIANGLES)
	var any := false
	for p in highlight:
		_tile_quad(p, highlight[p] * Color(1, 1, 1, 0.55), 0.0)
		any = true
	for p in area:
		_tile_quad(p, Color(1, 0.62, 0.15, 0.55), 0.0)
		any = true
	if in_bounds(hover):
		var a := 0.6 + 0.3 * sin(time * 6.0)
		_tile_border(hover, Color(1, 0.97, 0.8, a))
		any = true
	if not any:
		_tile_quad(Vector2i(0, 0), Color(0, 0, 0, 0), 0.0)
	_overlay_mesh.surface_end()


func _tile_quad(p: Vector2i, col: Color, inset: float) -> void:
	if not in_bounds(p):
		return
	var y := top_y(p.x, p.y) + 0.015
	var a := Vector3(p.x + inset, y, p.y + inset)
	var b := Vector3(p.x + 1 - inset, y, p.y + inset)
	var c := Vector3(p.x + 1 - inset, y, p.y + 1 - inset)
	var d := Vector3(p.x + inset, y, p.y + 1 - inset)
	for v in [a, b, c, a, c, d]:
		_overlay_mesh.surface_set_color(col)
		_overlay_mesh.surface_add_vertex(v)


func _tile_border(p: Vector2i, col: Color) -> void:
	var y := top_y(p.x, p.y) + 0.02
	var t := 0.09
	var rects := [
		[Vector2(0, 0), Vector2(1, t)], [Vector2(0, 1 - t), Vector2(1, 1)],
		[Vector2(0, 0), Vector2(t, 1)], [Vector2(1 - t, 0), Vector2(1, 1)],
	]
	for r in rects:
		var lo: Vector2 = r[0]
		var hi: Vector2 = r[1]
		var a := Vector3(p.x + lo.x, y, p.y + lo.y)
		var b := Vector3(p.x + hi.x, y, p.y + lo.y)
		var c := Vector3(p.x + hi.x, y, p.y + hi.y)
		var d := Vector3(p.x + lo.x, y, p.y + hi.y)
		for v in [a, b, c, a, c, d]:
			_overlay_mesh.surface_set_color(col)
			_overlay_mesh.surface_add_vertex(v)


## Disegna barre degli HP e numeri fluttuanti su un Control in coordinate di
## schermo (`scale` = fattore di ingrandimento della SubViewport).
func draw_hud(ci: CanvasItem, scale: float, font: Font) -> void:
	if cam == null:
		return
	for i in _nodes:
		var node: Dictionary = _nodes[i]
		var u: Dictionary = node["unit"]
		if not bool(u["active"]):
			continue
		var head := screen_of((node["pos"] as Vector3) + Vector3(0, 1.75, 0)) * scale
		var ratio := clampf(float(u["hp"]) / maxf(1.0, float(u["max_hp"])), 0.0, 1.0)
		var bw := 8.0 * scale
		var bar := Rect2(head + Vector2(-bw, 0), Vector2(bw * 2, scale * 2))
		ci.draw_rect(bar.grow(scale), Color(0.06, 0.04, 0.1))
		var hp_col := UI.GOOD if ratio > 0.5 else (UI.ACCENT if ratio > 0.25 else UI.BAD)
		ci.draw_rect(Rect2(bar.position, Vector2(roundf(bar.size.x * ratio / scale) * scale, bar.size.y)), hp_col)
		var team_col: Color = UI.TEAM_COLORS[int(u["team"])]
		ci.draw_rect(Rect2(bar.position + Vector2(-scale * 3, -scale), Vector2(scale * 2, scale * 4)), team_col)
		if int(u["suspended"]) > 0:
			ci.draw_rect(Rect2(bar.end + Vector2(scale * 2, -scale * 3), Vector2(scale * 3, scale * 3)), UI.ACCENT)
		# Icone degli status: quadratini colorati sopra la barra.
		var sx := bar.position.x
		for st in u.get("statuses", []):
			var col: Color = STATUS_COLORS.get(str(st["key"]), Color.WHITE)
			ci.draw_rect(Rect2(Vector2(sx, bar.position.y - scale * 4), Vector2(scale * 3, scale * 3)), Color(0.05, 0.03, 0.1))
			ci.draw_rect(Rect2(Vector2(sx + scale * 0.5, bar.position.y - scale * 3.5), Vector2(scale * 2, scale * 2)), col)
			sx += scale * 4
		if i == champion:
			# Corona a pixel sopra il campione.
			var c := head + Vector2(0, -scale * 5)
			var s := scale
			for r in [Rect2(-3, 0, 7, 2), Rect2(-3, -2, 1, 2), Rect2(0, -3, 1, 3), Rect2(3, -2, 1, 2)]:
				ci.draw_rect(Rect2(c + r.position * s, r.size * s), UI.ACCENT)
			ci.draw_rect(Rect2(c + Vector2(0, 0.5) * s, Vector2(s, s)), UI.BAD)
	for f in floats:
		var a := clampf(1.4 - float(f["t"]), 0.0, 1.0)
		var col: Color = f["color"]
		col.a = a
		var at := screen_of(f["world"] as Vector3) * scale + Vector2(-14, -float(f["t"]) * 30)
		ci.draw_string_outline(font, at, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 22, 6, Color(0.05, 0.03, 0.1, a))
		ci.draw_string(font, at, f["text"], HORIZONTAL_ALIGNMENT_LEFT, -1, 22, col)
