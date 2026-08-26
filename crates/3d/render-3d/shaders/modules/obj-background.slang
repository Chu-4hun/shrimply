float2 equirectangular_uv(float3 direction)
{
    float3 rotated = normalize(mul(
        scene.environment_inverse_rotation,
        float4(direction, 0.0)).xyz);
    float longitude = atan2(rotated.z, rotated.x);
    return float2(
        frac(longitude / (2.0 * PI) + 0.5),
        acos(clamp(rotated.y, -1.0, 1.0)) / PI);
}

float4 sample_composite_texture(float2 uv, float lod)
{
    float4 sampled = transmission_background_texture.SampleLevel(
        environment_sampler,
        uv,
        lod);
    return float4(
        float3(
            srgb_to_linear(sampled.r),
            srgb_to_linear(sampled.g),
            srgb_to_linear(sampled.b)) * sampled.a,
        sampled.a);
}

float2 composite_world_uv(float3 direction)
{
    float3 rotated = normalize(mul(
        scene.environment_inverse_rotation,
        float4(direction, 0.0)).xyz);
    float3 right = normalize(mul(
        scene.camera_world,
        float4(1.0, 0.0, 0.0, 0.0)).xyz);
    float3 up = normalize(mul(
        scene.camera_world,
        float4(0.0, 1.0, 0.0, 0.0)).xyz);
    float3 forward = normalize(mul(
        scene.camera_world,
        float4(0.0, 0.0, -1.0, 0.0)).xyz);
    float3 local = float3(
        dot(rotated, right),
        dot(rotated, up),
        dot(rotated, forward));
    float2 hemisphere = float2(
        atan2(local.x, abs(local.z)) * 2.0 / PI,
        atan2(local.y, length(local.xz)) * 2.0 / PI);
    return float2(
        hemisphere.x * 0.5 + 0.5,
        0.5 - hemisphere.y * 0.5);
}

bool environment_enabled()
{
    return scene.environment.source == EnvironmentSource::Black
        || scene.environment.source == EnvironmentSource::Image
        || (scene.environment.source == EnvironmentSource::Composite
            && scene.transmission_background.y != 0.0)
        || (scene.transmission_background.y != 0.0
            && (scene.transmission_background.w != 0.0
                || scene.ground_count != 0));
}

bool intersect_ground(
    Ground ground,
    float3 world_origin,
    float3 world_direction,
    out float distance,
    out float3 position,
    out float3 normal)
{
    normal = normalize(ground.normal_composite.xyz);
    float denominator = dot(world_direction, normal);
    distance = RAY_MAXIMUM;
    position = float3(0.0);
    if (abs(denominator) <= RAY_MINIMUM)
        return false;
    distance = dot(ground.position_opacity.xyz - world_origin, normal) / denominator;
    if (distance <= RAY_MINIMUM)
        return false;
    position = world_origin + world_direction * distance;
    if (ground.settings.w != 0.0)
    {
        float3 local = position - ground.position_opacity.xyz;
        float half_size = ground.basis_x_size.w * 0.5;
        if (abs(dot(local, normalize(ground.basis_x_size.xyz))) > half_size
            || abs(dot(local, normalize(ground.basis_z_intensity.xyz))) > half_size)
            return false;
    }
    return true;
}

bool composed_background_uv(float3 world_position, out float2 uv)
{
    uv = float2(0.0);
    float3 direction = normalize(world_position - scene.camera_position.xyz);
    float3 right = normalize(mul(
        scene.camera_world,
        float4(1.0, 0.0, 0.0, 0.0)).xyz);
    float3 up = normalize(mul(
        scene.camera_world,
        float4(0.0, 1.0, 0.0, 0.0)).xyz);
    float3 forward = normalize(mul(
        scene.camera_world,
        float4(0.0, 0.0, -1.0, 0.0)).xyz);
    float3 camera_direction = float3(
        dot(direction, right),
        dot(direction, up),
        dot(direction, forward));
    float2 ndc;
    CameraProjection projection = CameraProjection(uint(scene.camera_lens.w));
    switch (projection)
    {
    case CameraProjection::Equirectangular:
        {
            float longitude = atan2(camera_direction.x, camera_direction.z);
            float latitude = asin(clamp(camera_direction.y, -1.0, 1.0));
            ndc = float2(longitude / PI, latitude * 2.0 / PI);
            break;
        }
    case CameraProjection::Cylindrical:
        {
            float horizontal = length(camera_direction.xz);
            if (horizontal <= RAY_MINIMUM)
                return false;
            float longitude = atan2(camera_direction.x, camera_direction.z);
            ndc = float2(
                longitude / PI,
                camera_direction.y / (horizontal * scene.camera_lens.z));
            break;
        }
    case CameraProjection::Fisheye:
        {
            float angle = acos(clamp(camera_direction.z, -1.0, 1.0));
            float lens_radius = angle / scene.camera_lens.z;
            if (lens_radius > 1.0)
                return false;
            float transverse = length(camera_direction.xy);
            float2 lens_direction = transverse > RAY_MINIMUM
                ? camera_direction.xy / transverse
                : float2(0.0);
            uint width;
            uint height;
            transmission_background_texture.GetDimensions(width, height);
            float minimum_dimension = float(min(width, height));
            ndc = lens_direction * lens_radius * minimum_dimension
                / float2(width, height);
            break;
        }
    case CameraProjection::Perspective:
    case CameraProjection::Orthographic:
    default:
        {
            float4 clip = mul(
                scene.view_projection,
                float4(world_position, 1.0));
            if (clip.w <= RAY_MINIMUM)
                return false;
            ndc = clip.xy / clip.w;
            break;
        }
    }

    uv = float2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    uint address_mode = uint(scene.transmission_background.z);
    if (address_mode == 0)
        uv = saturate(uv);
    else if (address_mode == 1)
        uv = frac(uv);
    else
        uv = 1.0 - abs(frac(uv * 0.5) * 2.0 - 1.0);
    return true;
}

bool sample_transmission_background_at_lod(
    float3 world_position,
    float lod,
    out float4 background)
{
    background = float4(0.0);
    if (scene.transmission_background.y == 0.0)
        return false;
    float2 uv;
    if (!composed_background_uv(world_position, uv))
        return false;
    background = sample_composite_texture(uv, lod);
    return true;
}

bool sample_transmission_background_at(float3 world_position, out float4 background)
{
    return sample_transmission_background_at_lod(world_position, 0.0, background);
}

bool sample_composed_planes(
    float3 world_origin,
    float3 world_direction,
    float lod,
    out float4 color,
    out float distance,
    out int ground_index)
{
    color = float4(0.0);
    distance = RAY_MAXIMUM;
    ground_index = -1;
    if (scene.transmission_background.y == 0.0)
        return false;

    for (uint index = 0; index < scene.ground_count; ++index)
    {
        Ground ground = scene.grounds[index];
        if (ground.normal_composite.w == 0.0)
            continue;
        float ground_distance;
        float3 ground_position;
        float3 ground_normal;
        if (intersect_ground(
            ground,
            world_origin,
            world_direction,
            ground_distance,
            ground_position,
            ground_normal)
            && ground_distance < distance)
        {
            float4 ground_color;
            if (sample_transmission_background_at_lod(
                ground_position,
                lod,
                ground_color))
            {
                ground_color.rgb *= ground.basis_z_intensity.w;
                color = ground_color;
                distance = ground_distance;
                ground_index = int(index);
            }
        }
    }

    if (scene.transmission_background.w != 0.0)
    {
        float3 camera_forward = normalize(mul(
            scene.camera_world,
            float4(0.0, 0.0, -1.0, 0.0)).xyz);
        float background_denominator = dot(world_direction, camera_forward);
        if (abs(background_denominator) > RAY_MINIMUM)
        {
            float3 background_center = scene.camera_position.xyz
                + camera_forward * scene.transmission_background.x;
            float background_distance = dot(
                background_center - world_origin,
                camera_forward) / background_denominator;
            float4 background_color;
            if (background_distance > RAY_MINIMUM
                && background_distance < distance
                && sample_transmission_background_at_lod(
                    world_origin + world_direction * background_distance,
                    lod,
                    background_color))
            {
                background_color.rgb *= scene.composed_plane_intensity.y;
                color = background_color;
                distance = background_distance;
                ground_index = -1;
            }
        }
    }
    return distance < RAY_MAXIMUM;
}

float3 sample_environment(
    float3 world_origin,
    float3 direction,
    float lod,
    out bool composed_hit)
{
    float4 composed_color;
    float composed_distance;
    int ground_index;
    composed_hit = sample_composed_planes(
        world_origin,
        direction,
        lod,
        composed_color,
        composed_distance,
        ground_index);
    if (composed_hit)
        return composed_color.rgb;
    if (scene.environment.source == EnvironmentSource::Image)
        return environment_texture.SampleLevel(
            environment_sampler,
            equirectangular_uv(direction),
            lod).rgb;
    if (scene.environment.source == EnvironmentSource::Composite
        && scene.transmission_background.y != 0.0)
        return sample_composite_texture(
            composite_world_uv(direction),
            lod).rgb;
    if (scene.environment.source == EnvironmentSource::Black)
        return scene.environment_color.rgb;
    return float3(0.0);
}

bool sample_transmission_background(out float4 background)
{
    background = float4(0.0);
    if (scene.transmission_background.w == 0.0)
        return false;
    float3 world_origin = mul(scene.model, float4(WorldRayOrigin(), 1.0)).xyz;
    float3 world_direction = normalize(mul(
        scene.model,
        float4(WorldRayDirection(), 0.0)).xyz);
    float3 camera_forward = normalize(mul(
        scene.camera_world,
        float4(0.0, 0.0, -1.0, 0.0)).xyz);
    float3 plane_center = scene.camera_position.xyz
        + camera_forward * scene.transmission_background.x;
    float denominator = dot(world_direction, camera_forward);
    if (abs(denominator) <= RAY_MINIMUM)
        return false;
    float distance = dot(plane_center - world_origin, camera_forward) / denominator;
    if (distance <= RAY_MINIMUM
        || !sample_transmission_background_at(
            world_origin + world_direction * distance,
            background))
        return false;
    background.rgb *= scene.composed_plane_intensity.y;
    return true;
}
