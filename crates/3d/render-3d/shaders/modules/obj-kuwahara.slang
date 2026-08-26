static const float KUWAHARA_DEPTH_REJECTION = 0.1;
static const float KUWAHARA_NORMAL_REJECTION_COSINE = 0.5;

float3 screen_space_kuwahara(
    float3 center_color,
    float3 center_normal,
    float center_depth,
    ToonSettings settings)
{
    int radius = int(settings.kuwahara_radius);
    if (radius == 0 || settings.kuwahara_strength == 0.0)
        return center_color;

    uint2 dimensions = DispatchRaysDimensions().xy;
    float2 center_position = float2(DispatchRaysIndex().xy) + 0.5;
    float3 best_mean = center_color;
    float best_variance = RAY_MAXIMUM;
    for (int quadrant = 0; quadrant < 4; ++quadrant)
    {
        int2 direction = int2(
            (quadrant & 1) == 0 ? -1 : 1,
            (quadrant & 2) == 0 ? -1 : 1);
        float3 sum = center_color;
        float3 squared_sum = center_color * center_color;
        float sample_count = 1.0;
        for (int y = 0; y <= radius; ++y)
        {
            for (int x = 0; x <= radius; ++x)
            {
                if (x == 0 && y == 0)
                    continue;
                float2 sample_position = center_position
                    + float2(direction * int2(x, y));
                if (any(sample_position < float2(0.0))
                    || any(sample_position >= float2(dimensions)))
                    continue;
                RayPayload sample = trace_surface(sample_position, dimensions, true, 0);
                if (sample.hit == 0)
                    continue;
                float relative_depth = abs(center_depth - sample.depth)
                    / max(min(center_depth, sample.depth), RAY_MINIMUM);
                if (relative_depth > KUWAHARA_DEPTH_REJECTION
                    || dot(center_normal, sample.normal)
                        < KUWAHARA_NORMAL_REJECTION_COSINE)
                    continue;
                sum += sample.color.rgb;
                squared_sum += sample.color.rgb * sample.color.rgb;
                sample_count += 1.0;
            }
        }
        float3 mean = sum / sample_count;
        float3 variance = max(squared_sum / sample_count - mean * mean, 0.0);
        float variance_score = dot(variance, float3(0.2126, 0.7152, 0.0722));
        if (variance_score < best_variance)
        {
            best_variance = variance_score;
            best_mean = mean;
        }
    }
    return lerp(center_color, best_mean, settings.kuwahara_strength);
}
