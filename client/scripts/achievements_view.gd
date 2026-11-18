extends VBoxContainer
## Scheda «Traguardi»: riepilogo, filtro per categoria e stato, una riga per
## traguardo con grado, progresso, data di sblocco e rarità fra i giocatori.

const UI := preload("res://scripts/ui.gd")
const Stats := preload("res://scripts/stats_view.gd")

const TIER_NAMES := {"bronze": "Bronzo", "silver": "Argento", "gold": "Oro", "platinum": "Platino"}
const TIER_COLORS := {"bronze": Color(0.85, 0.55, 0.3), "silver": Color(0.8, 0.84, 0.9),
	"gold": Color(0.98, 0.8, 0.3), "platinum": Color(0.6, 0.95, 1.0)}
## Icone PixelLab: una per categoria e una medaglia per grado.
static func icon(name: String) -> Texture2D:
	var path := "res://assets/achievements/%s.png" % name
	return load(path) if ResourceLoader.exists(path) else null


static func icon_rect(name: String, px := 32, dim := false) -> TextureRect:
	var t := TextureRect.new()
	t.texture = icon(name)
	t.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	t.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	t.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	t.custom_minimum_size = Vector2(px, px)
	t.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	if dim:
		t.modulate = Color(0.35, 0.37, 0.45, 0.8)
	return t


const FILTERS := [["Tutti", ""], ["Sbloccati", "done"], ["Da fare", "todo"]]

var _book: Dictionary = {}
var _category := ""
var _filter := ""
var _head: Label
var _list: VBoxContainer
var _cats: HFlowContainer


func _ready() -> void:
	add_theme_constant_override("separation", 10)
	_head = UI.label("", 16, UI.ACCENT)
	add_child(_head)
	_cats = HFlowContainer.new()
	_cats.add_theme_constant_override("h_separation", 6)
	_cats.add_theme_constant_override("v_separation", 6)
	add_child(_cats)
	var row := UI.hbox(10)
	row.add_child(UI.label("Mostra:", 16))
	var opt := UI.option(FILTERS, 0)
	opt.item_selected.connect(func(i: int) -> void:
		_filter = str(opt.get_item_metadata(i))
		_fill())
	row.add_child(opt)
	add_child(row)
	_list = UI.vbox(6)
	add_child(_list)


func refresh() -> void:
	var r := await Net.get_json("/api/me/achievements")
	if int(r.status) != 200:
		_head.text = Net.error_text(r)
		return
	_book = r.data
	_head.text = "Traguardi sbloccati: %d su %d · monete guadagnate dai traguardi: %d" % [
		int(_book["unlocked"]), int(_book["total"]), int(_book["coins"])]
	UI.clear(_cats)
	var all := [["", "Tutte"]] + (_book["categories"] as Array)
	for c in all:
		var key := str(c[0])
		var done := 0
		var tot := 0
		for e in _book["entries"]:
			if key == "" or str(e["category"]) == key:
				tot += 1
				if e["unlocked_at"] != null:
					done += 1
		var b := UI.button("%s %d/%d" % [c[1], done, tot], func() -> void:
			_category = key
			_fill())
		if key != "":
			b.icon = icon(key)
			b.expand_icon = false
		_cats.add_child(b)
	_fill()


func _fill() -> void:
	UI.clear(_list)
	if _book.is_empty():
		return
	var shown := 0
	for e in _book["entries"]:
		if _category != "" and str(e["category"]) != _category:
			continue
		var done: bool = e["unlocked_at"] != null
		if (_filter == "done" and not done) or (_filter == "todo" and done):
			continue
		_list.add_child(_row(e))
		shown += 1
	if shown == 0:
		_list.add_child(UI.label("Niente da mostrare con questo filtro.", 16, UI.MUTED))


func _row(e: Dictionary) -> Control:
	var done: bool = e["unlocked_at"] != null
	var tier := str(e["tier"])
	var box := UI.hbox(12)
	# Icona della categoria e medaglia del grado, spente finché manca.
	box.add_child(icon_rect(str(e["category"]), 48, not done))
	box.add_child(icon_rect("medaglia_" + tier, 48, not done))
	var text := UI.vbox(2)
	text.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var title := "%s  ·  %s" % [e["name"], TIER_NAMES.get(tier, tier)]
	text.add_child(UI.label(title, 16, TIER_COLORS.get(tier, Color.WHITE) if done else Color(0.85, 0.86, 0.9)))
	var desc := UI.label(str(e["desc"]), 16, UI.MUTED)
	desc.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	desc.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	text.add_child(desc)
	box.add_child(text)
	var right := UI.vbox(2)
	right.custom_minimum_size = Vector2(260, 0)
	if done:
		right.add_child(UI.label("Sbloccato il " + Stats.date(int(e["unlocked_at"]), false), 16, UI.GOOD))
	else:
		var target := int(e["target"])
		var bar := ProgressBar.new()
		bar.max_value = target
		bar.value = int(e["progress"])
		bar.show_percentage = false
		bar.custom_minimum_size = Vector2(240, 12)
		right.add_child(bar)
		right.add_child(UI.label("%d / %d" % [int(e["progress"]), target], 16, UI.MUTED))
	right.add_child(UI.label("ce l'ha il %d%% dei giocatori" % int(round(float(e["rarity"]))), 16, UI.MUTED))
	box.add_child(right)
	return UI.panel(box, 8)


## Avviso per i traguardi appena sbloccati (dal campo `new_achievements` di /api/me).
static func toast(parent: Control, list: Array) -> void:
	if list.is_empty():
		return
	var lines: Array[String] = []
	var coins := 0
	for a in list:
		lines.append("★ %s (%s)" % [a["name"], TIER_NAMES.get(str(a["tier"]), "")])
		coins += int(a["coins"])
	var shown := lines.slice(0, 6)
	if lines.size() > 6:
		shown.append("…e altri %d" % (lines.size() - 6))
	var box := UI.vbox(4)
	box.add_child(icon_rect("medaglia_" + str(list[0]["tier"]), 64))
	box.add_child(UI.label("Traguardo sbloccato!" if list.size() == 1 else "%d traguardi sbloccati!" % list.size(), 26, UI.ACCENT))
	for l in shown:
		box.add_child(UI.label(l, 16))
	box.add_child(UI.label("+%d monete" % coins, 16, UI.GOOD))
	var p := UI.panel(box, 14)
	p.mouse_filter = Control.MOUSE_FILTER_IGNORE
	parent.add_child(p)
	p.set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
	p.position.y = 70
	p.modulate.a = 0.0
	var sfx := parent.get_node_or_null("/root/Sfx")
	if sfx != null:
		sfx.play("levelup")
	var tw := parent.create_tween()
	tw.tween_property(p, "modulate:a", 1.0, 0.25)
	tw.tween_interval(4.0)
	tw.tween_property(p, "modulate:a", 0.0, 0.6)
	tw.tween_callback(p.queue_free)
	Net.post_json("/api/me/achievements/seen")
