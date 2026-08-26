static const uint INTERACTIVE_MINIMUM_SAMPLES = 2u;
static const uint INTERACTIVE_SAMPLE_DIVISOR = 8u;

void adaptive_path_budget(
    out uint minimum_samples,
    out uint maximum_samples,
    out float relative_error)
{
    uint requested = max(uint(scene.pbr.path_tracing), 1u);
    if (scene.pbr.render_quality == RenderQuality::Interactive)
    {
        minimum_samples = INTERACTIVE_MINIMUM_SAMPLES;
        maximum_samples = max(requested / INTERACTIVE_SAMPLE_DIVISOR, minimum_samples);
        relative_error = INTERACTIVE_RELATIVE_ERROR;
        return;
    }
    minimum_samples = requested;
    maximum_samples = requested * 2u;
    relative_error = FINAL_RELATIVE_ERROR;
}

bool adaptive_converged(float4 mean, float4 squared_error, uint sample_count, float relative_error)
{
    if (sample_count < MINIMUM_VARIANCE_SAMPLES)
        return false;
    float4 variance = squared_error / float(sample_count - 1u);
    float color_standard_error = sqrt(
        max(max(variance.r, max(variance.g, variance.b)), 0.0) / float(sample_count));
    float alpha_standard_error = sqrt(max(variance.a, 0.0) / float(sample_count));
    float color_scale = max(
        max(abs(mean.r), max(abs(mean.g), abs(mean.b))),
        MINIMUM_ADAPTIVE_MEAN);
    return color_standard_error <= relative_error * color_scale
        && alpha_standard_error <= relative_error;
}

float2 anti_aliasing_offset(uint index, AntiAliasing strategy, uint2 pixel)
{
    if (strategy == AntiAliasing::RotatedGrid2x)
        return index == 0 ? float2(-0.25, 0.25) : float2(0.25, -0.25);
    if (strategy == AntiAliasing::Grid4x)
    {
        return float2(
            (index & 1) == 0 ? -0.25 : 0.25,
            (index & 2) == 0 ? -0.25 : 0.25);
    }
    if (strategy == AntiAliasing::Stochastic8x)
        return stochastic_pixel_offset(index, 8, pixel);
    return float2(0.0);
}

RayPayload trace_surface(float2 sample_position, uint2 dimensions, bool probe, uint random_seed)
{
    RayPayload payload = {
        float4(0.0),
        float4(0.0),
        float3(0.0),
        RAY_MAXIMUM,
        RAY_MAXIMUM,
        0.0,
        0,
        probe ? 1u : 0u,
        0,
        random_seed,
        0,
        0xffffffffu
    };
    float2 uv = sample_position / float2(dimensions);
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    float3 world_origin;
    float3 world_direction;
    float3 pinhole_direction;
    float3 ground_origin;
    CameraProjection camera_projection = CameraProjection(uint(scene.camera_lens.w));
    switch (camera_projection)
    {
    case CameraProjection::Equirectangular:
        {
            float longitude = ndc.x * PI;
            float latitude = ndc.y * PI * 0.5;
            float latitude_radius = cos(latitude);
            float3 camera_direction = float3(
                sin(longitude) * latitude_radius,
                sin(latitude),
                -cos(longitude) * latitude_radius);
            world_origin = scene.camera_position.xyz;
            world_direction = normalize(mul(
                scene.camera_world,
                float4(camera_direction, 0.0)).xyz);
            pinhole_direction = world_direction;
            break;
        }
    case CameraProjection::Cylindrical:
        {
            float longitude = ndc.x * PI;
            float3 camera_direction = normalize(float3(
                sin(longitude),
                ndc.y * scene.camera_lens.z,
                -cos(longitude)));
            world_origin = scene.camera_position.xyz;
            world_direction = normalize(mul(
                scene.camera_world,
                float4(camera_direction, 0.0)).xyz);
            pinhole_direction = world_direction;
            break;
        }
    case CameraProjection::Fisheye:
        {
            float minimum_dimension = float(min(dimensions.x, dimensions.y));
            float2 lens_position = ndc * float2(dimensions) / minimum_dimension;
            float lens_radius = length(lens_position);
            if (lens_radius > 1.0)
                return payload;
            float angle = lens_radius * scene.camera_lens.z;
            float2 lens_direction = lens_radius > RAY_MINIMUM
                ? lens_position / lens_radius
                : float2(0.0);
            float3 camera_direction = float3(
                lens_direction * sin(angle),
                -cos(angle));
            world_origin = scene.camera_position.xyz;
            world_direction = normalize(mul(
                scene.camera_world,
                float4(camera_direction, 0.0)).xyz);
            pinhole_direction = world_direction;
            break;
        }
    case CameraProjection::Perspective:
    case CameraProjection::Orthographic:
    default:
        {
            float4 near_point = mul(
                scene.inverse_view_projection,
                float4(ndc, 0.0, 1.0));
            float4 far_point = mul(
                scene.inverse_view_projection,
                float4(ndc, 1.0, 1.0));
            world_origin = near_point.xyz / near_point.w;
            float3 far_position = far_point.xyz / far_point.w;
            world_direction = normalize(far_position - world_origin);
            pinhole_direction = normalize(far_position - scene.camera_position.xyz);
            break;
        }
    }
    float3 camera_forward = normalize(
        mul(scene.camera_world, float4(0.0, 0.0, -1.0, 0.0)).xyz);
    float camera_plane_distance = dot(
        world_origin - scene.camera_position.xyz,
        camera_forward) / max(dot(world_direction, camera_forward), RAY_MINIMUM);
    ground_origin = world_origin - world_direction * camera_plane_distance;
    if (!probe && scene.camera_lens.y > 0.0)
    {
        float3 pinhole_origin = scene.camera_position.xyz;
        float focus_t = scene.camera_lens.x
            / max(dot(pinhole_direction, camera_forward), RAY_MINIMUM);
        float3 focus_position = pinhole_origin + pinhole_direction * focus_t;
        float3 lens_tangent;
        float3 lens_bitangent;
        tangent_frame(camera_forward, lens_tangent, lens_bitangent);
        uint lens_seed = random_seed;
        float2 lens_sample = random_disk(lens_seed) * scene.camera_lens.y;
        world_origin = pinhole_origin
            + lens_tangent * lens_sample.x
            + lens_bitangent * lens_sample.y;
        ground_origin = world_origin;
        world_direction = normalize(focus_position - world_origin);
    }

    RayDesc ray;
    ray.Origin = mul(scene.model_inverse, float4(world_origin, 1.0)).xyz;
    ray.Direction = normalize(mul(scene.model_inverse, float4(world_direction, 0.0)).xyz);
    ray.TMin = RAY_MINIMUM;
    ray.TMax = RAY_MAXIMUM;
    TraceRay(
        scene_acceleration,
        RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES,
        0xff,
        0,
        0,
        0,
        ray,
        payload);
    if (!probe)
        composite_virtual_surfaces(payload, ground_origin, world_direction, random_seed);
    return payload;
}
