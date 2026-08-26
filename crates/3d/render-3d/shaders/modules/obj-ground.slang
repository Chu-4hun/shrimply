uint adaptive_light_sample_count()
{
    uint requested = uint(scene.pbr.light_sampling_quality);
    if (scene.pbr.render_quality == RenderQuality::Interactive)
        return min(requested, MINIMUM_VARIANCE_SAMPLES);
    return requested;
}

float ground_shadow_alpha(float3 world_position, float3 normal, inout uint state)
{
    uint sample_count = adaptive_light_sample_count();
    float total_illuminance = 0.0;
    float blocked_illuminance = 0.0;

    for (uint light_index = 0; light_index < scene.sun_light_count; ++light_index)
    {
        SunLight source = scene.sun_lights[light_index];
        float3 center_direction = normalize(-source.direction_angular_radius.xyz);
        float radiance = luminance(source.color_intensity.rgb)
            * source.color_intensity.w;
        for (uint sample_index = 0; sample_index < sample_count; ++sample_index)
        {
            float3 light = sample_cone(
                center_direction,
                source.direction_angular_radius.w,
                state);
            float illuminance = radiance * max(dot(normal, light), 0.0);
            total_illuminance += illuminance;
            blocked_illuminance += illuminance * (
                1.0 - saturate(luminance(shadow_visibility(
                    world_position,
                    normal,
                    light,
                    RAY_MAXIMUM))));
        }
    }

    for (uint light_index = 0; light_index < scene.point_light_count; ++light_index)
    {
        PointLight source = scene.point_lights[light_index];
        for (uint sample_index = 0; sample_index < sample_count; ++sample_index)
        {
            float3 sampled_position = source.position_radius.xyz
                + sample_unit_sphere(state) * source.position_radius.w;
            float3 delta = sampled_position - world_position;
            float distance_squared = dot(delta, delta);
            float distance = sqrt(distance_squared);
            if (distance >= source.range_padding.x)
                continue;
            float3 light = delta / max(distance, RAY_MINIMUM);
            float falloff = pow(1.0 - distance / source.range_padding.x, 2.0)
                / max(distance_squared, 0.01);
            float radiance = luminance(source.color_intensity.rgb)
                * source.color_intensity.w * falloff;
            float illuminance = radiance * max(dot(normal, light), 0.0);
            total_illuminance += illuminance;
            blocked_illuminance += illuminance * (
                1.0 - saturate(luminance(shadow_visibility(
                    world_position,
                    normal,
                    light,
                    distance))));
        }
    }

    if (total_illuminance <= RAY_MINIMUM)
        return 0.0;
    return saturate(blocked_illuminance / total_illuminance);
}

float4 ground_reflection(
    Ground ground,
    float3 world_position,
    float3 normal,
    float3 world_direction,
    uint path_bounce,
    uint random_seed)
{
    if (ground.settings.x <= 0.0
        || ground.position_opacity.w <= 0.0)
        return float4(0.0);

    float3 reflection_normal = ground.settings.z > 0.0
        ? sample_ggx_half_vector(normal, ground.settings.z, random_seed)
        : normal;
    float3 reflection_direction = reflect(world_direction, reflection_normal);
    if (dot(reflection_direction, normal) <= RAY_MINIMUM)
        return float4(0.0);
    float3 reflection_origin = world_position + reflection_direction * RAY_MINIMUM;
    RayDesc ray = {
        mul(scene.model_inverse, float4(reflection_origin, 1.0)).xyz,
        RAY_MINIMUM,
        normalize(mul(
            scene.model_inverse,
            float4(reflection_direction, 0.0)).xyz),
        RAY_MAXIMUM
    };
    RayPayload reflected = {
        float4(0.0),
        float4(0.0),
        float3(0.0),
        RAY_MAXIMUM,
        RAY_MAXIMUM,
        0.0,
        0,
        0,
        path_bounce + 1,
        random_seed,
        0,
        0xffffffffu
    };
    TraceRay(
        scene_acceleration,
        RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES,
        0xff,
        0,
        0,
        0,
        ray,
        reflected);

    if (reflected.hit == 0)
        return float4(0.0);

    float4 result = float4(
        reflected.color.rgb,
        reflected.color.a * ground.position_opacity.w);
    result.a *= ground.settings.x;
    if (path_bounce == 0 && scene.pbr.optix_denoising == 0)
    {
        result.rgb = float3(
            linear_to_srgb(aces_tone_map(result.r)),
            linear_to_srgb(aces_tone_map(result.g)),
            linear_to_srgb(aces_tone_map(result.b)));
    }
    return result;
}

void composite_over(inout float4 destination, float4 source)
{
    float destination_weight = destination.a * (1.0 - source.a);
    float output_alpha = source.a + destination_weight;
    destination.rgb = output_alpha > 0.0
        ? (source.rgb * source.a + destination.rgb * destination_weight) / output_alpha
        : float3(0.0);
    destination.a = output_alpha;
}

void composite_virtual_surfaces(
    inout RayPayload payload,
    float3 world_origin,
    float3 world_direction,
    uint random_seed)
{
    float visible_distance = payload.hit != 0 ? payload.ray_distance : RAY_MAXIMUM;
    int ground_index = -1;
    float ground_distance = visible_distance + RAY_MINIMUM;
    float3 world_position = float3(0.0);
    float3 ground_normal = float3(0.0);
    if (payload.path_bounce != 0)
    {
        float4 virtual_color = float4(0.0);
        float virtual_distance = RAY_MAXIMUM;
        int virtual_ground = -1;
        if (!sample_composed_planes(
            world_origin,
            world_direction,
            0.0,
            virtual_color,
            virtual_distance,
            virtual_ground)
            || virtual_distance >= visible_distance)
            return;

        float3 virtual_position = world_origin + world_direction * virtual_distance;
        float3 camera_forward = normalize(mul(
            scene.camera_world,
            float4(0.0, 0.0, -1.0, 0.0)).xyz);
        payload.hit = 1;
        payload.material_index = 0xffffffffu;
        payload.normal = virtual_ground >= 0
            ? normalize(scene.grounds[virtual_ground].normal_composite.xyz)
            : -camera_forward;
        payload.depth = length(scene.camera_position.xyz - virtual_position);
        payload.ray_distance = virtual_distance;
        if (payload.transmitted != 0)
        {
            payload.color = float4(0.0);
            payload.background = virtual_color;
        }
        else
        {
            payload.color = virtual_color;
            payload.background = float4(0.0);
        }
        if (virtual_ground < 0)
            return;
        ground_index = virtual_ground;
        ground_distance = virtual_distance;
        world_position = virtual_position;
        ground_normal = payload.normal;
    }
    else
    {
        for (uint index = 0; index < scene.ground_count; ++index)
        {
            float candidate_distance;
            float3 candidate_position;
            float3 candidate_normal;
            if (intersect_ground(
                scene.grounds[index],
                world_origin,
                world_direction,
                candidate_distance,
                candidate_position,
                candidate_normal)
                && candidate_distance < ground_distance)
            {
                ground_index = int(index);
                ground_distance = candidate_distance;
                world_position = candidate_position;
                ground_normal = candidate_normal;
            }
        }
    }
    if (ground_index < 0 || ground_distance >= visible_distance + RAY_MINIMUM)
        return;
    Ground ground = scene.grounds[ground_index];

    if (scene.ground_shading_model == ShadingModel::Depth)
    {
        float depth = depth_buffer_value(world_position);
        payload.color = float4(float3(depth), ground.position_opacity.w);
        payload.background = float4(0.0);
        payload.normal = ground_normal;
        payload.depth = length(scene.camera_position.xyz - world_position);
        payload.ray_distance = ground_distance;
        payload.hit = 1;
        payload.material_index = 0xffffffffu;
        return;
    }

    uint state = random_seed;
    float shadow_occlusion = ground_shadow_alpha(world_position, ground_normal, state);
    float receiver_strength = ground.settings.y * ground.position_opacity.w;
    float shadow_alpha = shadow_occlusion * receiver_strength;
    ToonSettings ground_toon = scene.toon;
    ground_toon.shading_model = scene.ground_shading_model;
    ground_toon.shadow_kind = scene.ground_toon_shadow_kind;
    ground_toon.shadow = scene.ground_toon_shadow;
    bool toon = ground_toon.shading_model == ShadingModel::Toon;
    float3 toon_shadow_color = ground_toon.shadow.rgb * scene.material.z;
    float toon_ink_alpha = 0.0;
    if (toon && scene.pbr.optix_denoising == 0)
    {
        toon_shadow_color = float3(
            linear_to_srgb(aces_tone_map(toon_shadow_color.r)),
            linear_to_srgb(aces_tone_map(toon_shadow_color.g)),
            linear_to_srgb(aces_tone_map(toon_shadow_color.b)));
    }
    if (toon)
    {
        float tone_index = toon_tone_index(1.0 - shadow_occlusion, ground_toon);
        toon_ink_alpha = receiver_strength * ground_toon.shadow.w
            * toon_shadow_pattern(
                float2(DispatchRaysIndex().xy) + 0.5,
                tone_index,
                ground_toon);
    }
    float4 reflection = ground_reflection(
        ground,
        world_position,
        ground_normal,
        world_direction,
        payload.path_bounce,
        state);
    if (payload.transmitted != 0)
    {
        composite_over(payload.background, reflection);
        if (toon)
        {
            payload.background.rgb = lerp(
                payload.background.rgb,
                toon_shadow_color,
                toon_ink_alpha);
            payload.color.rgb = lerp(
                payload.color.rgb,
                toon_shadow_color,
                toon_ink_alpha);
        }
        else
        {
            payload.background.rgb *= 1.0 - shadow_alpha;
            payload.color.rgb *= 1.0 - shadow_alpha;
        }
        return;
    }
    composite_over(payload.color, reflection);
    if (toon)
    {
        if (toon_ink_alpha > 0.0)
            composite_over(
                payload.color,
                float4(toon_shadow_color, toon_ink_alpha));
        return;
    }
    if (shadow_alpha <= 0.0)
        return;
    float destination_weight = payload.color.a * (1.0 - shadow_alpha);
    float output_alpha = shadow_alpha + destination_weight;
    payload.color.rgb = output_alpha > 0.0
        ? payload.color.rgb * destination_weight / output_alpha
        : float3(0.0);
    payload.color.a = output_alpha;
}
