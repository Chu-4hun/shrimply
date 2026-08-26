float aces_tone_map(float value)
{
    return saturate((value * (2.51 * value + 0.03))
        / (value * (2.43 * value + 0.59) + 0.14));
}

float linear_to_srgb(float value)
{
    return value <= 0.0031308
        ? value * 12.92
        : 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

float srgb_to_linear(float value)
{
    return value <= 0.04045
        ? value / 12.92
        : pow((value + 0.055) / 1.055, 2.4);
}

float wrap_texture_coordinate(float value, uint mode)
{
    if (mode == 0)
        return saturate(value);
    if (mode == 1)
        return 1.0 - abs(frac(value * 0.5) * 2.0 - 1.0);
    return frac(value);
}

float2 mapped_texture_coordinates(TextureMapping mapping, float2 uv)
{
    float sine = sin(mapping.rotation_filter.x);
    float cosine = cos(mapping.rotation_filter.x);
    float2 scaled = uv * mapping.transform.zw;
    float2 transformed = mapping.transform.xy + float2(
        cosine * scaled.x - sine * scaled.y,
        sine * scaled.x + cosine * scaled.y);
    return float2(
        wrap_texture_coordinate(transformed.x, mapping.settings.z),
        wrap_texture_coordinate(transformed.y, mapping.settings.w));
}

float2 vertex_uv(uint vertex, uint set)
{
    return set == 0 ? tex_coords_0[vertex].xy : tex_coords_1[vertex].xy;
}

float4 sample_material_texture(
    TextureMapping mapping,
    uint first,
    float3 barycentric)
{
    if (mapping.settings.x == 0)
        return float4(1.0);
    float2 uv = vertex_uv(first, mapping.settings.y) * barycentric.x
        + vertex_uv(first + 1, mapping.settings.y) * barycentric.y
        + vertex_uv(first + 2, mapping.settings.y) * barycentric.z;
    uv = mapped_texture_coordinates(mapping, uv);
    float2 atlas_uv = mapping.atlas.xy + uv * mapping.atlas.zw;
    if (mapping.rotation_filter.y == 0.0)
        return material_texture.SampleLevel(material_sampler, atlas_uv, 0.0);
    uint width;
    uint height;
    material_texture.GetDimensions(width, height);
    int2 pixel = int2(clamp(
        floor(atlas_uv * float2(width, height)),
        float2(0.0),
        float2(width - 1, height - 1)));
    return material_texture.Load(int3(pixel, 0));
}

float4 surface_base_color(uint first, float3 barycentric, MeshMaterial material)
{
    float4 vertex_color = vertex_colors[first] * barycentric.x
        + vertex_colors[first + 1] * barycentric.y
        + vertex_colors[first + 2] * barycentric.z;
    float4 sampled = sample_material_texture(
        material.base_color_texture,
        first,
        barycentric);
    sampled.rgb = float3(
        srgb_to_linear(sampled.r),
        srgb_to_linear(sampled.g),
        srgb_to_linear(sampled.b));
    return material.base_color_factor * vertex_color * sampled;
}

