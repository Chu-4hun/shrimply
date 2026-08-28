use adw::prelude::AdwDialogExt;
use ffmpeg_next as ffmpeg;
use gtk::prelude::*;
use shrimply_core::timeline_value::*;
use shrimply_project::project::{Project, Time, VideoItemContent, generated_item_keyframe_span};
use shrimply_scene_3d::{
    AntiAliasing, BackgroundAddressMode, Camera3d, CameraProjection, Environment3d,
    EnvironmentSource, LightSamplingQuality, ObjScene, PathTracingMode, PbrMaterial, ShadingModel,
    ToonOutlineMethod, ToonOutlineMode, ToonOutlineQuality, ToonShadowKind, ToonTextureFilter,
};
use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nFileFilterExt;

use crate::InspectedItem as SelectedItem;
use crate::InspectorContext;
use crate::item::{DefaultInspectorItem, InspectorListItem};
use crate::player_state::{self, ProjectChange};
use crate::section::InspectorSection;
use crate::selector::{enum_selector, selector};
use crate::timeline_value::color::{ColorAccess, ColorTarget, color_control};
use crate::timeline_value::scalar::{
    ScalarAccess, ScalarSpec, ScalarTarget, SceneScalarGet, SceneScalarGetMut, scalar_control,
};
use crate::timeline_value::vector::vec3::{Vec3Target, control as vec3_control};

pub(super) fn items(scene: &ObjScene, _context: &InspectorContext) -> Vec<InspectorListItem> {
    vec![
        DefaultInspectorItem::new(
            "scene-3d-camera",
            "Camera",
            scene.camera.clone(),
            camera_controls,
            |context, value| {
                reset(context, "reset-scene-3d-camera", move |scene| {
                    scene.camera = value
                })
            },
        )
        .boxed(),
        DefaultInspectorItem::new(
            "scene-3d-render",
            "Render Style",
            scene.material.clone(),
            render_controls,
            |context, value| {
                reset(context, "reset-scene-3d-render", move |scene| {
                    scene.material = value
                })
            },
        )
        .boxed(),
        DefaultInspectorItem::new(
            "scene-3d-environment",
            "Environment",
            scene.environment.clone(),
            environment_controls,
            |context, value| {
                reset(context, "reset-scene-3d-environment", move |scene| {
                    scene.environment = value
                })
            },
        )
        .boxed(),
    ]
}

fn render_controls(value: &PbrMaterial, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls(|section| {
        section.add_wide_control(&shading_selector(value.shading_model, context));
        add_outline_controls(section, value, context);
        match value.shading_model {
            ShadingModel::Pbr => {
                section.add_wide_control(&path_tracing_selector(value.path_tracing, context));
                section.add_wide_control(&light_sampling_selector(
                    value.light_sampling_quality,
                    context,
                ));
                section.add_wide_control(&optix_denoising_selector(value.optix_denoising, context));
            }
            ShadingModel::Toon => add_toon_controls(section, value, context),
            ShadingModel::Depth => {}
        }
    })
}

fn add_toon_controls(
    section: &InspectorSection,
    material: &PbrMaterial,
    context: &InspectorContext,
) {
    let toon = &material.toon;
    section.add_wide_control(&light_sampling_selector(
        material.light_sampling_quality,
        context,
    ));
    for (label, value, get, get_mut, kind) in [
        (
            "Light bands",
            &toon.bands,
            (|scene: &ObjScene| &scene.material.toon.bands) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.bands) as SceneScalarGetMut,
            ScalarKind::Bands,
        ),
        (
            "Palette levels",
            &toon.color_levels,
            (|scene: &ObjScene| &scene.material.toon.color_levels) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.color_levels) as SceneScalarGetMut,
            ScalarKind::ColorLevels,
        ),
    ] {
        add_scalar(
            section,
            label,
            value,
            context,
            SceneScalar::new(get, get_mut).kind(kind),
        );
    }
    section.add_wide_control(&texture_filter_selector(toon.texture_filter, context));
    if toon.texture_filter == ToonTextureFilter::Kuwahara {
        add_scalar(
            section,
            "Kuwahara radius",
            &toon.kuwahara_radius,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.kuwahara_radius,
                |scene| &mut scene.material.toon.kuwahara_radius,
            )
            .kind(ScalarKind::KuwaharaRadius),
        );
        add_scalar(
            section,
            "Kuwahara strength",
            &toon.kuwahara_strength,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.kuwahara_strength,
                |scene| &mut scene.material.toon.kuwahara_strength,
            )
            .kind(ScalarKind::Unit),
        );
    }

    section.add_wide_control(&shadow_kind_selector(toon.shadow_kind, context));
    section.add_wide_control(&scene_color(
        "Shadow color",
        &toon.shadow_color,
        context,
        ColorAccess::Scene3d(|scene| &mut scene.material.toon.shadow_color),
    ));
    for (label, value, get, get_mut, kind) in [
        (
            "Shadow strength",
            &toon.shadow_strength,
            (|scene: &ObjScene| &scene.material.toon.shadow_strength) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.shadow_strength) as SceneScalarGetMut,
            ScalarKind::Unit,
        ),
        (
            "Darkest tone",
            &toon.shadow_darkest_tone,
            (|scene: &ObjScene| &scene.material.toon.shadow_darkest_tone) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.shadow_darkest_tone)
                as SceneScalarGetMut,
            ScalarKind::ShadowTone,
        ),
    ] {
        add_scalar(
            section,
            label,
            value,
            context,
            SceneScalar::new(get, get_mut).kind(kind),
        );
    }

    match toon.shadow_kind {
        ToonShadowKind::Solid => {}
        ToonShadowKind::Dots => {
            for (label, value, get, get_mut, kind) in [
                (
                    "Dot size",
                    &toon.shadow_dot_size,
                    (|scene: &ObjScene| &scene.material.toon.shadow_dot_size) as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_dot_size)
                        as SceneScalarGetMut,
                    ScalarKind::ShadowPatternSize,
                ),
                (
                    "Dot density",
                    &toon.shadow_dot_density,
                    (|scene: &ObjScene| &scene.material.toon.shadow_dot_density) as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_dot_density)
                        as SceneScalarGetMut,
                    ScalarKind::Frequency,
                ),
                (
                    "Distribution randomness",
                    &toon.shadow_dot_distribution_randomness,
                    (|scene: &ObjScene| &scene.material.toon.shadow_dot_distribution_randomness)
                        as SceneScalarGet,
                    (|scene: &mut ObjScene| {
                        &mut scene.material.toon.shadow_dot_distribution_randomness
                    }) as SceneScalarGetMut,
                    ScalarKind::Unit,
                ),
                (
                    "Size randomness",
                    &toon.shadow_dot_size_randomness,
                    (|scene: &ObjScene| &scene.material.toon.shadow_dot_size_randomness)
                        as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_dot_size_randomness)
                        as SceneScalarGetMut,
                    ScalarKind::Unit,
                ),
            ] {
                add_scalar(
                    section,
                    label,
                    value,
                    context,
                    SceneScalar::new(get, get_mut).kind(kind),
                );
            }
        }
        ToonShadowKind::Lines | ToonShadowKind::Crosshatch => {
            for (label, value, get, get_mut, kind) in [
                (
                    "Line direction",
                    &toon.shadow_line_direction_degrees,
                    (|scene: &ObjScene| &scene.material.toon.shadow_line_direction_degrees)
                        as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_line_direction_degrees)
                        as SceneScalarGetMut,
                    ScalarKind::ClampedDegrees,
                ),
                (
                    "Line width",
                    &toon.shadow_line_width,
                    (|scene: &ObjScene| &scene.material.toon.shadow_line_width) as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_line_width)
                        as SceneScalarGetMut,
                    ScalarKind::ShadowPatternSize,
                ),
                (
                    "Line density",
                    &toon.shadow_line_density,
                    (|scene: &ObjScene| &scene.material.toon.shadow_line_density) as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_line_density)
                        as SceneScalarGetMut,
                    ScalarKind::Frequency,
                ),
                (
                    "Distribution randomness",
                    &toon.shadow_line_distribution_randomness,
                    (|scene: &ObjScene| &scene.material.toon.shadow_line_distribution_randomness)
                        as SceneScalarGet,
                    (|scene: &mut ObjScene| {
                        &mut scene.material.toon.shadow_line_distribution_randomness
                    }) as SceneScalarGetMut,
                    ScalarKind::Unit,
                ),
                (
                    "Width randomness",
                    &toon.shadow_line_width_randomness,
                    (|scene: &ObjScene| &scene.material.toon.shadow_line_width_randomness)
                        as SceneScalarGet,
                    (|scene: &mut ObjScene| &mut scene.material.toon.shadow_line_width_randomness)
                        as SceneScalarGetMut,
                    ScalarKind::Unit,
                ),
            ] {
                add_scalar(
                    section,
                    label,
                    value,
                    context,
                    SceneScalar::new(get, get_mut).kind(kind),
                );
            }
            if toon.shadow_kind == ToonShadowKind::Crosshatch {
                add_scalar(
                    section,
                    "Maximum directions",
                    &toon.shadow_crosshatch_max_directions,
                    context,
                    SceneScalar::new(
                        |scene| &scene.material.toon.shadow_crosshatch_max_directions,
                        |scene| &mut scene.material.toon.shadow_crosshatch_max_directions,
                    )
                    .kind(ScalarKind::DirectionCount),
                );
            }
        }
    }
    if toon.shadow_kind != ToonShadowKind::Solid {
        add_scalar(
            section,
            "Pattern softness",
            &toon.shadow_pattern_softness,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.shadow_pattern_softness,
                |scene| &mut scene.material.toon.shadow_pattern_softness,
            )
            .kind(ScalarKind::ShadowSoftness),
        );
    }

    section.add_wide_control(&scene_color(
        "Rim tint",
        &toon.rim_color,
        context,
        ColorAccess::Scene3d(|scene| &mut scene.material.toon.rim_color),
    ));
    for (label, value, get, get_mut, kind) in [
        (
            "Rim strength",
            &toon.rim_strength,
            (|scene: &ObjScene| &scene.material.toon.rim_strength) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.rim_strength) as SceneScalarGetMut,
            ScalarKind::Nonnegative,
        ),
        (
            "Rim power",
            &toon.rim_power,
            (|scene: &ObjScene| &scene.material.toon.rim_power) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.rim_power) as SceneScalarGetMut,
            ScalarKind::Positive,
        ),
        (
            "Specular size",
            &toon.specular_size,
            (|scene: &ObjScene| &scene.material.toon.specular_size) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.specular_size) as SceneScalarGetMut,
            ScalarKind::Unit,
        ),
        (
            "Specular strength",
            &toon.specular_strength,
            (|scene: &ObjScene| &scene.material.toon.specular_strength) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.specular_strength)
                as SceneScalarGetMut,
            ScalarKind::Nonnegative,
        ),
    ] {
        add_scalar(
            section,
            label,
            value,
            context,
            SceneScalar::new(get, get_mut).kind(kind),
        );
    }
}

fn add_outline_controls(
    section: &InspectorSection,
    material: &PbrMaterial,
    context: &InspectorContext,
) {
    let outline = &material.toon.outline;
    section.add_wide_control(&outline_mode_selector(outline.mode, context));
    if outline.mode == ToonOutlineMode::Off {
        return;
    }

    section.add_wide_control(&outline_method_selector(outline.method, context));
    if matches!(
        outline.method,
        ToonOutlineMethod::RayTraced
            | ToonOutlineMethod::Hybrid
            | ToonOutlineMethod::DifferenceOfGaussians
    ) {
        section.add_wide_control(&outline_quality_selector(
            outline.quality,
            outline.method,
            context,
        ));
    }
    section.add_wide_control(&scene_color(
        "Default outline color",
        &outline.color,
        context,
        ColorAccess::Scene3d(|scene| &mut scene.material.toon.outline.color),
    ));

    if outline.method == ToonOutlineMethod::DifferenceOfGaussians {
        for (label, value, get, get_mut, kind) in [
            (
                "Sigma",
                &outline.dog_inner_radius,
                (|scene: &ObjScene| &scene.material.toon.outline.dog_inner_radius)
                    as SceneScalarGet,
                (|scene: &mut ObjScene| &mut scene.material.toon.outline.dog_inner_radius)
                    as SceneScalarGetMut,
                ScalarKind::OutlineWidth,
            ),
            (
                "Sigma ratio",
                &outline.dog_radius_ratio,
                (|scene: &ObjScene| &scene.material.toon.outline.dog_radius_ratio)
                    as SceneScalarGet,
                (|scene: &mut ObjScene| &mut scene.material.toon.outline.dog_radius_ratio)
                    as SceneScalarGetMut,
                ScalarKind::RadiusRatio,
            ),
            (
                "Sensitivity",
                &outline.dog_threshold,
                (|scene: &ObjScene| &scene.material.toon.outline.dog_threshold) as SceneScalarGet,
                (|scene: &mut ObjScene| &mut scene.material.toon.outline.dog_threshold)
                    as SceneScalarGetMut,
                ScalarKind::DogSensitivity,
            ),
            (
                "Sharpness",
                &outline.dog_sharpness,
                (|scene: &ObjScene| &scene.material.toon.outline.dog_sharpness) as SceneScalarGet,
                (|scene: &mut ObjScene| &mut scene.material.toon.outline.dog_sharpness)
                    as SceneScalarGetMut,
                ScalarKind::DogSharpness,
            ),
        ] {
            add_scalar(
                section,
                label,
                value,
                context,
                SceneScalar::new(get, get_mut).kind(kind),
            );
        }
    } else {
        add_scalar(
            section,
            "Outline width",
            &outline.width,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.outline.width,
                |scene| &mut scene.material.toon.outline.width,
            )
            .kind(ScalarKind::OutlineWidth),
        );
    }

    for (label, value, get, get_mut, kind) in [
        (
            "Outline opacity",
            &outline.opacity,
            (|scene: &ObjScene| &scene.material.toon.outline.opacity) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.opacity) as SceneScalarGetMut,
            ScalarKind::Unit,
        ),
        (
            "Aggressiveness",
            &outline.aggressiveness,
            (|scene: &ObjScene| &scene.material.toon.outline.aggressiveness) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.aggressiveness)
                as SceneScalarGetMut,
            ScalarKind::Aggressiveness,
        ),
        (
            "Offset variation",
            &outline.offset_variation,
            (|scene: &ObjScene| &scene.material.toon.outline.offset_variation) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.offset_variation)
                as SceneScalarGetMut,
            ScalarKind::PixelVariation,
        ),
        (
            "Offset frequency",
            &outline.offset_frequency,
            (|scene: &ObjScene| &scene.material.toon.outline.offset_frequency) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.offset_frequency)
                as SceneScalarGetMut,
            ScalarKind::Frequency,
        ),
        (
            "Width variation",
            &outline.width_variation,
            (|scene: &ObjScene| &scene.material.toon.outline.width_variation) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.width_variation)
                as SceneScalarGetMut,
            ScalarKind::PixelVariation,
        ),
        (
            "Width frequency",
            &outline.width_frequency,
            (|scene: &ObjScene| &scene.material.toon.outline.width_frequency) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.width_frequency)
                as SceneScalarGetMut,
            ScalarKind::Frequency,
        ),
        (
            "Noise seed",
            &outline.noise_seed,
            (|scene: &ObjScene| &scene.material.toon.outline.noise_seed) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.noise_seed)
                as SceneScalarGetMut,
            ScalarKind::Seed,
        ),
        (
            "Noise evolution",
            &outline.noise_evolution,
            (|scene: &ObjScene| &scene.material.toon.outline.noise_evolution) as SceneScalarGet,
            (|scene: &mut ObjScene| &mut scene.material.toon.outline.noise_evolution)
                as SceneScalarGetMut,
            ScalarKind::Plain,
        ),
    ] {
        add_scalar(
            section,
            label,
            value,
            context,
            SceneScalar::new(get, get_mut).kind(kind),
        );
    }

    if outline.mode == ToonOutlineMode::SilhouetteAndCreases
        && outline.method != ToonOutlineMethod::Fresnel
    {
        add_scalar(
            section,
            "Depth threshold",
            &outline.depth_threshold,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.outline.depth_threshold,
                |scene| &mut scene.material.toon.outline.depth_threshold,
            )
            .kind(ScalarKind::Unit),
        );
        add_scalar(
            section,
            "Normal angle",
            &outline.normal_angle_degrees,
            context,
            SceneScalar::new(
                |scene| &scene.material.toon.outline.normal_angle_degrees,
                |scene| &mut scene.material.toon.outline.normal_angle_degrees,
            )
            .kind(ScalarKind::ClampedDegrees),
        );
    }
}

fn camera_controls(value: &Camera3d, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls(|section| {
        let custom_source = crate::camera_source::add_controls(section, &value.source, context);
        if custom_source {
            section.add_wide_control(&projection_selector(value.projection, context));
            section.add_wide_control(&anti_aliasing_selector(value.anti_aliasing, context));
        }
        add_vec3(
            section,
            "Position",
            &value.position,
            context,
            Vec3Target::builder(
                |scene| &scene.camera.position,
                |scene| &mut scene.camera.position,
            )
            .build(),
        );
        add_vec3(
            section,
            "Rotation",
            &value.rotation_degrees,
            context,
            Vec3Target::builder(
                |scene| &scene.camera.rotation_degrees,
                |scene| &mut scene.camera.rotation_degrees,
            )
            .degrees()
            .build(),
        );
        if !custom_source {
            return;
        }
        match value.projection {
            CameraProjection::Perspective => {
                add_scalar(
                    section,
                    "Focal length",
                    &value.vertical_fov_degrees,
                    context,
                    SceneScalar::new(
                        |scene| &scene.camera.vertical_fov_degrees,
                        |scene| &mut scene.camera.vertical_fov_degrees,
                    )
                    .kind(ScalarKind::FocalLength),
                );
                add_scalar(
                    section,
                    "Focus distance (0 = off)",
                    &value.focus_distance,
                    context,
                    SceneScalar::new(
                        |scene| &scene.camera.focus_distance,
                        |scene| &mut scene.camera.focus_distance,
                    )
                    .kind(ScalarKind::Nonnegative),
                );
                add_scalar(
                    section,
                    "Aperture",
                    &value.f_stop,
                    context,
                    SceneScalar::new(
                        |scene| &scene.camera.f_stop,
                        |scene| &mut scene.camera.f_stop,
                    )
                    .kind(ScalarKind::FStop),
                );
            }
            CameraProjection::Orthographic => add_scalar(
                section,
                "Orthographic height",
                &value.orthographic_height,
                context,
                SceneScalar::new(
                    |scene| &scene.camera.orthographic_height,
                    |scene| &mut scene.camera.orthographic_height,
                )
                .kind(ScalarKind::Positive),
            ),
            CameraProjection::Cylindrical => add_scalar(
                section,
                "Vertical FOV",
                &value.vertical_fov_degrees,
                context,
                SceneScalar::new(
                    |scene| &scene.camera.vertical_fov_degrees,
                    |scene| &mut scene.camera.vertical_fov_degrees,
                )
                .kind(ScalarKind::Fov),
            ),
            CameraProjection::Equirectangular => {}
            CameraProjection::Fisheye => add_scalar(
                section,
                "FOV",
                &value.vertical_fov_degrees,
                context,
                SceneScalar::new(
                    |scene| &scene.camera.vertical_fov_degrees,
                    |scene| &mut scene.camera.vertical_fov_degrees,
                )
                .kind(ScalarKind::FisheyeFov),
            ),
        }
        section.add_wide_control(&background_plane_selector(
            value.background_plane_enabled,
            context,
        ));
        if value.background_plane_enabled {
            add_scalar(
                section,
                "Background distance",
                &value.background_distance,
                context,
                SceneScalar::new(
                    |scene| &scene.camera.background_distance,
                    |scene| &mut scene.camera.background_distance,
                )
                .kind(ScalarKind::Positive),
            );
            add_scalar(
                section,
                "Background intensity",
                &value.background_intensity,
                context,
                SceneScalar::new(
                    |scene| &scene.camera.background_intensity,
                    |scene| &mut scene.camera.background_intensity,
                )
                .kind(ScalarKind::Nonnegative),
            );
            section.add_wide_control(&background_address_selector(
                value.background_address_mode,
                context,
            ));
        }
        add_scalar(
            section,
            "Exposure",
            &value.exposure_ev,
            context,
            SceneScalar::new(
                |scene| &scene.camera.exposure_ev,
                |scene| &mut scene.camera.exposure_ev,
            )
            .kind(ScalarKind::Exposure),
        );
    })
}

fn environment_controls(value: &Environment3d, context: &InspectorContext) -> Vec<gtk::Widget> {
    let source = value.effective_source();
    controls(|section| {
        section.add_wide_control(&environment_source_selector(value, context));
        if source == EnvironmentSource::Image {
            section.add_wide_control(&environment_picker(value, context));
        }
        if source == EnvironmentSource::Black {
            section.add_wide_control(&scene_color(
                "Color",
                &value.solid_color,
                context,
                ColorAccess::Scene3d(|scene| &mut scene.environment.solid_color),
            ));
        } else {
            add_vec3(
                section,
                "Rotation",
                &value.rotation_degrees,
                context,
                Vec3Target::builder(
                    |scene| &scene.environment.rotation_degrees,
                    |scene| &mut scene.environment.rotation_degrees,
                )
                .degrees()
                .build(),
            );
        }
        add_scalar(
            section,
            "Intensity",
            &value.intensity,
            context,
            SceneScalar::new(
                |scene| &scene.environment.intensity,
                |scene| &mut scene.environment.intensity,
            )
            .kind(ScalarKind::Nonnegative),
        );
    })
}

fn environment_source_selector(value: &Environment3d, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Source",
        value.effective_source(),
        [
            (EnvironmentSource::Composite, "Composite"),
            (EnvironmentSource::Image, "HDRI"),
            (EnvironmentSource::Black, "Solid color"),
        ],
        move |source| {
            update_static(
                &context,
                "edit-scene-3d-environment-source",
                move |scene| scene.environment.source = Some(source),
                true,
            )
        },
    )
}

fn controls(add: impl FnOnce(&InspectorSection)) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    add(&section);
    vec![section.into_widget()]
}

fn add_vec3(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<glam::Vec3>,
    context: &InspectorContext,
    target: Vec3Target,
) {
    section.add_wide_control(&vec3_control(label, value, context, target));
}

struct SceneScalar {
    get: SceneScalarGet,
    get_mut: SceneScalarGetMut,
    kind: ScalarKind,
}

impl SceneScalar {
    fn new(get: SceneScalarGet, get_mut: SceneScalarGetMut) -> Self {
        Self {
            get,
            get_mut,
            kind: ScalarKind::Plain,
        }
    }

    fn kind(mut self, kind: ScalarKind) -> Self {
        self.kind = kind;
        self
    }
}

fn add_scalar(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    scalar: SceneScalar,
) {
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        ScalarTarget {
            access: ScalarAccess::Scene3d {
                get: scalar.get,
                get_mut: scalar.get_mut,
            },
            scope_id: Some(value.id),
            local_time: crate::video::visual_local_time,
            duration: scene_duration,
            refresh: ProjectChange {
                video: true,
                ..ProjectChange::default()
            },
            commit_name: "edit-scene-3d-scalar",
        },
        scalar_spec(scalar.kind),
    ));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarKind {
    Plain,
    Nonnegative,
    Positive,
    Unit,
    Bands,
    ColorLevels,
    KuwaharaRadius,
    ShadowTone,
    ShadowPatternSize,
    ShadowSoftness,
    DirectionCount,
    FocalLength,
    Fov,
    FisheyeFov,
    FStop,
    Exposure,
    OutlineWidth,
    PixelVariation,
    Frequency,
    Aggressiveness,
    Seed,
    RadiusRatio,
    DogSensitivity,
    DogSharpness,
    ClampedDegrees,
}

fn scalar_spec(kind: ScalarKind) -> ScalarSpec {
    ScalarSpec {
        drag_step: match kind {
            ScalarKind::Frequency | ScalarKind::DogSensitivity => 0.001,
            ScalarKind::Unit | ScalarKind::RadiusRatio => 0.01,
            ScalarKind::Bands
            | ScalarKind::ColorLevels
            | ScalarKind::KuwaharaRadius
            | ScalarKind::DirectionCount
            | ScalarKind::Seed => 1.0,
            _ => 0.1,
        },
        digits: match kind {
            ScalarKind::Frequency | ScalarKind::DogSensitivity => 3,
            ScalarKind::Bands
            | ScalarKind::ColorLevels
            | ScalarKind::KuwaharaRadius
            | ScalarKind::DirectionCount
            | ScalarKind::Seed => 0,
            _ => 2,
        },
        integer: matches!(
            kind,
            ScalarKind::Bands
                | ScalarKind::ColorLevels
                | ScalarKind::KuwaharaRadius
                | ScalarKind::DirectionCount
                | ScalarKind::Seed
        ),
        width_chars: 9,
        minimum: match kind {
            ScalarKind::Nonnegative | ScalarKind::Unit => Some(0.0),
            ScalarKind::Positive => Some(0.001),
            ScalarKind::Bands | ScalarKind::ColorLevels => Some(2.0),
            ScalarKind::KuwaharaRadius => Some(0.0),
            ScalarKind::ShadowTone => Some(shrimply_scene_3d::MIN_TOON_SHADOW_TONE as f64),
            ScalarKind::ShadowPatternSize => Some(0.25),
            ScalarKind::ShadowSoftness => Some(0.0),
            ScalarKind::DirectionCount => Some(1.0),
            ScalarKind::FocalLength | ScalarKind::Fov | ScalarKind::FisheyeFov => Some(1.0),
            ScalarKind::FStop => Some(shrimply_scene_3d::MIN_F_STOP as f64),
            ScalarKind::Exposure => Some(shrimply_scene_3d::MIN_EXPOSURE_EV as f64),
            ScalarKind::OutlineWidth => Some(0.25),
            ScalarKind::PixelVariation | ScalarKind::Seed => Some(0.0),
            ScalarKind::Frequency => Some(0.001),
            ScalarKind::Aggressiveness => Some(0.1),
            ScalarKind::RadiusRatio => Some(1.01),
            ScalarKind::DogSensitivity => Some(0.0),
            ScalarKind::DogSharpness => Some(1.0),
            ScalarKind::ClampedDegrees => Some(0.0),
            ScalarKind::Plain => None,
        },
        maximum: match kind {
            ScalarKind::Unit => Some(1.0),
            ScalarKind::Bands => Some(16.0),
            ScalarKind::ColorLevels => Some(32.0),
            ScalarKind::KuwaharaRadius => Some(4.0),
            ScalarKind::ShadowTone => Some(1.0),
            ScalarKind::ShadowPatternSize => Some(64.0),
            ScalarKind::ShadowSoftness => Some(4.0),
            ScalarKind::DirectionCount => Some(6.0),
            ScalarKind::FocalLength => Some(shrimply_scene_3d::focal_length_mm(1.0)),
            ScalarKind::Fov => Some(179.0),
            ScalarKind::FisheyeFov => Some(360.0),
            ScalarKind::FStop => Some(shrimply_scene_3d::MAX_F_STOP as f64),
            ScalarKind::Exposure => Some(shrimply_scene_3d::MAX_EXPOSURE_EV as f64),
            ScalarKind::OutlineWidth | ScalarKind::PixelVariation => Some(16.0),
            ScalarKind::Frequency => Some(1.0),
            ScalarKind::Aggressiveness => Some(8.0),
            ScalarKind::Seed => Some(u32::MAX as f64),
            ScalarKind::RadiusRatio => Some(4.0),
            ScalarKind::DogSensitivity => Some(0.25),
            ScalarKind::DogSharpness => Some(64.0),
            ScalarKind::ClampedDegrees => Some(180.0),
            _ => None,
        },
        unit_name: match kind {
            ScalarKind::FocalLength => Some("mm"),
            ScalarKind::Fov | ScalarKind::FisheyeFov | ScalarKind::ClampedDegrees => Some("deg"),
            ScalarKind::OutlineWidth | ScalarKind::PixelVariation => Some("px"),
            ScalarKind::KuwaharaRadius
            | ScalarKind::ShadowPatternSize
            | ScalarKind::ShadowSoftness => Some("px"),
            ScalarKind::Frequency => Some("/px"),
            _ => None,
        },
        rotating_icon: None,
        display: match kind {
            ScalarKind::FocalLength => |value| shrimply_scene_3d::focal_length_mm(value as f64),
            _ => |value| value as f64,
        },
        store: match kind {
            ScalarKind::FocalLength => {
                |value| shrimply_scene_3d::vertical_fov_degrees(value) as f32
            }
            _ => |value| value as f32,
        },
        clamp: match kind {
            ScalarKind::Nonnegative => |value| value.max(0.0),
            ScalarKind::Positive => |value| value.max(0.001),
            ScalarKind::Unit => |value| value.clamp(0.0, 1.0),
            ScalarKind::Bands => |value| value.round().clamp(2.0, 16.0),
            ScalarKind::ColorLevels => |value| value.round().clamp(2.0, 32.0),
            ScalarKind::KuwaharaRadius => |value| value.round().clamp(0.0, 4.0),
            ScalarKind::ShadowTone => {
                |value| value.clamp(shrimply_scene_3d::MIN_TOON_SHADOW_TONE, 1.0)
            }
            ScalarKind::ShadowPatternSize => |value| value.clamp(0.25, 64.0),
            ScalarKind::ShadowSoftness => |value| value.clamp(0.0, 4.0),
            ScalarKind::DirectionCount => |value| value.round().clamp(1.0, 6.0),
            ScalarKind::FocalLength | ScalarKind::Fov => |value| value.clamp(1.0, 179.0),
            ScalarKind::FisheyeFov => |value| value.clamp(1.0, 360.0),
            ScalarKind::FStop => {
                |value| value.clamp(shrimply_scene_3d::MIN_F_STOP, shrimply_scene_3d::MAX_F_STOP)
            }
            ScalarKind::Exposure => |value| {
                value.clamp(
                    shrimply_scene_3d::MIN_EXPOSURE_EV,
                    shrimply_scene_3d::MAX_EXPOSURE_EV,
                )
            },
            ScalarKind::OutlineWidth => |value| value.clamp(0.25, 16.0),
            ScalarKind::PixelVariation => |value| value.clamp(0.0, 16.0),
            ScalarKind::Frequency => |value| value.clamp(0.001, 1.0),
            ScalarKind::Aggressiveness => |value| value.clamp(0.1, 8.0),
            ScalarKind::Seed => |value| value.round().clamp(0.0, u32::MAX as f32),
            ScalarKind::RadiusRatio => |value| value.clamp(1.01, 4.0),
            ScalarKind::DogSensitivity => |value| value.clamp(0.0, 0.25),
            ScalarKind::DogSharpness => |value| value.clamp(1.0, 64.0),
            ScalarKind::ClampedDegrees => |value| value.clamp(0.0, 180.0),
            ScalarKind::Plain => |value| value,
        },
    }
}

fn scene_color(
    label: &str,
    value: &TimelineValue<shrimply_core::Color<u8>>,
    context: &InspectorContext,
    access: ColorAccess,
) -> gtk::Widget {
    color_control(
        label,
        value,
        context,
        ColorTarget {
            access,
            scope_id: Some(value.id),
            local_time: crate::video::visual_local_time,
            duration: scene_duration,
            refresh: ProjectChange {
                video: true,
                ..ProjectChange::default()
            },
            commit_name: "edit-scene-3d-color",
        },
    )
}

fn background_address_selector(
    value: BackgroundAddressMode,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Background tiling",
        value,
        [
            (BackgroundAddressMode::ExtendEdge, "Extend edge"),
            (BackgroundAddressMode::Repeat, "Repeat"),
            (BackgroundAddressMode::Mirror, "Mirror"),
        ],
        move |address_mode| {
            update_static(
                &context,
                "edit-scene-3d-background-addressing",
                move |scene| scene.camera.background_address_mode = address_mode,
                true,
            )
        },
    )
}

fn background_plane_selector(value: bool, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Background plane",
        value,
        [(false, "Off"), (true, "On")],
        move |enabled| {
            update_static(
                &context,
                "edit-scene-3d-background-plane",
                move |scene| scene.camera.background_plane_enabled = enabled,
                true,
            )
        },
    )
}

fn projection_selector(value: CameraProjection, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Projection", value, move |projection| {
        update_static(
            &context,
            "edit-scene-3d-projection",
            move |scene| scene.camera.projection = projection,
            true,
        )
    })
}

fn anti_aliasing_selector(value: AntiAliasing, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Anti-aliasing",
        value,
        [
            (AntiAliasing::None, "Off"),
            (AntiAliasing::RotatedGrid2x, "2× rotated grid"),
            (AntiAliasing::Grid4x, "4× grid"),
            (AntiAliasing::Stochastic8x, "8× stochastic"),
        ],
        move |anti_aliasing| {
            update_static(
                &context,
                "edit-scene-3d-anti-aliasing",
                move |scene| scene.camera.anti_aliasing = anti_aliasing,
                false,
            )
        },
    )
}

fn shading_selector(value: ShadingModel, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Shader", value, move |model| {
        update_static(
            &context,
            "edit-scene-3d-shading-model",
            move |scene| scene.material.shading_model = model,
            true,
        )
    })
}

fn texture_filter_selector(value: ToonTextureFilter, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Texture filter",
        value,
        [
            (ToonTextureFilter::Direct, "Direct"),
            (ToonTextureFilter::Kuwahara, "Kuwahara · painterly"),
        ],
        move |filter| {
            update_static(
                &context,
                "edit-scene-3d-texture-filter",
                move |scene| scene.material.toon.texture_filter = filter,
                true,
            )
        },
    )
}

fn shadow_kind_selector(value: ToonShadowKind, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Shadow kind",
        value,
        [
            (ToonShadowKind::Solid, "Solid"),
            (ToonShadowKind::Dots, "Dots"),
            (ToonShadowKind::Lines, "Lines"),
            (ToonShadowKind::Crosshatch, "Crosshatch"),
        ],
        move |kind| {
            update_static(
                &context,
                "edit-scene-3d-shadow-kind",
                move |scene| scene.material.toon.shadow_kind = kind,
                true,
            )
        },
    )
}

fn outline_mode_selector(value: ToonOutlineMode, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Outline",
        value,
        [
            (ToonOutlineMode::Off, "Off"),
            (ToonOutlineMode::Silhouette, "Silhouette"),
            (
                ToonOutlineMode::SilhouetteAndCreases,
                "Silhouette + creases",
            ),
        ],
        move |mode| {
            update_static(
                &context,
                "edit-scene-3d-outline-mode",
                move |scene| scene.material.toon.outline.mode = mode,
                true,
            )
        },
    )
}

fn outline_method_selector(value: ToonOutlineMethod, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Outline method",
        value,
        [
            (ToonOutlineMethod::RayTraced, "Radial probes"),
            (ToonOutlineMethod::Fresnel, "Fresnel contour"),
            (ToonOutlineMethod::Hybrid, "Radial + Fresnel"),
            (ToonOutlineMethod::Sobel, "Sobel"),
            (ToonOutlineMethod::RobertsCross, "Roberts cross"),
            (ToonOutlineMethod::DifferenceOfGaussians, "XDoG"),
            (
                ToonOutlineMethod::RegionBoundary,
                "Feature contours · multipass",
            ),
        ],
        move |method| {
            update_static(
                &context,
                "edit-scene-3d-outline-method",
                move |scene| scene.material.toon.outline.method = method,
                true,
            )
        },
    )
}

fn outline_quality_selector(
    value: ToonOutlineQuality,
    method: ToonOutlineMethod,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = context.detached();
    let options = if method == ToonOutlineMethod::DifferenceOfGaussians {
        [
            (ToonOutlineQuality::Standard, "Standard · 12 samples"),
            (ToonOutlineQuality::High, "High · 24 samples"),
            (ToonOutlineQuality::Ultra, "Ultra · 48 samples"),
        ]
    } else {
        [
            (ToonOutlineQuality::Standard, "Standard · 4 probes"),
            (ToonOutlineQuality::High, "High · 8 probes"),
            (ToonOutlineQuality::Ultra, "Ultra · 16 probes"),
        ]
    };
    selector("Outline quality", value, options, move |quality| {
        update_static(
            &context,
            "edit-scene-3d-outline-quality",
            move |scene| scene.material.toon.outline.quality = quality,
            false,
        )
    })
}

fn path_tracing_selector(value: PathTracingMode, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Path tracing",
        value,
        [
            (PathTracingMode::Off, "Off"),
            (PathTracingMode::Samples1, "1"),
            (PathTracingMode::Samples2, "2"),
            (PathTracingMode::Preview, "4"),
            (PathTracingMode::Samples8, "8"),
            (PathTracingMode::Quality, "16"),
            (PathTracingMode::Samples32, "32"),
            (PathTracingMode::Samples64, "64"),
            (PathTracingMode::Samples128, "128"),
        ],
        move |mode| {
            update_static(
                &context,
                "edit-scene-3d-path-tracing",
                move |scene| scene.material.path_tracing = mode,
                true,
            )
        },
    )
}

fn optix_denoising_selector(value: bool, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "OptiX denoiser",
        value,
        [(false, "Off"), (true, "On")],
        move |enabled| {
            update_static(
                &context,
                "edit-scene-3d-optix-denoising",
                move |scene| scene.material.optix_denoising = enabled,
                true,
            )
        },
    )
}

fn light_sampling_selector(value: LightSamplingQuality, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    selector(
        "Shadow quality",
        value,
        [
            (LightSamplingQuality::Rays1, "1"),
            (LightSamplingQuality::Rays2, "2"),
            (LightSamplingQuality::Standard, "4"),
            (LightSamplingQuality::High, "8"),
            (LightSamplingQuality::Ultra, "16"),
            (LightSamplingQuality::Rays32, "32"),
            (LightSamplingQuality::Rays64, "64"),
        ],
        move |quality| {
            update_static(
                &context,
                "edit-scene-3d-light-sampling",
                move |scene| scene.material.light_sampling_quality = quality,
                true,
            )
        },
    )
}

fn environment_picker(value: &Environment3d, context: &InspectorContext) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let choose = gtk::Button::with_label(if value.file.is_some() {
        "Replace image"
    } else {
        "Select image"
    });
    let clear = gtk::Button::with_label(tr!("Clear image").as_ref());
    clear.set_sensitive(value.file.is_some());
    row.append(&choose);
    row.append(&clear);
    let choose_context = context.detached();
    choose.connect_clicked(move |_| {
        let label = "Select environment image";
        let filter = gtk::FileFilter::new();
        filter.set_name_i18n("Environment images");
        for pattern in [
            "*.png", "*.jpg", "*.jpeg", "*.webp", "*.avif", "*.hdr", "*.exr",
        ] {
            filter.add_pattern(pattern);
        }
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(tr!(label).as_ref())
            .filters(&filters)
            .build();
        let context = choose_context.clone();
        shrimply_ui_foundation::file_picker::open(
            label,
            &dialog,
            None::<&gtk::Window>,
            move |result| {
                let Ok(file) = result else {
                    return;
                };
                let Some(path) = file.path() else {
                    return;
                };
                let error = validate_environment(&path).err();
                let accepted = error.is_none();
                update_static(
                    &context,
                    "edit-scene-3d-environment-file",
                    move |scene| {
                        scene.environment.file = accepted.then(|| path.into());
                        if accepted {
                            scene.environment.source = Some(EnvironmentSource::Image);
                        }
                    },
                    true,
                );
                if let Some(error) = error {
                    adw::AlertDialog::new(Some("Could not use environment image"), Some(&error))
                        .present(None::<&gtk::Widget>);
                }
            },
        );
    });
    let clear_context = context.detached();
    clear.connect_clicked(move |_| {
        update_static(
            &clear_context,
            "clear-scene-3d-environment-file",
            |scene| {
                scene.environment.file = None;
                scene.environment.source = Some(EnvironmentSource::Composite);
            },
            true,
        )
    });
    crate::ui::control_row("Image", &row)
}

fn validate_environment(path: &std::path::Path) -> Result<(), String> {
    let mut input = ffmpeg::format::input(path).map_err(|error| error.to_string())?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| "image has no decodable video stream".to_string())?;
    let stream_index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().video())
        .map_err(|error| error.to_string())?;
    let mut frame = ffmpeg::frame::Video::empty();
    for (stream, packet) in input.packets() {
        if stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| error.to_string())?;
        if decoder.receive_frame(&mut frame).is_ok() {
            return Ok(());
        }
    }
    decoder.send_eof().map_err(|error| error.to_string())?;
    decoder
        .receive_frame(&mut frame)
        .map_err(|error| format!("could not decode an image frame: {error}"))
}

fn update_static(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut ObjScene),
    inspector: bool,
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(scene) = selected_scene_mut(&mut project, key.clone()) else {
        return;
    };
    update(scene);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector,
            ..ProjectChange::default()
        },
    );
}

fn reset(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut ObjScene),
) {
    update_static(context, commit_name, update, true)
}

fn selected_scene_mut(project: &mut Project, key: SelectedItem) -> Option<&mut ObjScene> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Obj(scene) = &mut item.content else {
        return None;
    };
    Some(scene)
}

fn scene_duration(project: &Project, key: SelectedItem) -> Option<Time> {
    let item = project.video_item(&key)?;
    generated_item_keyframe_span(item)
        .map(|(start, end)| end.saturating_sub(start))
        .or_else(|| crate::video::visual_duration(project, key))
}
