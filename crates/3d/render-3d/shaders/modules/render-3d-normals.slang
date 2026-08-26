module render_3d_normals;

static const float NORMAL_EPSILON = 0.000001;

float3 spherical_interpolate(float3 first, float3 second, float amount)
{
    float cosine = clamp(dot(first, second), -1.0, 1.0);
    float angle = acos(cosine);
    if (angle < NORMAL_EPSILON)
        return first;
    float inverse_sine = 1.0 / sin(angle);
    return normalize(
        first * sin((1.0 - amount) * angle) * inverse_sine
        + second * sin(amount * angle) * inverse_sine);
}

public float3 spherical_normal(float3 normals[3], float3 barycentric)
{
    float first_pair_weight = barycentric.x + barycentric.y;
    if (first_pair_weight < NORMAL_EPSILON)
        return normals[2];
    float3 first_pair = spherical_interpolate(
        normals[0],
        normals[1],
        barycentric.y / first_pair_weight);
    return spherical_interpolate(first_pair, normals[2], barycentric.z);
}

float3 pn_edge_normal(float3 first_position, float3 second_position, float3 first, float3 second)
{
    float3 edge = second_position - first_position;
    float edge_length_squared = max(dot(edge, edge), NORMAL_EPSILON);
    float3 sum = first + second;
    return normalize(sum - edge * (2.0 * dot(edge, sum) / edge_length_squared));
}

public float3 pn_triangle_normal(
    float3 positions[3],
    float3 normals[3],
    float3 barycentric)
{
    float3 edge_01 = pn_edge_normal(positions[0], positions[1], normals[0], normals[1]);
    float3 edge_12 = pn_edge_normal(positions[1], positions[2], normals[1], normals[2]);
    float3 edge_20 = pn_edge_normal(positions[2], positions[0], normals[2], normals[0]);
    float x = barycentric.x;
    float y = barycentric.y;
    float z = barycentric.z;
    return normalize(
        normals[0] * x * x
        + normals[1] * y * y
        + normals[2] * z * z
        + edge_01 * (2.0 * x * y)
        + edge_12 * (2.0 * y * z)
        + edge_20 * (2.0 * z * x));
}
