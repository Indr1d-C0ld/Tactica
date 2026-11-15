## Oggetti: schede (catalogo o generati), colori di rarità e testi descrittivi.
##
## Le schede arrivano dal server (`/api/items`, inventario) oppure, per gli
## oggetti generati, si ricavano dalla chiave con l'estensione Rust: la chiave
## descrive l'oggetto per intero.

const RARITY_COLORS := {
	"common": Color(0.86, 0.86, 0.84),
	"uncommon": Color(0.45, 0.88, 0.45),
	"rare": Color(0.38, 0.66, 1.0),
	"epic": Color(0.78, 0.5, 1.0),
	"legendary": Color(1.0, 0.65, 0.25),
}

const STAT_NAMES := {"atk": "ATK", "def": "DEF", "mag": "MAG", "res": "RES", "max_hp": "HP",
	"max_mp": "MP", "spd": "VEL", "mov": "MOV", "evade": "SCHIV", "jump": "SALTO"}


static func info(key: String) -> Dictionary:
	if Net.items.has(key):
		return Net.items[key]
	var d: Variant = JSON.parse_string(TacticaEngine.item_info_json(key))
	if d is Dictionary and not (d as Dictionary).is_empty():
		Net.items[key] = d
		return d
	return {}


static func name_of(key: String) -> String:
	return str(info(key).get("name", key))


static func color(rarity: String) -> Color:
	return RARITY_COLORS.get(rarity, Color.WHITE)


static func color_of(key: String) -> Color:
	return color(str(info(key).get("rarity", "common")))


## Nome colorato per la rarità, in BBCode.
static func bb_name(key: String) -> String:
	var d := info(key)
	return "[color=#%s]%s[/color]" % [color(str(d.get("rarity", "common"))).to_html(false), d.get("name", key)]


static func bonus_text(b: Dictionary) -> String:
	var parts: Array[String] = []
	for k in STAT_NAMES:
		var v := int(b.get(k, 0))
		if v != 0:
			parts.append("%s %+d" % [STAT_NAMES[k], v])
	return " ".join(parts)


## Descrizione completa: rarità, slot, statistiche, effetti, abilità insegnata, storia.
static func describe(key: String, ability_name: Callable = Callable()) -> String:
	var d := info(key)
	if d.is_empty():
		return key
	var lines: Array[String] = [bb_name(key)]
	lines.append("[color=#aaa]%s · %s[/color]" % [str(d.get("rarity_name", "")).capitalize(), d.get("slot", "")])
	var b := bonus_text(d.get("bonus", {}))
	if b != "":
		lines.append(b)
	for e in d.get("effects", []):
		lines.append("[color=#9cf]%s[/color]" % e)
	if d.get("teaches") != null:
		var an := str(d["teaches"])
		if ability_name.is_valid():
			an = ability_name.call(an)
		lines.append("[color=#fc6]Insegna: %s[/color]" % an)
	if d.get("lore") != null:
		lines.append("[i][color=#cba]%s[/color][/i]" % d["lore"])
	return "\n".join(lines)


## Testo senza tag BBCode (per i tooltip, che non li interpretano).
static func plain(bb: String) -> String:
	var re := RegEx.new()
	re.compile("\\[[^\\]]*\\]")
	return re.sub(bb, "", true)


## Icone PixelLab (atlante client/assets/items.png con indice items.json).
static var _atlas: Texture2D = null
static var _index := {}
static var _icon_cache := {}

static func _load_atlas() -> void:
	if _atlas != null or not ResourceLoader.exists("res://assets/items.png"):
		return
	_atlas = preload("res://scripts/art.gd").tex("items")
	var f := FileAccess.open("res://assets/items.json", FileAccess.READ)
	if f:
		var d: Variant = JSON.parse_string(f.get_as_text())
		if d is Dictionary:
			_index = d


## Icona 32x32 di un oggetto (per chiave dell'oggetto o nome dell'icona); null se manca.
static func icon(key_or_icon: String) -> Texture2D:
	_load_atlas()
	if _atlas == null:
		return null
	var name := key_or_icon
	var d := info(key_or_icon)
	if not d.is_empty():
		name = str(d.get("icon", key_or_icon))
	if _icon_cache.has(name):
		return _icon_cache[name]
	var icons: Dictionary = _index.get("icons", {})
	if not icons.has(name):
		return null
	var i := int(icons[name])
	var cols := int(_index.get("columns", 8))
	var cell := int(_index.get("cell", 32))
	var at := AtlasTexture.new()
	at.atlas = _atlas
	at.region = Rect2((i % cols) * cell, (i / cols) * cell, cell, cell)
	_icon_cache[name] = at
	return at


## Riquadro con l'icona ingrandita 2× e una cornice del colore della rarità.
static func icon_box(key: String) -> Control:
	var tr := TextureRect.new()
	tr.texture = icon(key)
	tr.custom_minimum_size = Vector2(64, 64)
	tr.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	tr.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	tr.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	var panel := PanelContainer.new()
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.07, 0.08, 0.12)
	sb.border_color = color_of(key)
	sb.set_border_width_all(2)
	panel.add_theme_stylebox_override("panel", sb)
	panel.add_child(tr)
	return panel


## Icona del bottino a terra: un sacchetto in pixel-art del colore della rarità
## (12x14, contorno scuro, legaccio chiaro, riflesso). Generata una volta per rarità.
static var _bag_cache := {}

static func bag_texture(rarity: String) -> Texture2D:
	if _bag_cache.has(rarity):
		return _bag_cache[rarity]
	var c := color(rarity)
	var dark := c.darkened(0.55)
	var mid := c.darkened(0.2)
	var light := c.lightened(0.35)
	var outline := Color(0.08, 0.06, 0.1)
	var rope := Color(0.92, 0.82, 0.55)
	var rows := [
		"....oooo....",
		"...oRRRRo...",
		"....oRRo....",
		"...oMLLMo...",
		"..oMLLMMMo..",
		".oMLLMMMMDo.",
		".oMLMMMMMDo.",
		"oMMMMMMMMDDo",
		"oMMMMMMMMDDo",
		"oMMMMMMMDDDo",
		".oMMMMMDDDo.",
		".oDMMMDDDDo.",
		"..ooDDDDoo..",
		"....oooo....",
	]
	var img := Image.create(12, 14, false, Image.FORMAT_RGBA8)
	for y in rows.size():
		var row: String = rows[y]
		for x in row.length():
			var ch := row[x]
			var col := Color(0, 0, 0, 0)
			match ch:
				"o": col = outline
				"R": col = rope
				"M": col = mid
				"L": col = light
				"D": col = dark
			img.set_pixel(x, y, col)
	var tex := ImageTexture.create_from_image(img)
	_bag_cache[rarity] = tex
	return tex


## Scintilla 5x5 del colore della rarità, sopra il sacchetto a terra.
static var _spark_cache := {}

static func spark_texture(rarity: String) -> Texture2D:
	if _spark_cache.has(rarity):
		return _spark_cache[rarity]
	var c := color(rarity)
	var img := Image.create(5, 5, false, Image.FORMAT_RGBA8)
	for p in [Vector2i(2, 0), Vector2i(2, 4), Vector2i(0, 2), Vector2i(4, 2)]:
		img.set_pixelv(p, c)
	for p in [Vector2i(2, 1), Vector2i(2, 3), Vector2i(1, 2), Vector2i(3, 2)]:
		img.set_pixelv(p, c.lightened(0.4))
	img.set_pixel(2, 2, Color.WHITE)
	var tex := ImageTexture.create_from_image(img)
	_spark_cache[rarity] = tex
	return tex
