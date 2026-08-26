struct GaussianSource
{
    float4 position_opacity;
    float4 scale;
    float4 rotation;
    float4 dc;
};

struct GaussianUniforms
{
    float4x4 object_to_camera;
    float4 camera_in_object;
    float4 viewport_projection;
    float4 orthographic_source;
    float4 source_center_degree;
    float4 camera_lens;
};

struct GaussianVertexOutput
{
    float4 position : SV_Position;
    float2 delta : TEXCOORD0;
    nointerpolation float4 conic_opacity : TEXCOORD1;
    nointerpolation float4 color_depth : TEXCOORD2;
};

struct ResolveVertexOutput
{
    float4 position : SV_Position;
};

struct SortConstants
{
    uint count;
    uint shift;
    uint read_scratch;
    uint group_count;
};

static const float MINIMUM_DEPTH = 0.001;
static const float MINIMUM_LOG_SCALE = -20.0;
static const float MAXIMUM_LOG_SCALE = 20.0;
static const float COVARIANCE_LOW_PASS = 0.3;
static const float SIGMA_EXTENT = 3.0;
static const float MINIMUM_ALPHA = 1.0 / 255.0;
static const float MAXIMUM_ALPHA = 0.99;
static const float FRUSTUM_DILATION = 1.5;
static const float PI = 3.14159265358979323846;
static const uint SORT_THREADS = 256;
static const uint RADIX_BITS = 4;
static const uint RADIX_SIZE = 1 << RADIX_BITS;
static const uint MAXIMUM_WAVES = SORT_THREADS / 8;
static const float SH_C0 = 0.2820948;
static const float SH_C1 = 0.48860252;
static const float SH_C2[5] = {
    1.0925485, -1.0925485, 0.31539157, -1.0925485, 0.54627424
};
static const float SH_C3[7] = {
    -0.5900436, 2.8906114, -0.4570458, 0.37317634,
    -0.4570458, 1.4453057, -0.5900436
};

[[vk::binding(0, 0)]] StructuredBuffer<GaussianSource> gaussians;
[[vk::binding(1, 0)]] StructuredBuffer<float> higher_order_sh;
[[vk::binding(2, 0)]] Texture2D<float4> accumulation_texture;
[[vk::binding(4, 0)]] ConstantBuffer<GaussianUniforms> uniforms;
[[vk::binding(5, 0)]] RWStructuredBuffer<uint> sort_keys;
[[vk::binding(6, 0)]] RWStructuredBuffer<uint> sorted_indices;
[[vk::binding(7, 0)]] RWStructuredBuffer<uint> scratch_keys;
[[vk::binding(8, 0)]] RWStructuredBuffer<uint> scratch_indices;
[[vk::binding(9, 0)]] RWStructuredBuffer<uint> sort_group_offsets;
[[vk::binding(10, 0)]] RWStructuredBuffer<uint> draw_indirect;
[[vk::push_constant]] ConstantBuffer<SortConstants> sort_constants;

uint read_sort_key(uint index)
{
    return sort_constants.read_scratch != 0
        ? scratch_keys[index]
        : sort_keys[index];
}

uint read_sort_index(uint index)
{
    return sort_constants.read_scratch != 0
        ? scratch_indices[index]
        : sorted_indices[index];
}

void write_sort_value(uint offset, uint key, uint index)
{
    if (sort_constants.read_scratch != 0)
    {
        sort_keys[offset] = key;
        sorted_indices[offset] = index;
    }
    else
    {
        scratch_keys[offset] = key;
        scratch_indices[offset] = index;
    }
}

[shader("compute")]
[numthreads(SORT_THREADS, 1, 1)]
void prepare_depth_sort(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint index = dispatch_thread_id.x;
    if (index >= sort_constants.count)
        return;
    GaussianSource gaussian = gaussians[index];
    float3 object_position =
        (gaussian.position_opacity.xyz - uniforms.source_center_degree.xyz)
        / uniforms.orthographic_source.z;
    float3 camera_position = mul(
        uniforms.object_to_camera,
        float4(object_position, 1.0)).xyz;
    float projection = uniforms.viewport_projection.w;
    bool radial = projection == 2.0 || projection == 3.0 || projection == 4.0;
    float depth = radial ? length(camera_position) : -camera_position.z;
    bool visible = depth > MINIMUM_DEPTH && isfinite(depth);
    if (visible && projection == 1.0)
    {
        float2 ndc = 2.0 * uniforms.viewport_projection.z
            * camera_position.xy / (depth * uniforms.viewport_projection.xy);
        visible = all(abs(ndc) <= float2(FRUSTUM_DILATION));
    }
    else if (visible && projection == 0.0)
    {
        float2 ndc = 2.0 * camera_position.xy
            / uniforms.orthographic_source.xy;
        visible = all(abs(ndc) <= float2(FRUSTUM_DILATION));
    }
    float opacity = 1.0 / (
        1.0 + exp(-clamp(gaussian.position_opacity.w, -80.0, 80.0)));
    visible = visible && opacity >= MINIMUM_ALPHA;
    sort_keys[index] = visible
        ? ~asuint(depth)
        : 0xffffffff;
    sorted_indices[index] = index;
    if (visible)
    {
        uint lens_sample_count = max(uint(uniforms.camera_lens.w), 1u);
        uint instance_copies = projection == 2.0 || projection == 3.0
            ? 3u
            : lens_sample_count;
        uint ignored;
        InterlockedAdd(draw_indirect[1], instance_copies, ignored);
    }
}

float2 wrapped_center(float3 camera_position, float2 viewport, float projection)
{
    float longitude = atan2(camera_position.x, -camera_position.z);
    float x = viewport.x * (0.5 + longitude / (2.0 * PI));
    if (projection < 3.0)
    {
        float distance = max(length(camera_position), MINIMUM_DEPTH);
        float latitude = asin(clamp(camera_position.y / distance, -1.0, 1.0));
        return float2(x, viewport.y * (0.5 - latitude / PI));
    }
    float radial_distance = max(
        length(float2(camera_position.x, camera_position.z)),
        MINIMUM_DEPTH);
    return float2(
        x,
        viewport.y * (0.5 - 0.5 * camera_position.y
            / (radial_distance * uniforms.camera_lens.z)));
}

float2 wrapped_delta(float2 endpoint, float2 center, float viewport_width)
{
    float2 delta = endpoint - center;
    delta.x -= round(delta.x / viewport_width) * viewport_width;
    return delta;
}

float fisheye_angle(float3 camera_position)
{
    float distance = max(length(camera_position), MINIMUM_DEPTH);
    return acos(clamp(-camera_position.z / distance, -1.0, 1.0));
}

float2 fisheye_center(float3 camera_position, float2 viewport)
{
    float radial_distance = length(camera_position.xy);
    float2 direction = radial_distance > MINIMUM_DEPTH
        ? camera_position.xy / radial_distance
        : float2(0.0);
    float pixel_radius = fisheye_angle(camera_position)
        / uniforms.camera_lens.z
        * min(viewport.x, viewport.y)
        * 0.5;
    return viewport * 0.5 + float2(direction.x, -direction.y) * pixel_radius;
}

float2 aperture_sample(uint index, uint count)
{
    static const float GOLDEN_ANGLE = 2.39996322972865332;
    float radius = sqrt((float(index) + 0.5) / float(count));
    float angle = float(index) * GOLDEN_ANGLE;
    return float2(cos(angle), sin(angle)) * radius;
}

groupshared uint radix_histogram[RADIX_SIZE];

[shader("compute")]
[numthreads(SORT_THREADS, 1, 1)]
void radix_histogram_pass(
    uint3 group_id : SV_GroupID,
    uint group_thread_id : SV_GroupIndex,
    uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    if (group_thread_id < RADIX_SIZE)
        radix_histogram[group_thread_id] = 0;
    GroupMemoryBarrierWithGroupSync();
    uint index = dispatch_thread_id.x;
    if (index < sort_constants.count)
    {
        uint digit = (read_sort_key(index) >> sort_constants.shift) & (RADIX_SIZE - 1);
        uint ignored;
        InterlockedAdd(radix_histogram[digit], 1, ignored);
    }
    GroupMemoryBarrierWithGroupSync();
    if (group_thread_id < RADIX_SIZE)
    {
        sort_group_offsets[group_id.x * RADIX_SIZE + group_thread_id] =
            radix_histogram[group_thread_id];
    }
}

groupshared uint radix_totals[RADIX_SIZE];

[shader("compute")]
[numthreads(RADIX_SIZE, 1, 1)]
void radix_prefix_pass(uint group_thread_id : SV_GroupIndex)
{
    uint digit = group_thread_id;
    uint offset = 0;
    for (uint group = 0; group < sort_constants.group_count; ++group)
    {
        uint index = group * RADIX_SIZE + digit;
        uint count = sort_group_offsets[index];
        sort_group_offsets[index] = offset;
        offset += count;
    }
    radix_totals[digit] = offset;
    GroupMemoryBarrierWithGroupSync();
    uint base = 0;
    for (uint preceding_digit = 0; preceding_digit < digit; ++preceding_digit)
        base += radix_totals[preceding_digit];
    for (uint group = 0; group < sort_constants.group_count; ++group)
        sort_group_offsets[group * RADIX_SIZE + digit] += base;
}

groupshared uint wave_histograms[MAXIMUM_WAVES * RADIX_SIZE];

[shader("compute")]
[numthreads(SORT_THREADS, 1, 1)]
void radix_scatter_pass(
    uint3 group_id : SV_GroupID,
    uint group_thread_id : SV_GroupIndex,
    uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint index = dispatch_thread_id.x;
    bool active = index < sort_constants.count;
    uint key = active ? read_sort_key(index) : 0;
    uint digit = (key >> sort_constants.shift) & (RADIX_SIZE - 1);
    uint lane = WaveGetLaneIndex();
    uint wave = group_thread_id / WaveGetLaneCount();
    uint wave_rank = 0;
    for (uint candidate = 0; candidate < RADIX_SIZE; ++candidate)
    {
        bool selected = active && digit == candidate;
        uint rank = WavePrefixCountBits(selected);
        uint count = WaveActiveCountBits(selected);
        if (selected)
            wave_rank = rank;
        if (lane == 0)
            wave_histograms[wave * RADIX_SIZE + candidate] = count;
    }
    GroupMemoryBarrierWithGroupSync();
    if (!active)
        return;
    uint local_offset = wave_rank;
    for (uint preceding_wave = 0; preceding_wave < wave; ++preceding_wave)
        local_offset += wave_histograms[preceding_wave * RADIX_SIZE + digit];
    uint output_offset =
        sort_group_offsets[group_id.x * RADIX_SIZE + digit] + local_offset;
    write_sort_value(output_offset, key, read_sort_index(index));
}

float3 quaternion_axis_x(float4 quaternion)
{
    float w = quaternion.x;
    float x = quaternion.y;
    float y = quaternion.z;
    float z = quaternion.w;
    return float3(
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + w * z),
        2.0 * (x * z - w * y));
}

float3 quaternion_axis_y(float4 quaternion)
{
    float w = quaternion.x;
    float x = quaternion.y;
    float y = quaternion.z;
    float z = quaternion.w;
    return float3(
        2.0 * (x * y - w * z),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + w * x));
}

float3 quaternion_axis_z(float4 quaternion)
{
    float w = quaternion.x;
    float x = quaternion.y;
    float y = quaternion.z;
    float z = quaternion.w;
    return float3(
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        1.0 - 2.0 * (x * x + y * y));
}

float3 evaluate_sh(uint gaussian_index, GaussianSource gaussian, float3 direction)
{
    uint degree = uint(uniforms.source_center_degree.w);
    float basis[16] = { 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0 };
    basis[0] = SH_C0;
    if (degree >= 1)
    {
        basis[1] = -SH_C1 * direction.y;
        basis[2] = SH_C1 * direction.z;
        basis[3] = -SH_C1 * direction.x;
    }
    if (degree >= 2)
    {
        float x = direction.x;
        float y = direction.y;
        float z = direction.z;
        basis[4] = SH_C2[0] * x * y;
        basis[5] = SH_C2[1] * y * z;
        basis[6] = SH_C2[2] * (2.0 * z * z - x * x - y * y);
        basis[7] = SH_C2[3] * x * z;
        basis[8] = SH_C2[4] * (x * x - y * y);
    }
    if (degree >= 3)
    {
        float x = direction.x;
        float y = direction.y;
        float z = direction.z;
        basis[9] = SH_C3[0] * y * (3.0 * x * x - y * y);
        basis[10] = SH_C3[1] * x * y * z;
        basis[11] = SH_C3[2] * y * (4.0 * z * z - x * x - y * y);
        basis[12] = SH_C3[3] * z * (2.0 * z * z - 3.0 * x * x - 3.0 * y * y);
        basis[13] = SH_C3[4] * x * (4.0 * z * z - x * x - y * y);
        basis[14] = SH_C3[5] * z * (x * x - y * y);
        basis[15] = SH_C3[6] * x * (x * x - 3.0 * y * y);
    }

    float3 color = float3(0.5) + SH_C0 * gaussian.dc.xyz;
    uint coefficient_count = (degree + 1) * (degree + 1) - 1;
    uint gaussian_offset = gaussian_index * coefficient_count * 3;
    for (uint coefficient = 1; coefficient <= coefficient_count; ++coefficient)
    {
        uint offset = coefficient - 1;
        color.x += higher_order_sh[gaussian_offset + offset] * basis[coefficient];
        color.y += higher_order_sh[gaussian_offset + coefficient_count + offset]
            * basis[coefficient];
        color.z += higher_order_sh[gaussian_offset + coefficient_count * 2 + offset]
            * basis[coefficient];
    }
    return max(color * uniforms.orthographic_source.w, float3(0.0));
}

[shader("vertex")]
GaussianVertexOutput gaussian_vertex(uint vertex_id : SV_VertexID, uint instance_id : SV_InstanceID)
{
    static const float2 corners[4] = {
        float2(-1.0, -1.0), float2(-1.0, 1.0),
        float2(1.0, -1.0), float2(1.0, 1.0)
    };
    float projection = uniforms.viewport_projection.w;
    bool wrapped = projection == 2.0 || projection == 3.0;
    bool fisheye = projection == 4.0;
    uint lens_sample_count = max(uint(uniforms.camera_lens.w), 1u);
    uint instance_copies = wrapped ? 3u : lens_sample_count;
    uint sorted_instance = instance_id / instance_copies;
    uint gaussian_index = sorted_indices[sorted_instance];
    GaussianSource gaussian = gaussians[gaussian_index];
    float source_radius = uniforms.orthographic_source.z;
    float3 object_position =
        (gaussian.position_opacity.xyz - uniforms.source_center_degree.xyz) / source_radius;
    float3 camera_position = mul(
        uniforms.object_to_camera,
        float4(object_position, 1.0)).xyz;
    float depth = wrapped || fisheye ? length(camera_position) : -camera_position.z;
    float safe_depth = max(abs(depth), MINIMUM_DEPTH);
    float2 viewport = uniforms.viewport_projection.xy;
    bool perspective = projection == 1.0;

    float2 center;
    if (perspective)
    {
        float focal = uniforms.viewport_projection.z;
        center = float2(
            viewport.x * 0.5 + focal * camera_position.x / safe_depth,
            viewport.y * 0.5 - focal * camera_position.y / safe_depth);
    }
    else if (!wrapped && !fisheye)
    {
        center = float2(
            viewport.x * 0.5
                + viewport.x * camera_position.x / uniforms.orthographic_source.x,
            viewport.y * 0.5
                - viewport.y * camera_position.y / uniforms.orthographic_source.y);
    }
    else if (wrapped)
    {
        center = wrapped_center(camera_position, viewport, projection);
        center.x += (float(instance_id % 3) - 1.0) * viewport.x;
    }
    else
    {
        center = fisheye_center(camera_position, viewport);
    }
    if (perspective && lens_sample_count > 1u)
    {
        float signed_defocus = uniforms.viewport_projection.z * uniforms.camera_lens.y
            * (1.0 / uniforms.camera_lens.x - 1.0 / safe_depth);
        float2 lens_offset = aperture_sample(
            instance_id % lens_sample_count,
            lens_sample_count);
        center += float2(lens_offset.x, -lens_offset.y) * signed_defocus;
    }

    float4 quaternion = normalize(gaussian.rotation);
    float3 scale = exp(clamp(
        gaussian.scale.xyz,
        float3(MINIMUM_LOG_SCALE),
        float3(MAXIMUM_LOG_SCALE))) / source_radius;
    float3 axis_x = mul(
        uniforms.object_to_camera,
        float4(quaternion_axis_x(quaternion) * scale.x, 0.0)).xyz;
    float3 axis_y = mul(
        uniforms.object_to_camera,
        float4(quaternion_axis_y(quaternion) * scale.y, 0.0)).xyz;
    float3 axis_z = mul(
        uniforms.object_to_camera,
        float4(quaternion_axis_z(quaternion) * scale.z, 0.0)).xyz;

    float3 pixel_x;
    float3 pixel_y;
    if (perspective)
    {
        float focal = uniforms.viewport_projection.z;
        float inverse_depth = 1.0 / safe_depth;
        float inverse_depth_squared = inverse_depth * inverse_depth;
        float3 projection_x = float3(
            focal * inverse_depth,
            0.0,
            focal * camera_position.x * inverse_depth_squared);
        float3 projection_y = float3(
            0.0,
            -focal * inverse_depth,
            -focal * camera_position.y * inverse_depth_squared);
        pixel_x = float3(
            dot(projection_x, axis_x),
            dot(projection_x, axis_y),
            dot(projection_x, axis_z));
        pixel_y = float3(
            dot(projection_y, axis_x),
            dot(projection_y, axis_y),
            dot(projection_y, axis_z));
    }
    else if (!wrapped && !fisheye)
    {
        pixel_x = float3(axis_x.x, axis_y.x, axis_z.x)
            * viewport.x / uniforms.orthographic_source.x;
        pixel_y = -float3(axis_x.y, axis_y.y, axis_z.y)
            * viewport.y / uniforms.orthographic_source.y;
    }
    else if (wrapped)
    {
        float2 base_center = wrapped_center(camera_position, viewport, projection);
        float2 axis_x_delta = wrapped_delta(
            wrapped_center(camera_position + axis_x, viewport, projection),
            base_center,
            viewport.x);
        float2 axis_y_delta = wrapped_delta(
            wrapped_center(camera_position + axis_y, viewport, projection),
            base_center,
            viewport.x);
        float2 axis_z_delta = wrapped_delta(
            wrapped_center(camera_position + axis_z, viewport, projection),
            base_center,
            viewport.x);
        pixel_x = float3(axis_x_delta.x, axis_y_delta.x, axis_z_delta.x);
        pixel_y = float3(axis_x_delta.y, axis_y_delta.y, axis_z_delta.y);
    }
    else
    {
        float2 axis_x_delta = fisheye_center(camera_position + axis_x, viewport)
            - center;
        float2 axis_y_delta = fisheye_center(camera_position + axis_y, viewport)
            - center;
        float2 axis_z_delta = fisheye_center(camera_position + axis_z, viewport)
            - center;
        pixel_x = float3(axis_x_delta.x, axis_y_delta.x, axis_z_delta.x);
        pixel_y = float3(axis_x_delta.y, axis_y_delta.y, axis_z_delta.y);
    }
    float covariance_xx = dot(pixel_x, pixel_x) + COVARIANCE_LOW_PASS;
    float covariance_xy = dot(pixel_x, pixel_y);
    float covariance_yy = dot(pixel_y, pixel_y) + COVARIANCE_LOW_PASS;
    float determinant = covariance_xx * covariance_yy - covariance_xy * covariance_xy;
    float safe_determinant = isfinite(determinant) && determinant > 0.0
        ? determinant
        : 1.0;
    float opacity = 1.0 / (
        1.0 + exp(-clamp(gaussian.position_opacity.w, -80.0, 80.0)));
    float sample_opacity = 1.0 - pow(
        max(1.0 - opacity, 0.0),
        1.0 / float(lens_sample_count));
    float2 radius = SIGMA_EXTENT * sqrt(float2(covariance_xx, covariance_yy));
    bool visible = depth > MINIMUM_DEPTH
        && (!fisheye || fisheye_angle(camera_position) <= uniforms.camera_lens.z)
        && isfinite(determinant)
        && determinant > 0.0
        && center.x + radius.x > 0.0
        && center.y + radius.y > 0.0
        && center.x - radius.x < viewport.x
        && center.y - radius.y < viewport.y;

    float2 corner = corners[vertex_id];
    float2 pixel = center + corner * radius;
    float3 direction = normalize(object_position - uniforms.camera_in_object.xyz);
    GaussianVertexOutput output;
    output.position = visible
        ? float4(
            pixel.x * 2.0 / viewport.x - 1.0,
            pixel.y * 2.0 / viewport.y - 1.0,
            0.0,
            1.0)
        : float4(2.0, 2.0, 0.0, 1.0);
    output.delta = corner * radius;
    output.conic_opacity = float4(
        covariance_yy / safe_determinant,
        -covariance_xy / safe_determinant,
        covariance_xx / safe_determinant,
        sample_opacity);
    output.color_depth = float4(
        visible ? evaluate_sh(gaussian_index, gaussian, direction) : float3(0.0),
        depth);
    return output;
}

[shader("fragment")]
float4 gaussian_fragment(GaussianVertexOutput input) : SV_Target
{
    if (uniforms.viewport_projection.w == 4.0
        && length(input.position.xy - uniforms.viewport_projection.xy * 0.5)
            > min(uniforms.viewport_projection.x, uniforms.viewport_projection.y) * 0.5)
        discard;
    float power = -0.5 * (
        input.conic_opacity.x * input.delta.x * input.delta.x
        + 2.0 * input.conic_opacity.y * input.delta.x * input.delta.y
        + input.conic_opacity.z * input.delta.y * input.delta.y);
    if (power > 0.0)
        discard;
    float alpha = min(MAXIMUM_ALPHA, input.conic_opacity.w * exp(power));
    if (alpha < MINIMUM_ALPHA / max(uniforms.camera_lens.w, 1.0))
        discard;
    return float4(input.color_depth.rgb * alpha, alpha);
}

[shader("vertex")]
ResolveVertexOutput resolve_vertex(uint vertex_id : SV_VertexID)
{
    float2 corner = float2(float((vertex_id << 1) & 2), float(vertex_id & 2));
    ResolveVertexOutput output;
    output.position = float4(corner * 2.0 - 1.0, 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 resolve_fragment(ResolveVertexOutput input) : SV_Target
{
    int3 coordinate = int3(int2(input.position.xy), 0);
    float4 accumulation = accumulation_texture.Load(coordinate);
    return accumulation.a > 0.0
        ? float4(accumulation.rgb / accumulation.a, accumulation.a)
        : float4(0.0);
}
