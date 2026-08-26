float3 shadow_visibility(
    float3 world_position,
    float3 world_normal,
    float3 direction,
    float distance)
{
    float3 origin = world_position + world_normal * RAY_MINIMUM;
    float3 object_origin = mul(scene.model_inverse, float4(origin, 1.0)).xyz;
    float3 object_target = mul(
        scene.model_inverse,
        float4(origin + direction * distance, 1.0)).xyz;
    float3 object_delta = object_target - object_origin;

    RayDesc ray;
    ray.Origin = object_origin;
    ray.Direction = normalize(object_delta);
    ray.TMin = RAY_MINIMUM;
    ray.TMax = max(length(object_delta) - RAY_MINIMUM, RAY_MINIMUM);
    ShadowPayload shadow = {
        float3(1.0),
        0
    };
    TraceRay(
        scene_acceleration,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH
            | RAY_FLAG_SKIP_CLOSEST_HIT_SHADER
            | RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES,
        0xff,
        1,
        0,
        1,
        ray,
        shadow);
    return shadow.visible != 0 ? shadow.transmittance : float3(0.0);
}

float3 sample_pbr_sun(
    SunLight source,
    float3 world_position,
    float3 normal,
    float3 view,
    float3 base_color,
    float metallic,
    float roughness,
    PbrSettings settings,
    uint sample_count,
    inout uint state)
{
    float3 color = float3(0.0);
    float3 center_direction = normalize(-source.direction_angular_radius.xyz);
    float3 radiance = source.color_intensity.rgb * source.color_intensity.w;
    for (uint sample_index = 0; sample_index < sample_count; ++sample_index)
    {
        float3 light = sample_cone(
            center_direction,
            source.direction_angular_radius.w,
            state);
        float3 visibility = shadow_visibility(
            world_position,
            normal,
            light,
            RAY_MAXIMUM);
        color += pbr_light(
            normal,
            view,
            light,
            radiance,
            base_color,
            metallic,
            roughness,
            visibility,
            settings);
    }
    return color / float(sample_count);
}

float3 sample_pbr_point(
    PointLight source,
    float3 world_position,
    float3 normal,
    float3 view,
    float3 base_color,
    float metallic,
    float roughness,
    PbrSettings settings,
    uint sample_count,
    inout uint state)
{
    float3 color = float3(0.0);
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
        float3 visibility = shadow_visibility(
            world_position,
            normal,
            light,
            distance);
        float falloff = pow(1.0 - distance / source.range_padding.x, 2.0)
            / max(distance_squared, 0.01);
        float3 radiance = source.color_intensity.rgb
            * source.color_intensity.w * falloff;
        color += pbr_light(
            normal,
            view,
            light,
            radiance,
            base_color,
            metallic,
            roughness,
            visibility,
            settings);
    }
    return color / float(sample_count);
}

float3 sample_pbr_environment(
    float3 world_position,
    float3 normal,
    float3 view,
    float3 base_color,
    float metallic,
    float roughness,
    PbrSettings settings,
    inout uint state)
{
    uint candidate_count = uint(scene.pbr.light_sampling_quality);
    if (scene.pbr.render_quality == RenderQuality::Interactive)
        candidate_count = MINIMUM_VARIANCE_SAMPLES;

    float3 selected_light = float3(0.0);
    float3 selected_contribution = float3(0.0);
    float selected_target = 0.0;
    float reservoir_weight_sum = 0.0;
    for (uint candidate_index = 0; candidate_index < candidate_count; ++candidate_index)
    {
        float3 light = sample_unit_sphere(state);
        bool composed_hit;
        float3 radiance = sample_environment(
            world_position,
            light,
            0.0,
            composed_hit);
        if (!composed_hit)
            radiance *= scene.environment.intensity;
        float bsdf_pdf = pbr_reflection_pdf(
            normal,
            view,
            light,
            metallic,
            roughness,
            settings);
        float mis_weight = power_heuristic(UNIFORM_ENVIRONMENT_PDF, bsdf_pdf);
        float3 contribution = pbr_light(
            normal,
            view,
            light,
            radiance,
            base_color,
            metallic,
            roughness,
            float3(1.0),
            settings) * mis_weight;
        float target = max(luminance(contribution), 0.0);
        float reservoir_weight = target / UNIFORM_ENVIRONMENT_PDF;
        reservoir_weight_sum += reservoir_weight;
        if (reservoir_weight > 0.0
            && random_float(state) * reservoir_weight_sum < reservoir_weight)
        {
            selected_light = light;
            selected_contribution = contribution;
            selected_target = target;
        }
    }

    if (selected_target <= 0.0)
        return float3(0.0);

    float3 visibility = shadow_visibility(
        world_position,
        normal,
        selected_light,
        RAY_MAXIMUM);
    return selected_contribution * visibility * reservoir_weight_sum
        / (float(candidate_count) * selected_target);
}
