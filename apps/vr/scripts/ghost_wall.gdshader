shader_type spatial;
render_mode unshaded, cull_disabled, depth_draw_never, shadows_disabled;

uniform vec3 energy : source_color = vec3(0.20, 0.87, 1.0);
uniform float opacity : hint_range(0.0, 1.0) = 1.0;
uniform float layer : hint_range(0.0, 1.0) = 1.0;

float band(float center, float width, float value) {
	float distance = abs(value - center);
	return 1.0 - smoothstep(width, width + max(fwidth(value), 0.002), distance);
}

void fragment() {
	vec2 p = abs(UV - 0.5);
	float inset = min(min(0.5 - p.x, 0.5 - p.y), (0.89 - p.x - p.y) * 0.707);
	if (inset < 0.0) {
		discard;
	}
	float edge = band(0.012, 0.003, inset);
	float rail = band(0.053, 0.002, inset);
	float corner = smoothstep(0.32, 0.46, min(p.x, p.y));
	float halo = exp(-inset * 44.0) * 0.75;
	vec2 lattice = abs(fract(UV * vec2(9.0, 7.0)) - 0.5);
	float grid = band(0.47, 0.005, max(lattice.x, lattice.y));
	float scan_position = fract(TIME * 0.42);
	float sweep = band(scan_position, 0.016, UV.y) * 0.42;
	float chevrons = band(0.038, 0.006, abs(p.x - p.y * 0.70)) * smoothstep(0.22, 0.37, p.y);
	float highlight = clamp(edge + rail * 0.55 + corner * 0.45 + sweep * 0.42, 0.0, 1.0);
	vec3 glass = mix(energy * 0.15, energy * 0.68, 1.0 - UV.y);
	vec3 rim = mix(energy, vec3(0.93, 1.0, 1.0), edge * 0.83);
	ALBEDO = mix(glass + energy * (grid * 0.20 + chevrons * 0.25 + sweep * 0.38), rim, highlight);
	ALPHA = opacity * layer * clamp(0.24 + halo * 0.36 + highlight * 0.56 + chevrons * 0.24 + grid * 0.16, 0.0, 0.97);
}
