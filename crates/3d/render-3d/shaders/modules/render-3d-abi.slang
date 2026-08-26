module render_3d_abi;

public enum class AntiAliasing : uint
{
    None = 0,
    RotatedGrid2x = 1,
    Grid4x = 2,
    Stochastic8x = 3,
};

public enum class CameraProjection : uint
{
    Perspective = 0,
    Orthographic = 1,
    Equirectangular = 2,
    Cylindrical = 3,
    Fisheye = 4,
};

public enum class NormalMode : uint
{
    Smooth = 0,
    Spherical = 1,
    PnTriangle = 2,
    Flat = 3,
};

public enum class ShadingModel : uint
{
    Pbr = 0,
    Toon = 1,
    Depth = 2,
};

public enum class PathTracingMode : uint
{
    Off = 0,
    Samples1 = 1,
    Samples2 = 2,
    Preview = 4,
    Samples8 = 8,
    Quality = 16,
    Samples32 = 32,
    Samples64 = 64,
    Samples128 = 128,
};

public enum class LightSamplingQuality : uint
{
    Rays1 = 1,
    Rays2 = 2,
    Standard = 4,
    High = 8,
    Ultra = 16,
    Rays32 = 32,
    Rays64 = 64,
};

public enum class RenderQuality : uint
{
    Interactive = 0,
    Final = 1,
};

public struct PbrSettings
{
    public PathTracingMode path_tracing;
    public LightSamplingQuality light_sampling_quality;
    public RenderQuality render_quality;
    public uint optix_denoising;
    public float subsurface;
    public float clearcoat;
    public float sheen;
    public float transmission;
    public float ior;
    public float3 _padding;
};

public enum class EnvironmentSource : uint
{
    Black = 0,
    Image = 1,
    Composite = 2,
};

public struct EnvironmentSettings
{
    public float intensity;
    public EnvironmentSource source;
    public float2 _padding;
};

public enum class OutlineMode : uint
{
    Off = 0,
    Silhouette = 1,
    SilhouetteAndCreases = 2,
};

public enum class OutlineQuality : uint
{
    Standard = 4,
    High = 8,
    Ultra = 16,
};

public enum class OutlineMethod : uint
{
    RayTraced = 0,
    Fresnel = 1,
    Hybrid = 2,
    Sobel = 3,
    RobertsCross = 4,
    DifferenceOfGaussians = 5,
    RegionBoundary = 6,
};

public enum class ToonTextureFilter : uint
{
    Direct = 0,
    Kuwahara = 1,
};

public enum class ToonShadowKind : uint
{
    Solid = 0,
    Dots = 1,
    Lines = 2,
    Crosshatch = 3,
};

public struct ToonSettings
{
    public float4 shadow;
    public float4 rim;
    public ShadingModel shading_model;
    public float bands;
    public float rim_power;
    public float specular_size;
    public float specular_strength;
    public float color_levels;
    public float2 outline_frequency;
    public float4 outline_color;
    public OutlineMode outline_mode;
    public float outline_width;
    public float outline_opacity;
    public AntiAliasing anti_aliasing;
    public OutlineQuality outline_quality;
    public float outline_depth_threshold;
    public float outline_normal_cosine;
    public OutlineMethod outline_method;
    public float dog_inner_radius;
    public float dog_radius_ratio;
    public float dog_threshold;
    public float dog_sharpness;
    public float outline_offset_variation;
    public float outline_width_variation;
    public uint outline_noise_seed;
    public float outline_noise_evolution;
    public float outline_aggressiveness;
    public float kuwahara_radius;
    public float kuwahara_strength;
    public ToonTextureFilter texture_filter;
    public ToonShadowKind shadow_kind;
    public float shadow_darkest_tone;
    public float shadow_crosshatch_max_directions;
    public float shadow_dot_size;
    public float shadow_line_direction;
    public float shadow_line_width;
    public float shadow_line_distribution_randomness;
    public float shadow_line_width_randomness;
    public float shadow_dot_density;
    public float shadow_line_density;
    public float shadow_dot_distribution_randomness;
    public float shadow_dot_size_randomness;
    public float shadow_pattern_softness;
    public float shadow_crosshatch_angle;
    public float2 _shadow_quantization_padding;
};

static const uint MAX_SCENE_LIGHTS = 32;
static const uint MAX_SCENE_GROUNDS = 32;

public struct PointLight
{
    public float4 position_radius;
    public float4 color_intensity;
    public float4 range_padding;
};

public struct SunLight
{
    public float4 direction_angular_radius;
    public float4 color_intensity;
};

public struct Ground
{
    public float4 position_opacity;
    public float4 basis_x_size;
    public float4 normal_composite;
    public float4 basis_z_intensity;
    public float4 settings;
};

public struct MeshInstance
{
    public float4x4 model;
    public float4x4 normal;
    public uint4 geometry_offsets;
};

public struct SceneUniforms
{
    public float4x4 model;
    public float4x4 model_inverse;
    public float4x4 normal;
    public float4x4 camera_world;
    public float4x4 view_projection;
    public float4x4 inverse_view_projection;
    public float4x4 environment_inverse_rotation;
    public float4 camera_position;
    public float4 camera_lens;
    public float4 base_color;
    public PointLight point_lights[MAX_SCENE_LIGHTS];
    public SunLight sun_lights[MAX_SCENE_LIGHTS];
    public Ground grounds[MAX_SCENE_GROUNDS];
    public float4 shadow_receiver_position_opacity;
    public float4 shadow_receiver_normal_enabled;
    public float4 ground_settings;
    public float4 transmission_background;
    public float4 composed_plane_intensity;
    public float4 material;
    public float4 environment_color;
    public uint point_light_count;
    public uint sun_light_count;
    public uint ground_count;
    public uint _ground_count_padding;
    public float4 ground_toon_shadow;
    public ShadingModel ground_shading_model;
    public ToonShadowKind ground_toon_shadow_kind;
    public NormalMode normal_mode;
    public uint _ground_style_padding;
    public EnvironmentSettings environment;
    public PbrSettings pbr;
    public ToonSettings toon;
};

public struct TextureMapping
{
    public float4 atlas;
    public float4 transform;
    public uint4 settings;
    public float4 rotation_filter;
};

public struct MeshMaterial
{
    public float4 base_color_factor;
    public float4 metallic_roughness_normal_alpha;
    public uint4 flags;
    public TextureMapping base_color_texture;
    public TextureMapping metallic_roughness_texture;
    public TextureMapping normal_texture;
    public uint4 surface;
    public PbrSettings pbr;
    public ToonSettings toon;
};
