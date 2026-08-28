use shrimply_3dgs::{
    AnimatedTransform3d as Transform3d, Camera3d, CameraProjection, GaussianScene,
};
use shrimply_core::timeline_value::TimelineValue;
use shrimply_project::project::{Project, Time, VideoItemContent, generated_item_keyframe_span};

use crate::InspectedItem as SelectedItem;
use crate::InspectorContext;
use crate::item::{DefaultInspectorItem, InspectorListItem};
use crate::player_state::{self, ProjectChange};
use crate::section::InspectorSection;
use crate::selector::enum_selector;
use crate::timeline_value::scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control};
use crate::timeline_value::step::{StepTarget, step_control};
use crate::timeline_value::vector::vec3::{Vec3Target, control as vec3_control};

pub(super) fn items(scene: &GaussianScene, _context: &InspectorContext) -> Vec<InspectorListItem> {
    vec![
        DefaultInspectorItem::new(
            "gaussian-model",
            "Model",
            scene.model.clone(),
            model_controls,
            |context, model| {
                reset(context, "reset-gaussian-model", move |scene| {
                    scene.model = model
                })
            },
        )
        .boxed(),
        DefaultInspectorItem::new(
            "gaussian-camera",
            "Camera",
            scene.camera.clone(),
            camera_controls,
            |context, camera| {
                reset(context, "reset-gaussian-camera", move |scene| {
                    scene.camera = camera
                })
            },
        )
        .boxed(),
    ]
}

fn model_controls(value: &Transform3d, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls(|section| {
        add_vec3(
            section,
            "Position",
            &value.position,
            context,
            Vec3Target::item_builder(model_position, model_position_mut).build(),
        );
        add_vec3(
            section,
            "Anchor",
            &value.anchor,
            context,
            Vec3Target::item_builder(model_anchor, model_anchor_mut).build(),
        );
        add_vec3(
            section,
            "Rotation",
            &value.rotation_degrees,
            context,
            Vec3Target::item_builder(model_rotation, model_rotation_mut)
                .degrees()
                .build(),
        );
        section.add_wide_control(&step_control(
            "Rotation order",
            &value.rotation_order,
            context,
            StepTarget::new(
                |project, key| Some(&selected_scene(project, key.clone())?.model.rotation_order),
                |project, key| {
                    Some(
                        &mut selected_scene_mut(project, key.clone())?
                            .model
                            .rotation_order,
                    )
                },
                "edit-gaussian-rotation-order",
                video_change(),
            ),
        ));
        add_vec3(
            section,
            "Scale",
            &value.scale,
            context,
            Vec3Target::item_builder(model_scale, model_scale_mut)
                .minimum(0.0)
                .lock()
                .build(),
        );
    })
}

fn camera_controls(value: &Camera3d, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls(|section| {
        let custom_source = crate::camera_source::add_controls(section, &value.source, context);
        if custom_source {
            section.add_wide_control(&projection_selector(value.projection, context));
        }
        add_vec3(
            section,
            "Position",
            &value.position,
            context,
            Vec3Target::item_builder(camera_position, camera_position_mut).build(),
        );
        add_vec3(
            section,
            "Rotation",
            &value.rotation_degrees,
            context,
            Vec3Target::item_builder(camera_rotation, camera_rotation_mut)
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
                    camera_fov,
                    camera_fov_mut,
                    ScalarKind::FocalLength,
                );
                add_scalar(
                    section,
                    "Focus distance (0 = off)",
                    &value.focus_distance,
                    context,
                    camera_focus_distance,
                    camera_focus_distance_mut,
                    ScalarKind::Nonnegative,
                );
                add_scalar(
                    section,
                    "Aperture",
                    &value.f_stop,
                    context,
                    camera_f_stop,
                    camera_f_stop_mut,
                    ScalarKind::FStop,
                );
            }
            CameraProjection::Orthographic => add_scalar(
                section,
                "Orthographic height",
                &value.orthographic_height,
                context,
                camera_orthographic_height,
                camera_orthographic_height_mut,
                ScalarKind::Positive,
            ),
            CameraProjection::Cylindrical => add_scalar(
                section,
                "Vertical FOV",
                &value.vertical_fov_degrees,
                context,
                camera_fov,
                camera_fov_mut,
                ScalarKind::Fov,
            ),
            CameraProjection::Equirectangular => {}
            CameraProjection::Fisheye => add_scalar(
                section,
                "FOV",
                &value.vertical_fov_degrees,
                context,
                camera_fov,
                camera_fov_mut,
                ScalarKind::FisheyeFov,
            ),
        }
        add_scalar(
            section,
            "Exposure",
            &value.exposure_ev,
            context,
            camera_exposure,
            camera_exposure_mut,
            ScalarKind::Exposure,
        );
    })
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

type ScalarGet = for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<f32>>;
type ScalarGetMut = for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<f32>>;

fn add_scalar(
    section: &InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    get: ScalarGet,
    get_mut: ScalarGetMut,
    kind: ScalarKind,
) {
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        ScalarTarget {
            access: ScalarAccess::Item { get, get_mut },
            scope_id: Some(value.id),
            local_time: crate::video::visual_local_time,
            duration: scene_duration,
            refresh: video_change(),
            commit_name: "edit-gaussian-scalar",
        },
        scalar_spec(kind),
    ));
}

#[derive(Clone, Copy)]
enum ScalarKind {
    Nonnegative,
    Positive,
    FocalLength,
    Fov,
    FisheyeFov,
    FStop,
    Exposure,
}

fn scalar_spec(kind: ScalarKind) -> ScalarSpec {
    ScalarSpec {
        drag_step: 0.1,
        digits: 2,
        integer: false,
        width_chars: 9,
        minimum: match kind {
            ScalarKind::Nonnegative => Some(0.0),
            ScalarKind::Positive => Some(0.001),
            ScalarKind::FocalLength | ScalarKind::Fov | ScalarKind::FisheyeFov => Some(1.0),
            ScalarKind::FStop => Some(shrimply_3dgs::MIN_F_STOP as f64),
            ScalarKind::Exposure => Some(shrimply_3dgs::MIN_EXPOSURE_EV as f64),
        },
        maximum: match kind {
            ScalarKind::FocalLength => Some(shrimply_3dgs::focal_length_mm(1.0)),
            ScalarKind::Fov => Some(179.0),
            ScalarKind::FisheyeFov => Some(360.0),
            ScalarKind::FStop => Some(shrimply_3dgs::MAX_F_STOP as f64),
            ScalarKind::Exposure => Some(shrimply_3dgs::MAX_EXPOSURE_EV as f64),
            _ => None,
        },
        unit_name: match kind {
            ScalarKind::FocalLength => Some("mm"),
            ScalarKind::Fov | ScalarKind::FisheyeFov => Some("deg"),
            _ => None,
        },
        rotating_icon: None,
        display: match kind {
            ScalarKind::FocalLength => |value| shrimply_3dgs::focal_length_mm(value as f64),
            _ => |value| value as f64,
        },
        store: match kind {
            ScalarKind::FocalLength => |value| shrimply_3dgs::vertical_fov_degrees(value) as f32,
            _ => |value| value as f32,
        },
        clamp: match kind {
            ScalarKind::Nonnegative => |value| value.max(0.0),
            ScalarKind::Positive => |value| value.max(0.001),
            ScalarKind::FocalLength | ScalarKind::Fov => |value| value.clamp(1.0, 179.0),
            ScalarKind::FisheyeFov => |value| value.clamp(1.0, 360.0),
            ScalarKind::FStop => {
                |value| value.clamp(shrimply_3dgs::MIN_F_STOP, shrimply_3dgs::MAX_F_STOP)
            }
            ScalarKind::Exposure => |value| {
                value.clamp(
                    shrimply_3dgs::MIN_EXPOSURE_EV,
                    shrimply_3dgs::MAX_EXPOSURE_EV,
                )
            },
        },
    }
}

fn projection_selector(value: CameraProjection, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Projection", value, move |projection| {
        update_static(
            &context,
            "edit-gaussian-projection",
            move |scene| scene.camera.projection = projection,
            true,
        )
    })
}

fn reset(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut GaussianScene),
) {
    update_static(context, commit_name, update, true)
}

fn update_static(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut GaussianScene),
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

fn selected_scene(project: &Project, key: SelectedItem) -> Option<&GaussianScene> {
    let item = project.video_item(&key)?;
    let VideoItemContent::Gaussian(scene) = &item.content else {
        return None;
    };
    Some(scene)
}

fn selected_scene_mut(project: &mut Project, key: SelectedItem) -> Option<&mut GaussianScene> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Gaussian(scene) = &mut item.content else {
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

fn video_change() -> ProjectChange {
    ProjectChange {
        video: true,
        ..ProjectChange::default()
    }
}

macro_rules! vec3_accessors {
    ($get:ident, $get_mut:ident, $field:ident.$value:ident) => {
        fn $get(project: &Project, key: SelectedItem) -> Option<&TimelineValue<glam::Vec3>> {
            Some(&selected_scene(project, key.clone())?.$field.$value)
        }

        fn $get_mut(
            project: &mut Project,
            key: SelectedItem,
        ) -> Option<&mut TimelineValue<glam::Vec3>> {
            Some(&mut selected_scene_mut(project, key.clone())?.$field.$value)
        }
    };
}

macro_rules! scalar_accessors {
    ($get:ident, $get_mut:ident, $value:ident) => {
        fn $get(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
            Some(&selected_scene(project, key.clone())?.camera.$value)
        }

        fn $get_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<f32>> {
            Some(&mut selected_scene_mut(project, key.clone())?.camera.$value)
        }
    };
}

vec3_accessors!(model_position, model_position_mut, model.position);
vec3_accessors!(model_anchor, model_anchor_mut, model.anchor);
vec3_accessors!(model_rotation, model_rotation_mut, model.rotation_degrees);
vec3_accessors!(model_scale, model_scale_mut, model.scale);
vec3_accessors!(camera_position, camera_position_mut, camera.position);
vec3_accessors!(
    camera_rotation,
    camera_rotation_mut,
    camera.rotation_degrees
);
scalar_accessors!(camera_fov, camera_fov_mut, vertical_fov_degrees);
scalar_accessors!(
    camera_focus_distance,
    camera_focus_distance_mut,
    focus_distance
);
scalar_accessors!(camera_f_stop, camera_f_stop_mut, f_stop);
scalar_accessors!(
    camera_orthographic_height,
    camera_orthographic_height_mut,
    orthographic_height
);
scalar_accessors!(camera_exposure, camera_exposure_mut, exposure_ev);
