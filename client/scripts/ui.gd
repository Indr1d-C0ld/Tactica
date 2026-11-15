extends RefCounted
## Piccole funzioni per costruire l'interfaccia da codice.

const TEAM_COLORS := [Color(0.25, 0.47, 0.9), Color(0.85, 0.3, 0.25)]
const ACCENT := Color(0.95, 0.78, 0.35)
const MUTED := Color(0.65, 0.68, 0.75)
const BAD := Color(0.95, 0.4, 0.35)
const GOOD := Color(0.45, 0.85, 0.5)


static func label(text: String, size := 0, color := Color(0, 0, 0, 0)) -> Label:
	var l := Label.new()
	l.text = text
	if size > 0:
		l.add_theme_font_size_override("font_size", size)
	if color.a > 0:
		l.add_theme_color_override("font_color", color)
	return l


static func button(text: String, action: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.pressed.connect(action)
	return b


static func hbox(sep := 8) -> HBoxContainer:
	var h := HBoxContainer.new()
	h.add_theme_constant_override("separation", sep)
	return h


static func vbox(sep := 6) -> VBoxContainer:
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", sep)
	return v


static func panel(child: Control, pad := 12) -> PanelContainer:
	var p := PanelContainer.new()
	var m := MarginContainer.new()
	for side in ["left", "right", "top", "bottom"]:
		m.add_theme_constant_override("margin_" + side, pad)
	m.add_child(child)
	p.add_child(m)
	return p


static func scroll(child: Control) -> ScrollContainer:
	var s := ScrollContainer.new()
	s.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	s.size_flags_vertical = Control.SIZE_EXPAND_FILL
	s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	child.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	s.add_child(child)
	return s


static func line_edit(placeholder: String, text := "", secret := false) -> LineEdit:
	var e := LineEdit.new()
	e.placeholder_text = placeholder
	e.text = text
	e.secret = secret
	e.custom_minimum_size = Vector2(260, 0)
	return e


static func option(items: Array, selected := 0) -> OptionButton:
	## `items`: lista di [testo, valore].
	var o := OptionButton.new()
	for i in items.size():
		o.add_item(str(items[i][0]), i)
		o.set_item_metadata(i, items[i][1])
	if items.size() > 0:
		o.select(clampi(selected, 0, items.size() - 1))
	return o


static func clear(node: Node) -> void:
	for c in node.get_children():
		node.remove_child(c)
		c.queue_free()


static func full_rect(c: Control) -> void:
	c.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
