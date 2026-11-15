// Filtro "VGA/SVGA": riduce i colori a pochi livelli per canale con retino
// ordinato (Bayer 4x4) calcolato sui pixel della risoluzione interna.
shader_type canvas_item;

uniform vec2 source_size = vec2(426.0, 240.0);
uniform float levels = 6.0;
uniform float dither = 1.0;
uniform bool enabled = true;

const float BAYER[16] = float[](0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
                                3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);

void fragment() {
	vec4 c = texture(TEXTURE, UV);
	if (enabled) {
		ivec2 p = ivec2(floor(UV * source_size));
		float t = (BAYER[(p.y % 4) * 4 + (p.x % 4)] + 0.5) / 16.0 - 0.5;
		float n = levels - 1.0;
		c.rgb = clamp(floor(c.rgb * n + 0.5 + t * dither) / n, 0.0, 1.0);
	}
	COLOR = c;
}
