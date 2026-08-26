static const uint OUTLINE_NOISE_X_CHANNEL = 0x68bc21ebu;
static const uint OUTLINE_NOISE_Y_CHANNEL = 0x02e5be93u;
static const uint OUTLINE_NOISE_WIDTH_CHANNEL = 0x967a889bu;
static const uint OUTLINE_NOISE_HASH_X = 0x9e3779b9u;
static const uint OUTLINE_NOISE_HASH_Y = 0x85ebca6bu;
static const uint OUTLINE_NOISE_HASH_Z = 0xc2b2ae35u;
static const uint OUTLINE_NOISE_HASH_A = 0x7feb352du;
static const uint OUTLINE_NOISE_HASH_B = 0x846ca68bu;
static const float OUTLINE_UINT_TO_UNIT_FLOAT = 1.0 / 4294967296.0;
static const float OUTLINE_DETECTION_THRESHOLD = 0.1;
static const int MAXIMUM_OUTLINE_WIDTH = 16;
static const int OUTLINE_CONTINUATION_STEPS = 3;
static const float OUTLINE_EDGE_PEAK_RATIO = 2.0;

float binary_outline_coverage(float coverage)
{
    return coverage >= OUTLINE_DETECTION_THRESHOLD / scene.toon.outline_aggressiveness
        ? 1.0
        : 0.0;
}

float4 surface_outline_color()
{
    return scene.toon.outline_color;
}

float outline_noise_value(int3 cell, uint channel)
{
    uint state = asuint(cell.x) * OUTLINE_NOISE_HASH_X
        ^ asuint(cell.y) * OUTLINE_NOISE_HASH_Y
        ^ asuint(cell.z) * OUTLINE_NOISE_HASH_Z
        ^ scene.toon.outline_noise_seed
        ^ channel;
    state ^= state >> 16;
    state *= OUTLINE_NOISE_HASH_A;
    state ^= state >> 15;
    state *= OUTLINE_NOISE_HASH_B;
    state ^= state >> 16;
    return float(state) * OUTLINE_UINT_TO_UNIT_FLOAT;
}

float outline_noise(float2 position, float frequency, uint channel)
{
    float3 point = float3(
        position * frequency,
        scene.toon.outline_noise_evolution);
    int3 cell = int3(floor(point));
    float3 blend = frac(point);
    blend = blend * blend * (3.0 - 2.0 * blend);
    float lower = lerp(
        lerp(
            outline_noise_value(cell, channel),
            outline_noise_value(cell + int3(1, 0, 0), channel),
            blend.x),
        lerp(
            outline_noise_value(cell + int3(0, 1, 0), channel),
            outline_noise_value(cell + int3(1, 1, 0), channel),
            blend.x),
        blend.y);
    float upper = lerp(
        lerp(
            outline_noise_value(cell + int3(0, 0, 1), channel),
            outline_noise_value(cell + int3(1, 0, 1), channel),
            blend.x),
        lerp(
            outline_noise_value(cell + int3(0, 1, 1), channel),
            outline_noise_value(cell + int3(1, 1, 1), channel),
            blend.x),
        blend.y);
    return lerp(lower, upper, blend.z) * 2.0 - 1.0;
}

void outline_variation(float2 position, out float2 offset, out float width)
{
    offset = float2(0.0);
    width = 0.0;
    if (scene.toon.outline_offset_variation > 0.0)
    {
        offset = scene.toon.outline_offset_variation * float2(
            outline_noise(
                position,
                scene.toon.outline_frequency.x,
                OUTLINE_NOISE_X_CHANNEL),
            outline_noise(
                position,
                scene.toon.outline_frequency.x,
                OUTLINE_NOISE_Y_CHANNEL));
    }
    if (scene.toon.outline_width_variation > 0.0)
    {
        width = scene.toon.outline_width_variation
            * outline_noise(
                position,
                scene.toon.outline_frequency.y,
                OUTLINE_NOISE_WIDTH_CHANNEL);
    }
}

bool outline_guide_hit(float4 guide)
{
    return guide.w < RAY_MAXIMUM;
}

float4 sample_outline_guide(float2 position, uint2 dimensions)
{
    if (any(position < float2(0.0)) || any(position >= float2(dimensions)))
        return float4(0.0, 0.0, 0.0, RAY_MAXIMUM);
    return outline_guide[uint2(position)];
}

bool directional_outline_crease(
    float2 position,
    float2 direction,
    uint2 dimensions,
    float depth_threshold,
    float normal_threshold)
{
    float4 center = sample_outline_guide(position, dimensions);
    float4 neighbor = sample_outline_guide(position + direction, dimensions);
    if (!outline_guide_hit(center) || !outline_guide_hit(neighbor))
        return false;

    float depth_change = abs(center.w - neighbor.w)
        / max(min(center.w, neighbor.w), RAY_MINIMUM);
    float normal_change = 1.0 - dot(center.xyz, neighbor.xyz);
    if (depth_change <= depth_threshold && normal_change <= normal_threshold)
        return false;

    float continuation_depth = 0.0;
    float continuation_normal = 0.0;
    float4 previous = neighbor;
    for (int step = 2; step <= OUTLINE_CONTINUATION_STEPS + 1; ++step)
    {
        float4 following = sample_outline_guide(position + direction * step, dimensions);
        if (!outline_guide_hit(following))
            break;
        continuation_depth = max(
            continuation_depth,
            abs(previous.w - following.w)
                / max(min(previous.w, following.w), RAY_MINIMUM));
        continuation_normal = max(
            continuation_normal,
            1.0 - dot(previous.xyz, following.xyz));
        previous = following;
    }

    return (depth_change > depth_threshold
            && depth_change > continuation_depth * OUTLINE_EDGE_PEAK_RATIO)
        || (normal_change > normal_threshold
            && normal_change > continuation_normal * OUTLINE_EDGE_PEAK_RATIO);
}

bool outline_crease_seed(float2 position, uint2 dimensions)
{
    float depth_threshold = scene.toon.outline_depth_threshold
        / scene.toon.outline_aggressiveness;
    float normal_threshold = (1.0 - scene.toon.outline_normal_cosine)
        / scene.toon.outline_aggressiveness;
    for (int axis = 0; axis < 2; ++axis)
    {
        for (int direction = -1; direction <= 1; direction += 2)
        {
            float2 offset = axis == 0
                ? float2(direction, 0.0)
                : float2(0.0, direction);
            if (directional_outline_crease(
                    position,
                    offset,
                    dimensions,
                    depth_threshold,
                    normal_threshold))
                return true;
        }
    }
    return false;
}

float2 horizontal_outline_distances(
    float2 position,
    uint2 dimensions,
    OutlineMode mode)
{
    float region_distance = RAY_MAXIMUM;
    float crease_distance = RAY_MAXIMUM;
    int maximum_width = min(
        MAXIMUM_OUTLINE_WIDTH,
        int(ceil(scene.toon.outline_width + scene.toon.outline_width_variation)));
    for (int x = -maximum_width; x <= maximum_width; ++x)
    {
        float2 candidate_position = position + float2(x, 0.0);
        float4 candidate = sample_outline_guide(candidate_position, dimensions);
        if (!outline_guide_hit(candidate))
            continue;
        float distance = abs(float(x));
        region_distance = min(region_distance, distance);
        if (mode == OutlineMode::SilhouetteAndCreases
            && outline_crease_seed(candidate_position, dimensions))
            crease_distance = min(crease_distance, distance);
    }
    return float2(region_distance, crease_distance);
}

float2 sample_outline_distance(float2 position, uint2 dimensions)
{
    if (any(position < float2(0.0)) || any(position >= float2(dimensions)))
        return float2(RAY_MAXIMUM);
    return outline_distance[uint2(position)];
}

RayPayload region_outline_surface(
    float2 center_position,
    uint2 dimensions,
    float width)
{
    RayPayload center = trace_surface(center_position, dimensions, true, 0);
    if (center.hit != 0)
        return center;

    float nearest_distance = RAY_MAXIMUM;
    float2 nearest_offset = float2(0.0);
    int vertical_extent = min(MAXIMUM_OUTLINE_WIDTH, int(ceil(width)));
    for (int y = -vertical_extent; y <= vertical_extent; ++y)
    {
        float horizontal = sample_outline_distance(
            center_position + float2(0.0, y),
            dimensions).x;
        float distance = length(float2(horizontal, y));
        if (distance < nearest_distance)
        {
            nearest_distance = distance;
            nearest_offset = float2(horizontal, y);
        }
    }
    if (nearest_distance == RAY_MAXIMUM)
        return center;

    RayPayload surface = trace_surface(
        center_position + nearest_offset,
        dimensions,
        true,
        0);
    if (surface.hit != 0 || nearest_offset.x == 0.0)
        return surface;
    return trace_surface(
        center_position + float2(-nearest_offset.x, nearest_offset.y),
        dimensions,
        true,
        0);
}

float region_boundary_outline(
    float2 center_position,
    uint2 dimensions,
    OutlineMode mode,
    float width)
{
    bool center_hit = outline_guide_hit(sample_outline_guide(center_position, dimensions));
    float region_distance = RAY_MAXIMUM;
    float crease_distance = RAY_MAXIMUM;
    float crease_radius = width * 0.5;
    int vertical_extent = min(MAXIMUM_OUTLINE_WIDTH, int(ceil(width)));
    for (int y = -vertical_extent; y <= vertical_extent; ++y)
    {
        float vertical_distance = abs(float(y));
        float2 horizontal = sample_outline_distance(
            center_position + float2(0.0, y),
            dimensions);
        if (!center_hit && vertical_distance <= width)
            region_distance = min(
                region_distance,
                length(float2(horizontal.x, vertical_distance)));
        if (center_hit
            && mode == OutlineMode::SilhouetteAndCreases
            && vertical_distance <= crease_radius)
            crease_distance = min(
                crease_distance,
                length(float2(horizontal.y, vertical_distance)));
    }

    // Region samples are pixel centers, so the actual boundary lies half a pixel
    // before the nearest hit. Creases are centered and use half the requested width.
    bool silhouette = !center_hit && region_distance <= width + 0.5;
    bool crease = center_hit
        && mode == OutlineMode::SilhouetteAndCreases
        && crease_distance <= crease_radius;
    return silhouette || crease ? 1.0 : 0.0;
}

float geometry_edge(RayPayload center, RayPayload neighbor, OutlineMode mode)
{
    if (center.hit != neighbor.hit)
        return 1.0;
    if (center.hit == 0 || mode != OutlineMode::SilhouetteAndCreases)
        return 0.0;
    float relative_depth = abs(center.depth - neighbor.depth)
        / max(min(center.depth, neighbor.depth), RAY_MINIMUM);
    return relative_depth > scene.toon.outline_depth_threshold
        || dot(center.normal, neighbor.normal) < scene.toon.outline_normal_cosine
        ? 1.0
        : 0.0;
}

float radial_outline(
    float2 center_position,
    uint2 dimensions,
    RayPayload center,
    OutlineMode mode,
    float radius,
    uint probe_count)
{
    float edges = 0.0;
    for (uint probe_index = 0; probe_index < probe_count; ++probe_index)
    {
        float angle = 2.0 * PI * float(probe_index) / float(probe_count);
        float2 offset = float2(cos(angle), sin(angle)) * radius;
        RayPayload neighbor = trace_surface(
            center_position + offset,
            dimensions,
            true,
            probe_index);
        edges += geometry_edge(center, neighbor, mode);
    }
    return saturate(edges * 2.0 / float(probe_count));
}

float relative_depth(RayPayload center, RayPayload sample)
{
    return center.hit != 0 && sample.hit != 0
        ? (sample.depth - center.depth) / max(center.depth, RAY_MINIMUM)
        : 0.0;
}

float3 sampled_normal(RayPayload center, RayPayload sample)
{
    return center.hit != 0 && sample.hit != 0 ? sample.normal : center.normal;
}

float threshold_response(float value, float threshold)
{
    return saturate((value - threshold) / max(threshold, RAY_MINIMUM));
}

float sobel_outline(
    float2 center_position,
    uint2 dimensions,
    RayPayload center,
    OutlineMode mode,
    float radius)
{
    static const float2 offsets[8] = {
        float2(-1.0, -1.0), float2(0.0, -1.0), float2(1.0, -1.0),
        float2(-1.0, 0.0), float2(1.0, 0.0),
        float2(-1.0, 1.0), float2(0.0, 1.0), float2(1.0, 1.0)
    };
    static const float kernel_x[8] = { -1.0, 0.0, 1.0, -2.0, 2.0, -1.0, 0.0, 1.0 };
    static const float kernel_y[8] = { -1.0, -2.0, -1.0, 0.0, 0.0, 1.0, 2.0, 1.0 };
    float2 hit_gradient = float2(0.0);
    float2 depth_gradient = float2(0.0);
    float3 normal_gradient_x = float3(0.0);
    float3 normal_gradient_y = float3(0.0);
    for (uint index = 0; index < 8; ++index)
    {
        RayPayload neighbor = trace_surface(
            center_position + offsets[index] * radius,
            dimensions,
            true,
            index);
        float hit = neighbor.hit != 0 ? 1.0 : 0.0;
        float depth = relative_depth(center, neighbor);
        float3 normal = sampled_normal(center, neighbor);
        hit_gradient += float2(kernel_x[index], kernel_y[index]) * hit;
        depth_gradient += float2(kernel_x[index], kernel_y[index]) * depth;
        normal_gradient_x += normal * kernel_x[index];
        normal_gradient_y += normal * kernel_y[index];
    }
    float response = saturate(length(hit_gradient) / 4.0);
    if (center.hit != 0 && mode == OutlineMode::SilhouetteAndCreases)
    {
        float depth_response = threshold_response(
            length(depth_gradient) / 4.0,
            scene.toon.outline_depth_threshold);
        float normal_threshold = sqrt(max(
            2.0 - 2.0 * scene.toon.outline_normal_cosine,
            RAY_MINIMUM));
        float normal_response = threshold_response(
            sqrt(
                dot(normal_gradient_x, normal_gradient_x)
                + dot(normal_gradient_y, normal_gradient_y)) / 4.0,
            normal_threshold);
        response = max(response, max(depth_response, normal_response));
    }
    return response;
}

float roberts_cross_outline(
    float2 center_position,
    uint2 dimensions,
    RayPayload center,
    OutlineMode mode,
    float radius)
{
    RayPayload right = trace_surface(
        center_position + float2(radius, 0.0),
        dimensions,
        true,
        0);
    RayPayload down = trace_surface(
        center_position + float2(0.0, radius),
        dimensions,
        true,
        1);
    RayPayload diagonal = trace_surface(
        center_position + float2(radius, radius),
        dimensions,
        true,
        2);
    float hit_x = (center.hit != 0 ? 1.0 : 0.0) - (diagonal.hit != 0 ? 1.0 : 0.0);
    float hit_y = (right.hit != 0 ? 1.0 : 0.0) - (down.hit != 0 ? 1.0 : 0.0);
    float response = saturate(length(float2(hit_x, hit_y)));
    if (center.hit != 0 && mode == OutlineMode::SilhouetteAndCreases)
    {
        float depth_x = -relative_depth(center, diagonal);
        float depth_y = relative_depth(center, right) - relative_depth(center, down);
        float depth_response = threshold_response(
            length(float2(depth_x, depth_y)),
            scene.toon.outline_depth_threshold);
        float3 normal_x = center.normal - sampled_normal(center, diagonal);
        float3 normal_y = sampled_normal(center, right) - sampled_normal(center, down);
        float normal_threshold = sqrt(max(
            2.0 - 2.0 * scene.toon.outline_normal_cosine,
            RAY_MINIMUM));
        float normal_response = threshold_response(
            sqrt(dot(normal_x, normal_x) + dot(normal_y, normal_y)),
            normal_threshold);
        response = max(response, max(depth_response, normal_response));
    }
    return response;
}

float difference_of_gaussians_outline(
    float2 center_position,
    uint2 dimensions,
    RayPayload center,
    OutlineMode mode,
    float inner_sigma,
    uint probe_count)
{
    float inner_hit = 0.0;
    float outer_hit = 0.0;
    float inner_depth = 0.0;
    float outer_depth = 0.0;
    float3 inner_normal = float3(0.0);
    float3 outer_normal = float3(0.0);
    float inner_weight_sum = 0.0;
    float outer_weight_sum = 0.0;
    float outer_sigma = inner_sigma * scene.toon.dog_radius_ratio;
    for (uint ring = 0; ring < DOG_RING_COUNT; ++ring)
    {
        float radius = 3.0 * outer_sigma
            * (float(ring) + 0.5) / float(DOG_RING_COUNT);
        float inner_weight = radius
            * exp(-0.5 * radius * radius / (inner_sigma * inner_sigma));
        float outer_weight = radius
            * exp(-0.5 * radius * radius / (outer_sigma * outer_sigma));
        for (uint probe_index = 0; probe_index < probe_count; ++probe_index)
        {
            float angle = 2.0 * PI * (
                float(probe_index) / float(probe_count)
                + float(ring) / float(DOG_RING_COUNT * probe_count));
            float2 direction = float2(cos(angle), sin(angle));
            RayPayload neighbor = trace_surface(
                center_position + direction * radius,
                dimensions,
                true,
                ring * probe_count + probe_index);
            float hit = neighbor.hit != 0 ? 1.0 : 0.0;
            float depth = relative_depth(center, neighbor);
            float3 normal = sampled_normal(center, neighbor);
            inner_hit += hit * inner_weight;
            outer_hit += hit * outer_weight;
            inner_depth += depth * inner_weight;
            outer_depth += depth * outer_weight;
            inner_normal += normal * inner_weight;
            outer_normal += normal * outer_weight;
            inner_weight_sum += inner_weight;
            outer_weight_sum += outer_weight;
        }
    }
    float response = center.hit != 0
        ? inner_hit / inner_weight_sum - outer_hit / outer_weight_sum
        : 0.0;
    if (center.hit != 0 && mode == OutlineMode::SilhouetteAndCreases)
    {
        float depth_response = threshold_response(
            abs(inner_depth / inner_weight_sum - outer_depth / outer_weight_sum),
            scene.toon.outline_depth_threshold);
        float normal_threshold = sqrt(max(
            2.0 - 2.0 * scene.toon.outline_normal_cosine,
            RAY_MINIMUM));
        float normal_response = threshold_response(
            length(
                inner_normal / inner_weight_sum
                - outer_normal / outer_weight_sum),
            normal_threshold);
        response = max(response, max(depth_response, normal_response));
    }
    return 1.0 - exp(
        -scene.toon.dog_sharpness
        * max(response - scene.toon.dog_threshold, 0.0));
}
