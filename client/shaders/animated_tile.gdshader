// Sommità animate (acqua, lava): scorre fra 3 fotogrammi affiancati nell'atlante.
shader_type spatial;
render_mode unshaded, cull_disabled;

uniform sampler2D atlas : source_color, filter_nearest;
uniform float cell_u = 0.25;
uniform float fps = 2.0;

void fragment() {
	float f = floor(mod(TIME * fps + COLOR.a * 3.0, 3.0));
	vec4 c = texture(atlas, UV + vec2(f * cell_u, 0.0));
	ALBEDO = c.rgb * COLOR.rgb;
}
