module render_3d_toon;

import render_3d_abi;

static const uint TOON_SHADOW_HASH_MULTIPLIER = 0x9e3779b9u;
static const uint TOON_SHADOW_HASH_CHANNEL = 0x85ebca6bu;
static const uint TOON_SHADOW_HASH_MIX = 0x7feb352du;
static const float TOON_SHADOW_HASH_SCALE = 1.0 / 16777216.0;
static const float TOON_GOLDEN_ANGLE = 2.39996322972865332;
static const float3 TOON_LUMINANCE = float3(0.2126, 0.7152, 0.0722);
static const float TOON_EPSILON = 0.00001;
static const int TOON_MAX_HATCH_DIRECTIONS = 6;

float toon_shadow_hash(int index, uint channel)
{
    uint state = asuint(index) * TOON_SHADOW_HASH_MULTIPLIER ^ channel;
    state ^= state >> 16;
    state *= TOON_SHADOW_HASH_MIX;
    state ^= state >> 15;
    return float(state & 0x00ffffffu) * TOON_SHADOW_HASH_SCALE;
}

float toon_shadow_hash(int2 index, uint channel)
{
    uint state = asuint(index.x) * TOON_SHADOW_HASH_MULTIPLIER
        ^ asuint(index.y) * TOON_SHADOW_HASH_CHANNEL
        ^ channel;
    state ^= state >> 16;
    state *= TOON_SHADOW_HASH_MIX;
    state ^= state >> 15;
    return float(state & 0x00ffffffu) * TOON_SHADOW_HASH_SCALE;
}

float toon_shadow_dots(float2 position, ToonSettings settings)
{
    float base_diameter = settings.shadow_dot_size;
    float period = max(1.0 / settings.shadow_dot_density, base_diameter + 1.0);
    float softness = max(settings.shadow_pattern_softness, 0.001);
    int2 base_cell = int2(floor(position / period));
    float coverage = 0.0;
    for (int y = -1; y <= 1; ++y)
    {
        for (int x = -1; x <= 1; ++x)
        {
            int2 cell = base_cell + int2(x, y);
            float size_scale = lerp(
                1.0 - settings.shadow_dot_size_randomness,
                1.0 + settings.shadow_dot_size_randomness,
                toon_shadow_hash(cell, TOON_SHADOW_HASH_CHANNEL));
            float diameter = max(base_diameter * size_scale, 0.25);
            float available_spacing = max(period - diameter, 0.0);
            float2 jitter = float2(
                toon_shadow_hash(cell, 0u),
                toon_shadow_hash(cell, TOON_SHADOW_HASH_MULTIPLIER)) - 0.5;
            float2 center = (float2(cell) + 0.5) * period
                + jitter * available_spacing
                    * settings.shadow_dot_distribution_randomness;
            float radius = diameter * 0.5;
            coverage = max(
                coverage,
                1.0 - smoothstep(
                    max(radius - softness, 0.0),
                    radius + softness,
                    length(position - center)));
        }
    }
    return coverage;
}

float toon_shadow_lines(
    float2 position,
    ToonSettings settings,
    float direction,
    uint family)
{
    float sine;
    float cosine;
    sincos(direction, sine, cosine);
    float coordinate = dot(position, float2(-sine, cosine));
    float base_width = settings.shadow_line_width;
    float period = max(1.0 / settings.shadow_line_density, base_width + 1.0);
    float softness = max(settings.shadow_pattern_softness, 0.001);
    int base_line = int(floor(coordinate / period));
    float coverage = 0.0;
    for (int offset = -1; offset <= 1; ++offset)
    {
        int line = base_line + offset;
        float width_scale = lerp(
            1.0 - settings.shadow_line_width_randomness,
            1.0 + settings.shadow_line_width_randomness,
            toon_shadow_hash(
                line,
                TOON_SHADOW_HASH_CHANNEL ^ family * TOON_SHADOW_HASH_MULTIPLIER));
        float line_width = max(base_width * width_scale, 0.25);
        float available_spacing = max(period - line_width, 0.0);
        float jitter = (toon_shadow_hash(
            line,
            family * TOON_SHADOW_HASH_CHANNEL) - 0.5)
            * available_spacing
            * settings.shadow_line_distribution_randomness;
        float line_center = (float(line) + 0.5) * period + jitter;
        float half_width = line_width * 0.5;
        coverage = max(
            coverage,
            1.0 - smoothstep(
                max(half_width - softness, 0.0),
                half_width + softness,
                abs(coordinate - line_center)));
    }
    return coverage;
}

public float toon_tone_index(float illumination, ToonSettings settings)
{
    return round(saturate(illumination) * (settings.bands - 1.0));
}

public float toon_tone_value(float tone_index, ToonSettings settings)
{
    return tone_index / (settings.bands - 1.0);
}

public float toon_quantized_intensity(float intensity, ToonSettings settings)
{
    return toon_tone_value(toon_tone_index(intensity, settings), settings);
}

public float toon_shadow_pattern(
    float2 position,
    float tone_index,
    ToonSettings settings)
{
    float darkness = 1.0 - toon_tone_value(tone_index, settings);
    if (darkness <= 0.0)
        return 0.0;

    switch (settings.shadow_kind)
    {
    case ToonShadowKind::Dots:
        return toon_shadow_dots(position, settings);
    case ToonShadowKind::Lines:
        return toon_shadow_lines(
            position,
            settings,
            settings.shadow_line_direction,
            0u);
    case ToonShadowKind::Crosshatch:
        int direction_count = int(ceil(
            darkness * settings.shadow_crosshatch_max_directions));
        float coverage = 0.0;
        for (int family = 0; family < TOON_MAX_HATCH_DIRECTIONS; ++family)
        {
            if (family >= direction_count)
                break;
            float direction = settings.shadow_line_direction
                + float(family) * TOON_GOLDEN_ANGLE;
            coverage = max(
                coverage,
                toon_shadow_lines(
                    position,
                    settings,
                    direction,
                    uint(family)));
        }
        return coverage;
    default:
        return darkness;
    }
}

public float toon_light_response(
    float3 normal,
    float3 light,
    float3 radiance,
    float visibility)
{
    return dot(radiance, TOON_LUMINANCE)
        * saturate(dot(normal, light))
        * visibility;
}

public float3 toon_specular_light(
    float3 normal,
    float3 view,
    float3 light,
    float3 radiance,
    float visibility,
    ToonSettings settings)
{
    float3 half_direction = normalize(view + light);
    float highlight = saturate(dot(normal, half_direction));
    float specular = step(1.0 - settings.specular_size, highlight)
        * settings.specular_strength
        * visibility;
    return radiance * specular;
}

public float3 toon_tone_fill(
    float3 base_color,
    float3 light_color,
    float tone,
    ToonSettings settings)
{
    float base_luminance = dot(base_color, TOON_LUMINANCE);
    float light_luminance = dot(light_color, TOON_LUMINANCE);
    float3 tint = light_luminance > TOON_EPSILON
        ? light_color / light_luminance
        : float3(1.0);
    float3 tinted = base_color * tint;
    float tinted_luminance = dot(tinted, TOON_LUMINANCE);
    float target_luminance = base_luminance
        * lerp(settings.shadow_darkest_tone, 1.0, tone);
    return tinted_luminance > TOON_EPSILON
        ? saturate(tinted * target_luminance / tinted_luminance)
        : base_color * lerp(settings.shadow_darkest_tone, 1.0, tone);
}

public float3 toon_palette_quantize(float3 color, float levels)
{
    return round(saturate(color) * (levels - 1.0)) / (levels - 1.0);
}

public float toon_outline_fresnel(float facing, float width)
{
    return smoothstep(saturate(1.0 - width * 0.08), 1.0, facing);
}
