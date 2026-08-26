module render_3d_sampling;

static const float PI = 3.14159265358979323846;
static const float GOLDEN_ANGLE = 2.39996322972865332;
static const uint PIXEL_HASH_X = 0x9e3779b9u;
static const uint PIXEL_HASH_Y = 0x85ebca6bu;
static const uint SAMPLE_HASH = 0xc2b2ae35u;
static const uint HASH_MULTIPLIER_A = 0x7feb352du;
static const uint HASH_MULTIPLIER_B = 0x846ca68bu;
static const float UINT_TO_UNIT_FLOAT = 1.0 / 4294967296.0;
static const float3 LINEAR_LUMINANCE_WEIGHTS = float3(0.2126, 0.7152, 0.0722);

public float luminance(float3 color)
{
    return dot(color, LINEAR_LUMINANCE_WEIGHTS);
}

public uint random_seed(uint2 pixel, uint sample_index)
{
    uint state = pixel.x * PIXEL_HASH_X
        ^ pixel.y * PIXEL_HASH_Y
        ^ sample_index * SAMPLE_HASH;
    state ^= state >> 16;
    state *= HASH_MULTIPLIER_A;
    state ^= state >> 15;
    state *= HASH_MULTIPLIER_B;
    return state ^ (state >> 16);
}

public float random_float(inout uint state)
{
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    return float(state) * UINT_TO_UNIT_FLOAT;
}

public float2 random_disk(inout uint state)
{
    float radius = sqrt(random_float(state));
    float angle = 2.0 * PI * random_float(state);
    return radius * float2(cos(angle), sin(angle));
}

public void tangent_frame(float3 normal, out float3 tangent, out float3 bitangent)
{
    tangent = normalize(abs(normal.y) < 0.999
        ? cross(normal, float3(0.0, 1.0, 0.0))
        : cross(normal, float3(1.0, 0.0, 0.0)));
    bitangent = cross(normal, tangent);
}

public float3 local_to_world(float3 local, float3 normal)
{
    float3 tangent;
    float3 bitangent;
    tangent_frame(normal, tangent, bitangent);
    return normalize(local.x * tangent + local.y * bitangent + local.z * normal);
}

public float3 sample_cosine_hemisphere(float3 normal, inout uint state)
{
    float2 disk = random_disk(state);
    return local_to_world(
        float3(disk, sqrt(max(0.0, 1.0 - dot(disk, disk)))),
        normal);
}

public float3 sample_unit_sphere(inout uint state)
{
    float z = 1.0 - 2.0 * random_float(state);
    float angle = 2.0 * PI * random_float(state);
    float radius = sqrt(max(0.0, 1.0 - z * z));
    return float3(radius * cos(angle), radius * sin(angle), z);
}

public float3 sample_cone(float3 axis, float angular_radius, inout uint state)
{
    if (angular_radius <= 0.0)
        return axis;
    float cosine = lerp(1.0, cos(angular_radius), random_float(state));
    float sine = sqrt(max(0.0, 1.0 - cosine * cosine));
    float angle = 2.0 * PI * random_float(state);
    return local_to_world(float3(sine * cos(angle), sine * sin(angle), cosine), axis);
}

public float2 stochastic_pixel_offset(uint index, uint count, uint2 pixel)
{
    float radius = sqrt((float(index) + 0.5) / float(max(count, 1u))) * 0.48;
    uint seed = random_seed(pixel, 0);
    float angle = float(index) * GOLDEN_ANGLE + 2.0 * PI * random_float(seed);
    return float2(cos(angle), sin(angle)) * radius;
}

public float power_heuristic(float first_pdf, float second_pdf)
{
    float first_squared = first_pdf * first_pdf;
    float second_squared = second_pdf * second_pdf;
    return first_squared / max(first_squared + second_squared, 0.0000001);
}
