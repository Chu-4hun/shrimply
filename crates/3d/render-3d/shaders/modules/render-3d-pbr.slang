module render_3d_pbr;

import render_3d_abi;
import render_3d_sampling;

static const float PI = 3.14159265358979323846;
static const float MINIMUM_PDF = 0.00001;

public struct PbrSample
{
    public float3 direction;
    public float3 weight;
    public float pdf;
    public bool valid;
    public bool delta;
    public bool refracted;
};

float dielectric_f0(float ior)
{
    float ratio = (ior - 1.0) / (ior + 1.0);
    return ratio * ratio;
}

public float3 pbr_fresnel(
    float3 base_color,
    float metallic,
    float cosine,
    float ior)
{
    float3 f0 = lerp(float3(dielectric_f0(ior)), base_color, metallic);
    return f0 + (1.0 - f0) * pow(1.0 - saturate(cosine), 5.0);
}

float pbr_distribution_ggx(float n_dot_h, float roughness)
{
    float alpha = roughness * roughness;
    float alpha_squared = alpha * alpha;
    float denominator = n_dot_h * n_dot_h * (alpha_squared - 1.0) + 1.0;
    return alpha_squared / max(PI * denominator * denominator, MINIMUM_PDF);
}

float pbr_geometry_schlick(float n_dot_direction, float roughness)
{
    float k = (roughness + 1.0) * (roughness + 1.0) * 0.125;
    return n_dot_direction / max(n_dot_direction * (1.0 - k) + k, MINIMUM_PDF);
}

public float3 pbr_light(
    float3 normal,
    float3 view,
    float3 light,
    float3 radiance,
    float3 base_color,
    float metallic,
    float roughness,
    float3 visibility,
    PbrSettings settings)
{
    float raw_n_dot_l = dot(normal, light);
    float n_dot_l = saturate(raw_n_dot_l);
    float wrapped_diffuse = saturate(
        (raw_n_dot_l + settings.subsurface) / (1.0 + settings.subsurface));
    if (max(n_dot_l, wrapped_diffuse) <= 0.0
        || max(visibility.r, max(visibility.g, visibility.b)) <= 0.0)
        return float3(0.0);

    float3 half_direction = normalize(view + light);
    float n_dot_v = max(dot(normal, view), MINIMUM_PDF);
    float n_dot_h = saturate(dot(normal, half_direction));
    float3 fresnel = pbr_fresnel(
        base_color,
        metallic,
        dot(half_direction, view),
        settings.ior);
    float geometry = pbr_geometry_schlick(n_dot_v, roughness)
        * pbr_geometry_schlick(n_dot_l, roughness);
    float specular_scale = pbr_distribution_ggx(n_dot_h, roughness) * geometry
        / max(4.0 * n_dot_v * n_dot_l, MINIMUM_PDF);
    float3 diffuse = base_color * (1.0 - metallic) * (1.0 - settings.transmission)
        * (1.0 - fresnel)
        * lerp(n_dot_l, wrapped_diffuse, settings.subsurface) / PI;
    float clearcoat_roughness = lerp(0.04, roughness, 0.25);
    float clearcoat = settings.clearcoat
        * pbr_distribution_ggx(n_dot_h, clearcoat_roughness)
        * pbr_geometry_schlick(n_dot_v, clearcoat_roughness)
        * pbr_geometry_schlick(n_dot_l, clearcoat_roughness)
        * 0.25;
    float3 sheen = base_color * settings.sheen * pow(1.0 - n_dot_h, 5.0);
    return (diffuse + (fresnel * specular_scale + clearcoat + sheen) * n_dot_l)
        * radiance * visibility;
}

public float3 sample_ggx_half_vector(float3 normal, float roughness, inout uint state)
{
    float alpha = roughness * roughness;
    float alpha_squared = alpha * alpha;
    float azimuth = 2.0 * PI * random_float(state);
    float sample = random_float(state);
    float cosine = sqrt((1.0 - sample) / (1.0 + (alpha_squared - 1.0) * sample));
    float sine = sqrt(max(0.0, 1.0 - cosine * cosine));
    return local_to_world(float3(sine * cos(azimuth), sine * sin(azimuth), cosine), normal);
}

void pbr_lobe_probabilities(
    float metallic,
    PbrSettings settings,
    out float diffuse_probability,
    out float specular_probability,
    out float glass_probability)
{
    glass_probability = settings.transmission * (1.0 - metallic);
    specular_probability = lerp(
        max(dielectric_f0(settings.ior), 0.08),
        0.9,
        metallic) * (1.0 - glass_probability);
    diffuse_probability = (1.0 - metallic) * (1.0 - settings.transmission);
    float probability_sum = diffuse_probability + specular_probability + glass_probability;
    diffuse_probability /= probability_sum;
    specular_probability /= probability_sum;
    glass_probability /= probability_sum;
}

public float pbr_reflection_pdf(
    float3 normal,
    float3 view,
    float3 light,
    float metallic,
    float roughness,
    PbrSettings settings)
{
    float n_dot_l = saturate(dot(normal, light));
    if (n_dot_l <= 0.0)
        return 0.0;
    float3 half_direction = normalize(view + light);
    float n_dot_h = saturate(dot(normal, half_direction));
    float v_dot_h = saturate(dot(view, half_direction));
    float diffuse_probability;
    float specular_probability;
    float glass_probability;
    pbr_lobe_probabilities(
        metallic,
        settings,
        diffuse_probability,
        specular_probability,
        glass_probability);
    float diffuse_pdf = n_dot_l / PI;
    float specular_pdf = pbr_distribution_ggx(n_dot_h, roughness) * n_dot_h
        / max(4.0 * v_dot_h, MINIMUM_PDF);
    return diffuse_probability * diffuse_pdf + specular_probability * specular_pdf;
}

public PbrSample pbr_sample(
    float3 normal,
    float3 view,
    bool front_face,
    float3 base_color,
    float metallic,
    float roughness,
    PbrSettings settings,
    inout uint state)
{
    float diffuse_probability;
    float specular_probability;
    float glass_probability;
    pbr_lobe_probabilities(
        metallic,
        settings,
        diffuse_probability,
        specular_probability,
        glass_probability);

    float choice = random_float(state);
    PbrSample result;
    result.valid = true;
    result.delta = false;
    result.refracted = false;
    if (choice < diffuse_probability)
        result.direction = sample_cosine_hemisphere(normal, state);
    else if (choice < diffuse_probability + specular_probability)
    {
        if (roughness <= 0.0)
        {
            result.direction = reflect(-view, normal);
            result.valid = dot(normal, result.direction) > 0.0;
            result.pdf = 0.0;
            result.delta = true;
            result.weight = pbr_fresnel(
                base_color,
                metallic,
                dot(normal, view),
                settings.ior) / max(specular_probability, MINIMUM_PDF);
            return result;
        }
        float3 half_direction = sample_ggx_half_vector(normal, roughness, state);
        result.direction = reflect(-view, half_direction);
    }
    else
    {
        float eta = front_face ? 1.0 / settings.ior : settings.ior;
        float cosine = saturate(dot(normal, view));
        float fresnel = dielectric_f0(settings.ior)
            + (1.0 - dielectric_f0(settings.ior)) * pow(1.0 - cosine, 5.0);
        float3 microfacet_normal = roughness <= 0.0
            ? normal
            : sample_ggx_half_vector(normal, roughness, state);
        float3 refracted = refract(-view, microfacet_normal, eta);
        bool total_internal_reflection = dot(refracted, refracted) <= MINIMUM_PDF;
        bool reflect_glass = total_internal_reflection || random_float(state) < fresnel;
        result.direction = reflect_glass
            ? reflect(-view, microfacet_normal)
            : normalize(refracted);
        result.valid = dot(result.direction, result.direction) > MINIMUM_PDF;
        result.pdf = 0.0;
        result.delta = true;
        result.refracted = !reflect_glass;
        result.weight = reflect_glass
            ? float3(1.0)
            : base_color;
        result.weight *= settings.transmission / max(glass_probability, MINIMUM_PDF);
        return result;
    }

    float n_dot_l = saturate(dot(normal, result.direction));
    float n_dot_v = saturate(dot(normal, view));
    float3 half_direction = normalize(view + result.direction);
    float n_dot_h = saturate(dot(normal, half_direction));
    float v_dot_h = saturate(dot(view, half_direction));
    if (n_dot_l <= 0.0 || v_dot_h <= 0.0)
    {
        result.valid = false;
        result.weight = float3(0.0);
        return result;
    }

    float distribution = pbr_distribution_ggx(n_dot_h, roughness);
    float geometry = pbr_geometry_schlick(n_dot_v, roughness)
        * pbr_geometry_schlick(n_dot_l, roughness);
    float3 fresnel = pbr_fresnel(base_color, metallic, v_dot_h, settings.ior);
    float3 diffuse_brdf = base_color * (1.0 - metallic) * (1.0 - settings.transmission)
        * (1.0 - fresnel) / PI;
    diffuse_brdf *= lerp(1.0, 1.0 + settings.subsurface, settings.subsurface);
    diffuse_brdf += base_color * settings.sheen * pow(1.0 - n_dot_h, 5.0);
    float3 specular_brdf = fresnel * distribution * geometry
        / max(4.0 * n_dot_v * n_dot_l, MINIMUM_PDF);
    float clearcoat_roughness = lerp(0.04, roughness, 0.25);
    specular_brdf += settings.clearcoat * float3(0.04)
        * pbr_distribution_ggx(n_dot_h, clearcoat_roughness);

    float diffuse_pdf = n_dot_l / PI;
    float specular_pdf = distribution * n_dot_h / max(4.0 * v_dot_h, MINIMUM_PDF);
    float mixture_pdf = diffuse_probability * diffuse_pdf
        + specular_probability * specular_pdf;
    result.pdf = mixture_pdf;
    result.weight = (diffuse_brdf + specular_brdf) * n_dot_l
        / max(mixture_pdf, MINIMUM_PDF);
    return result;
}
