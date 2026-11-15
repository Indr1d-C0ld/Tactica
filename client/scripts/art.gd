extends RefCounted
## Caricamento della grafica con sostituzioni per il pixel artist.
##
## Ordine di ricerca per un atlante `nome`:
##   1. user://override/nome.png   (prova immediata, senza reimportare)
##   2. res://assets/override/nome.png   (sostituzione inclusa nel progetto)
##   3. res://assets/nome.png   (grafica generata da tools/pixelart)

static var _cache := {}


static func tex(name: String) -> Texture2D:
	if _cache.has(name):
		return _cache[name]
	var t: Texture2D = null
	var user_path := "user://override/%s.png" % name
	if FileAccess.file_exists(user_path):
		var img := Image.load_from_file(user_path)
		if img != null:
			t = ImageTexture.create_from_image(img)
			print("Grafica sostituita: ", user_path)
	if t == null and ResourceLoader.exists("res://assets/override/%s.png" % name):
		t = load("res://assets/override/%s.png" % name)
	if t == null:
		t = load("res://assets/%s.png" % name)
	_cache[name] = t
	return t
