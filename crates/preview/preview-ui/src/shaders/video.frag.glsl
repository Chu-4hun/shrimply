#version 330 core
in vec2 v_uv;
uniform sampler2D u_rgba_texture;
uniform int u_has_frame;
uniform int u_draw_frame;
uniform vec2 u_surface_size;
uniform vec4 u_content_rect;
uniform vec4 u_background_color;
uniform float u_shadow_size;
out vec4 out_color;

const vec3 SHADOW_COLOR = vec3(0.0, 0.0, 6.0 / 255.0);
const float ADW_SHADOW_SIZE = 6.0;
const float OUTLINE_OPACITY = 0.03;
const float NEAR_SHADOW_OPACITY = 0.07;
const float FAR_SHADOW_OPACITY = 0.03;

float box_distance(vec2 point, vec2 center, vec2 half_size) {
    vec2 offset = abs(point - center) - half_size;
    return length(max(offset, vec2(0.0))) + min(max(offset.x, offset.y), 0.0);
}

float shadow_layer(
    vec2 point,
    vec2 center,
    vec2 half_size,
    float offset_y,
    float spread,
    float blur
) {
    float distance = box_distance(
        point - vec2(0.0, offset_y),
        center,
        half_size + vec2(spread)
    );
    return 1.0 - smoothstep(-blur, blur, distance);
}

void main() {
    if (u_draw_frame == 1) {
        vec2 tile = floor(v_uv * u_content_rect.zw / 32.0);
        float value = mod(tile.x + tile.y, 2.0) < 1.0 ? 0.21 : 0.29;
        vec4 frame = u_has_frame == 1
            ? texture(u_rgba_texture, v_uv)
            : vec4(value, value, value, 1.0);
        out_color = vec4(mix(vec3(value), frame.rgb, frame.a), 1.0);
        return;
    }

    vec2 pixel = vec2(gl_FragCoord.x, u_surface_size.y - gl_FragCoord.y);
    vec2 center = u_content_rect.xy + u_content_rect.zw * 0.5;
    vec2 half_size = u_content_rect.zw * 0.5;
    float shadow_alpha = 0.0;
    if (u_shadow_size > 0.0) {
        float scale = u_shadow_size / ADW_SHADOW_SIZE;
        float outline = shadow_layer(pixel, center, half_size, 0.0, 0.0, 1.0 * scale)
            * OUTLINE_OPACITY;
        float near_shadow = shadow_layer(
            pixel,
            center,
            half_size,
            1.0 * scale,
            1.0 * scale,
            3.0 * scale
        ) * NEAR_SHADOW_OPACITY;
        float far_shadow = shadow_layer(
            pixel,
            center,
            half_size,
            2.0 * scale,
            2.0 * scale,
            6.0 * scale
        ) * FAR_SHADOW_OPACITY;
        shadow_alpha = 1.0
            - (1.0 - outline) * (1.0 - near_shadow) * (1.0 - far_shadow);
    }
    float alpha = shadow_alpha + u_background_color.a * (1.0 - shadow_alpha);
    vec3 premultiplied = SHADOW_COLOR * shadow_alpha
        + u_background_color.rgb * u_background_color.a * (1.0 - shadow_alpha);
    out_color = vec4(premultiplied, alpha);
}
