use crate::InspectedItem as SelectedItem;
use crate::player_state::ProjectChange;
use crate::timeline_value::*;
use gtk::prelude::Cast;
use shrimply_project::project::{
    Project, SHAPE_APPEARANCE_PREVIEW_FACET, SHAPE_CONTENT_PREVIEW_FACET, ShapeItem, ShapeKind,
    ShapeRoundingStrategy, Time, VideoItem, VideoItemContent, generated_item_keyframe_span,
};

use crate::{
    InspectorContext,
    item::{DefaultInspectorItem, InspectorListItem},
    section::InspectorSection,
    timeline_value::color::{ColorTarget, color_control},
    timeline_value::scalar::{ScalarSpec, ScalarTarget},
    timeline_value::vector::vec2::{VecSpec, VecTarget, vec_control},
};

pub(crate) fn shape_items(
    shape: &ShapeItem,
    _context: &InspectorContext,
) -> Vec<InspectorListItem> {
    vec![
        DefaultInspectorItem::new(
            "shape",
            "Shape",
            ShapeContent {
                shape: shape.shape.clone(),
                size: shape.size.clone(),
                star_points: shape.star_points.clone(),
                star_inner_radius_percent: shape.star_inner_radius_percent.clone(),
                arrow_shaft_width_percent: shape.arrow_shaft_width_percent.clone(),
                arrow_head_length_percent: shape.arrow_head_length_percent.clone(),
                cross_arm_thickness_percent: shape.cross_arm_thickness_percent.clone(),
                ellipse_inner_radius_percent: shape.ellipse_inner_radius_percent.clone(),
                ellipse_completion_degrees: shape.ellipse_completion_degrees.clone(),
            },
            shape_content_controls,
            |context, value: ShapeContent| {
                apply_shape_reset(context, "reset-shape-content", move |shape| {
                    shape.shape = value.shape;
                    shape.size = value.size;
                    shape.star_points = value.star_points;
                    shape.star_inner_radius_percent = value.star_inner_radius_percent;
                    shape.arrow_shaft_width_percent = value.arrow_shaft_width_percent;
                    shape.arrow_head_length_percent = value.arrow_head_length_percent;
                    shape.cross_arm_thickness_percent = value.cross_arm_thickness_percent;
                    shape.ellipse_inner_radius_percent = value.ellipse_inner_radius_percent;
                    shape.ellipse_completion_degrees = value.ellipse_completion_degrees;
                });
            },
        )
        .default_with(|context| ShapeContent::from(default_shape(context)))
        .preview_facet(SHAPE_CONTENT_PREVIEW_FACET)
        .boxed(),
        DefaultInspectorItem::new(
            "shape-appearance",
            "Appearance",
            ShapeAppearance::from(shape.clone()),
            shape_appearance_controls,
            |context, value: ShapeAppearance| {
                apply_shape_reset(context, "reset-shape-appearance", move |shape| {
                    value.apply(shape)
                });
            },
        )
        .default_with(|context| ShapeAppearance::from(default_shape(context)))
        .preview_facet(SHAPE_APPEARANCE_PREVIEW_FACET)
        .boxed(),
    ]
}

struct ShapeContent {
    shape: TimelineValue<ShapeKind>,
    size: TimelineValue<glam::Vec2>,
    star_points: TimelineValue<u32>,
    star_inner_radius_percent: TimelineValue<f32>,
    arrow_shaft_width_percent: TimelineValue<f32>,
    arrow_head_length_percent: TimelineValue<f32>,
    cross_arm_thickness_percent: TimelineValue<f32>,
    ellipse_inner_radius_percent: TimelineValue<f32>,
    ellipse_completion_degrees: TimelineValue<f32>,
}

impl Default for ShapeContent {
    fn default() -> Self {
        Self::from(default_shape_for_canvas(
            shrimply_project::project::CanvasSize {
                width: 1,
                height: 1,
            },
        ))
    }
}

impl From<ShapeItem> for ShapeContent {
    fn from(shape: ShapeItem) -> Self {
        Self {
            shape: shape.shape,
            size: shape.size,
            star_points: shape.star_points,
            star_inner_radius_percent: shape.star_inner_radius_percent,
            arrow_shaft_width_percent: shape.arrow_shaft_width_percent,
            arrow_head_length_percent: shape.arrow_head_length_percent,
            cross_arm_thickness_percent: shape.cross_arm_thickness_percent,
            ellipse_inner_radius_percent: shape.ellipse_inner_radius_percent,
            ellipse_completion_degrees: shape.ellipse_completion_degrees,
        }
    }
}

#[derive(Clone)]
struct ShapeAppearance {
    rounding_strategy: TimelineValue<ShapeRoundingStrategy>,
    fill: TimelineValue<shrimply_core::Color<u8>>,
    outline_color: TimelineValue<shrimply_core::Color<u8>>,
    outline_width: TimelineValue<f32>,
    corner_radius: TimelineValue<f32>,
    shadow_color: TimelineValue<shrimply_core::Color<u8>>,
    shadow_distance: TimelineValue<f32>,
    shadow_direction_degrees: TimelineValue<f32>,
    shadow_width: TimelineValue<f32>,
    shadow_blur: TimelineValue<f32>,
}

impl Default for ShapeAppearance {
    fn default() -> Self {
        Self::from(default_shape_for_canvas(
            shrimply_project::project::CanvasSize {
                width: 1,
                height: 1,
            },
        ))
    }
}

impl From<ShapeItem> for ShapeAppearance {
    fn from(shape: ShapeItem) -> Self {
        Self {
            rounding_strategy: shape.rounding_strategy,
            fill: shape.fill,
            outline_color: shape.outline_color,
            outline_width: shape.outline_width,
            corner_radius: shape.corner_radius,
            shadow_color: shape.shadow_color,
            shadow_distance: shape.shadow_distance,
            shadow_direction_degrees: shape.shadow_direction_degrees,
            shadow_width: shape.shadow_width,
            shadow_blur: shape.shadow_blur,
        }
    }
}

impl ShapeAppearance {
    fn apply(self, shape: &mut ShapeItem) {
        shape.rounding_strategy = self.rounding_strategy;
        shape.fill = self.fill;
        shape.outline_color = self.outline_color;
        shape.outline_width = self.outline_width;
        shape.corner_radius = self.corner_radius;
        shape.shadow_color = self.shadow_color;
        shape.shadow_distance = self.shadow_distance;
        shape.shadow_direction_degrees = self.shadow_direction_degrees;
        shape.shadow_width = self.shadow_width;
        shape.shadow_blur = self.shadow_blur;
    }
}

fn shape_content_controls(value: &ShapeContent, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    section.add_wide_control(&shape_dropdown(&value.shape, context));
    section.add_wide_control(&size_control("Size", &value.size, context));
    let position = crate::player_state::snapshot(&context.player_state).position;
    let local_time = context
        .selected_item
        .clone()
        .and_then(|key| crate::video::visual_local_time(&context.project.borrow(), key, position))
        .unwrap_or(Time::ZERO);
    match value.shape.value_at(local_time) {
        ShapeKind::Star => {
            section.add_wide_control(&integer_control("Points", &value.star_points, context));
            section.add_wide_control(&scalar_control(
                "Inner radius",
                &value.star_inner_radius_percent,
                context,
                ShapeField::StarInnerRadiusPercent,
            ));
        }
        ShapeKind::Arrow => {
            section.add_wide_control(&scalar_control(
                "Shaft width",
                &value.arrow_shaft_width_percent,
                context,
                ShapeField::ArrowShaftWidthPercent,
            ));
            section.add_wide_control(&scalar_control(
                "Head length",
                &value.arrow_head_length_percent,
                context,
                ShapeField::ArrowHeadLengthPercent,
            ));
        }
        ShapeKind::Cross => section.add_wide_control(&scalar_control(
            "Arm thickness",
            &value.cross_arm_thickness_percent,
            context,
            ShapeField::CrossArmThicknessPercent,
        )),
        ShapeKind::Ellipse => {
            section.add_wide_control(&scalar_control(
                "Completion",
                &value.ellipse_completion_degrees,
                context,
                ShapeField::EllipseCompletionDegrees,
            ));
            section.add_wide_control(&scalar_control(
                "Inner radius",
                &value.ellipse_inner_radius_percent,
                context,
                ShapeField::EllipseInnerRadiusPercent,
            ));
        }
        _ => {}
    }
    vec![section.into_widget()]
}

fn shape_appearance_controls(
    value: &ShapeAppearance,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    section.add_wide_control(&color_control(
        "Fill",
        &value.fill,
        context,
        color_target(shape_fill),
    ));
    section.add_wide_control(&color_control(
        "Outline",
        &value.outline_color,
        context,
        color_target(shape_outline_color),
    ));
    section.add_wide_control(&scalar_control(
        "Outline width",
        &value.outline_width,
        context,
        ShapeField::OutlineWidth,
    ));
    section.add_wide_control(&scalar_control(
        "Rounded",
        &value.corner_radius,
        context,
        ShapeField::CornerRadius,
    ));
    section.add_wide_control(&rounding_strategy_dropdown(
        &value.rounding_strategy,
        context,
    ));
    section.add_wide_control(&color_control(
        "Shadow color",
        &value.shadow_color,
        context,
        color_target(shape_shadow_color),
    ));
    section.add_wide_control(&scalar_control(
        "Shadow distance",
        &value.shadow_distance,
        context,
        ShapeField::ShadowDistance,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow direction",
        &value.shadow_direction_degrees,
        context,
        ShapeField::ShadowDirectionDegrees,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow width",
        &value.shadow_width,
        context,
        ShapeField::ShadowWidth,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow blur",
        &value.shadow_blur,
        context,
        ShapeField::ShadowBlur,
    ));
    vec![section.into_widget()]
}

fn default_shape(context: &InspectorContext) -> ShapeItem {
    default_shape_for_canvas(context.project.borrow().canvas_size)
}

fn default_shape_for_canvas(canvas_size: shrimply_project::project::CanvasSize) -> ShapeItem {
    let VideoItemContent::Shape(shape) =
        VideoItem::shape_item(canvas_size, Time::ZERO, Time::ZERO).content
    else {
        unreachable!()
    };
    *shape
}

fn apply_shape_reset(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut ShapeItem),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let VideoItemContent::Shape(shape) = &mut item.content else {
        return;
    };
    update(shape);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    crate::player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ShapeField {
    #[expect(dead_code, reason = "the shared visual transform owns rotation")]
    RotationDegrees,
    StarInnerRadiusPercent,
    ArrowShaftWidthPercent,
    ArrowHeadLengthPercent,
    CrossArmThicknessPercent,
    EllipseInnerRadiusPercent,
    EllipseCompletionDegrees,
    OutlineWidth,
    CornerRadius,
    ShadowDistance,
    ShadowDirectionDegrees,
    ShadowWidth,
    ShadowBlur,
}

fn shape_dropdown(value: &TimelineValue<ShapeKind>, context: &InspectorContext) -> gtk::Widget {
    crate::timeline_value::step::step_control(
        "Shape",
        value,
        context,
        crate::timeline_value::step::StepTarget::new(
            |project, key| {
                let VideoItemContent::Shape(shape) = &project.video_item(&key)?.content else {
                    return None;
                };
                Some(&shape.shape)
            },
            |project, key| {
                let VideoItemContent::Shape(shape) = &mut project.video_item_mut(&key)?.content
                else {
                    return None;
                };
                Some(&mut shape.shape)
            },
            "edit-shape-kind",
            ProjectChange {
                video: true,
                inspector: true,
                ..Default::default()
            },
        ),
    )
}

fn rounding_strategy_dropdown(
    value: &TimelineValue<ShapeRoundingStrategy>,
    context: &InspectorContext,
) -> gtk::Widget {
    crate::timeline_value::step::step_control(
        "Rounding",
        value,
        context,
        crate::timeline_value::step::StepTarget::new(
            |project, key| {
                let VideoItemContent::Shape(shape) = &project.video_item(&key)?.content else {
                    return None;
                };
                Some(&shape.rounding_strategy)
            },
            |project, key| {
                let VideoItemContent::Shape(shape) = &mut project.video_item_mut(&key)?.content
                else {
                    return None;
                };
                Some(&mut shape.rounding_strategy)
            },
            "edit-shape-rounding-strategy",
            ProjectChange {
                video: true,
                inspector: true,
                ..Default::default()
            },
        ),
    )
}

fn scalar_control(
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    field: ShapeField,
) -> gtk::Widget {
    crate::timeline_value::scalar::scalar_control(
        label,
        value,
        context,
        shape_scalar_target(field),
        shape_scalar_spec(field),
    )
}

fn integer_control(
    label: &str,
    value: &TimelineValue<u32>,
    context: &InspectorContext,
) -> gtk::Widget {
    let editor = gtk::SpinButton::with_range(3.0, 32.0, 1.0);
    editor.set_value(f64::from(value.fallback()));
    let key = context.selected_item.clone();
    let editor_key = key.clone();
    let project = context.project.clone();
    let player = context.player_state.clone();
    editor.connect_value_changed(move |editor| {
        let Some(key) = editor_key.clone() else {
            return;
        };
        let next = editor.value_as_int().clamp(3, 32) as u32;
        let position = crate::player_state::snapshot(&player).position;
        let mut project_ref = project.borrow_mut();
        let Some(time) = project_ref.keyframe_time(&key, position) else {
            return;
        };
        let step = crate::keyframe_editor::project_frame_step(&project_ref);
        let Some(shape) = selected_shape_mut(&mut project_ref, key.clone()) else {
            return;
        };
        match &mut shape.star_points.base {
            TimelineBase::Const(value) => *value = next,
            TimelineBase::Keyframes(keyframes) => {
                if let Some(keyframe) = keyframes
                    .iter_mut()
                    .find(|keyframe| crate::keyframe_model::same_frame(keyframe.time, time, step))
                {
                    keyframe.time = time;
                    keyframe.value = next;
                } else {
                    keyframes.push(u32::keyframe(time, next));
                    keyframes.sort_by_key(|keyframe| keyframe.time);
                }
            }
        }
        shrimply_project::project::commit_coalesced_edit(&project_ref, "edit-shape-points");
        drop(project_ref);
        crate::player_state::refresh_project(
            &player,
            ProjectChange {
                video: true,
                ..Default::default()
            },
        );
    });
    let project_for_keyframes = context.project.clone();
    let player_for_keyframes = context.player_state.clone();
    let project_for_expression = context.project.clone();
    let player_for_expression = context.player_state.clone();
    let keyframes_key = key.clone();
    let expression_key = key;
    crate::timeline_value::layered::control(
        label,
        value,
        editor.upcast(),
        Vec::new(),
        move |enabled| {
            let Some(key) = keyframes_key.clone() else {
                return;
            };
            let position = crate::player_state::snapshot(&player_for_keyframes).position;
            let mut project = project_for_keyframes.borrow_mut();
            let Some(evaluation_time) =
                crate::video::visual_local_time(&project, key.clone(), position)
            else {
                return;
            };
            let Some(keyframe_time) = project.keyframe_time(&key, position) else {
                return;
            };
            let Some(shape) = selected_shape_mut(&mut project, key.clone()) else {
                return;
            };
            let current = shape.star_points.value_at(evaluation_time);
            shape.star_points.base = if enabled {
                TimelineBase::Keyframes(vec![u32::keyframe(keyframe_time, current)])
            } else {
                TimelineBase::Const(current)
            };
            shrimply_project::project::commit_edit(&project, "edit-shape-points-keyframes");
            drop(project);
            crate::player_state::refresh_project(
                &player_for_keyframes,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..Default::default()
                },
            );
        },
        move |enabled| {
            let Some(key) = expression_key.clone() else {
                return;
            };
            let mut project = project_for_expression.borrow_mut();
            let Some(shape) = selected_shape_mut(&mut project, key.clone()) else {
                return;
            };
            shape
                .star_points
                .expression
                .get_or_insert_with(|| TimelineExpression {
                    id: uuid::Uuid::new_v4(),
                    enabled,
                    source: "x".to_string(),
                })
                .enabled = enabled;
            shrimply_project::project::commit_edit(&project, "edit-shape-points-expression");
            drop(project);
            crate::player_state::refresh_project(
                &player_for_expression,
                ProjectChange {
                    video: true,
                    inspector: true,
                    ..Default::default()
                },
            );
        },
    )
}

fn size_control(
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    context: &InspectorContext,
) -> gtk::Widget {
    vec_control(
        label,
        value,
        context,
        vec_target(shape_size, shape_size_mut),
        VecSpec {
            first_prefix: "W",
            second_prefix: "H",
            minimum: Some(1.0),
            ..vec_spec()
        },
    )
}

fn vec_target(
    get: fn(&Project, SelectedItem) -> Option<&TimelineValue<glam::Vec2>>,
    get_mut: fn(&mut Project, SelectedItem) -> Option<&mut TimelineValue<glam::Vec2>>,
) -> VecTarget {
    VecTarget {
        access: crate::timeline_value::vector::vec2::VecAccess::Item { get, get_mut },
        scope_id: None,
        local_time: video_local_time_for_key,
        duration: video_duration_for_key,
        refresh: ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
        commit_name: "edit-shape-vector",
    }
}

fn vec_spec() -> VecSpec {
    VecSpec {
        first_prefix: "X",
        second_prefix: "Y",
        drag_step: 1.0,
        digits: 0,
        width_chars: 7,
        minimum: None,
        maximum: None,
        unit_name: "px",
    }
}

fn shape_scalar_target(field: ShapeField) -> ScalarTarget {
    ScalarTarget {
        access: crate::timeline_value::scalar::ScalarAccess::Item {
            get: match field {
                ShapeField::RotationDegrees => shape_rotation_degrees,
                ShapeField::StarInnerRadiusPercent => shape_star_inner_radius_percent,
                ShapeField::ArrowShaftWidthPercent => shape_arrow_shaft_width_percent,
                ShapeField::ArrowHeadLengthPercent => shape_arrow_head_length_percent,
                ShapeField::CrossArmThicknessPercent => shape_cross_arm_thickness_percent,
                ShapeField::EllipseInnerRadiusPercent => shape_ellipse_inner_radius_percent,
                ShapeField::EllipseCompletionDegrees => shape_ellipse_completion_degrees,
                ShapeField::OutlineWidth => shape_outline_width,
                ShapeField::CornerRadius => shape_corner_radius,
                ShapeField::ShadowDistance => shape_shadow_distance,
                ShapeField::ShadowDirectionDegrees => shape_shadow_direction_degrees,
                ShapeField::ShadowWidth => shape_shadow_width,
                ShapeField::ShadowBlur => shape_shadow_blur,
            },
            get_mut: match field {
                ShapeField::RotationDegrees => shape_rotation_degrees_mut,
                ShapeField::StarInnerRadiusPercent => shape_star_inner_radius_percent_mut,
                ShapeField::ArrowShaftWidthPercent => shape_arrow_shaft_width_percent_mut,
                ShapeField::ArrowHeadLengthPercent => shape_arrow_head_length_percent_mut,
                ShapeField::CrossArmThicknessPercent => shape_cross_arm_thickness_percent_mut,
                ShapeField::EllipseInnerRadiusPercent => shape_ellipse_inner_radius_percent_mut,
                ShapeField::EllipseCompletionDegrees => shape_ellipse_completion_degrees_mut,
                ShapeField::OutlineWidth => shape_outline_width_mut,
                ShapeField::CornerRadius => shape_corner_radius_mut,
                ShapeField::ShadowDistance => shape_shadow_distance_mut,
                ShapeField::ShadowDirectionDegrees => shape_shadow_direction_degrees_mut,
                ShapeField::ShadowWidth => shape_shadow_width_mut,
                ShapeField::ShadowBlur => shape_shadow_blur_mut,
            },
        },
        scope_id: None,
        local_time: video_local_time_for_key,
        duration: video_duration_for_key,
        refresh: ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
        commit_name: "edit-shape-scalar",
    }
}

fn shape_scalar_spec(field: ShapeField) -> ScalarSpec {
    let percent = matches!(
        field,
        ShapeField::StarInnerRadiusPercent
            | ShapeField::ArrowShaftWidthPercent
            | ShapeField::ArrowHeadLengthPercent
            | ShapeField::CrossArmThicknessPercent
            | ShapeField::EllipseInnerRadiusPercent
    );
    ScalarSpec {
        drag_step: 1.0,
        digits: 0,
        integer: false,
        width_chars: 9,
        minimum: match field {
            ShapeField::RotationDegrees | ShapeField::ShadowDirectionDegrees => None,
            ShapeField::StarInnerRadiusPercent
            | ShapeField::ArrowShaftWidthPercent
            | ShapeField::ArrowHeadLengthPercent
            | ShapeField::CrossArmThicknessPercent => Some(5.0),
            ShapeField::EllipseInnerRadiusPercent | ShapeField::EllipseCompletionDegrees => {
                Some(0.0)
            }
            _ => Some(0.0),
        },
        maximum: match field {
            ShapeField::StarInnerRadiusPercent
            | ShapeField::ArrowShaftWidthPercent
            | ShapeField::ArrowHeadLengthPercent
            | ShapeField::CrossArmThicknessPercent => Some(95.0),
            ShapeField::EllipseInnerRadiusPercent => Some(95.0),
            ShapeField::EllipseCompletionDegrees => Some(360.0),
            _ => None,
        },
        unit_name: if percent {
            Some("%")
        } else if matches!(
            field,
            ShapeField::RotationDegrees
                | ShapeField::EllipseCompletionDegrees
                | ShapeField::ShadowDirectionDegrees
        ) {
            Some("deg")
        } else {
            Some("px")
        },
        rotating_icon: match field {
            ShapeField::RotationDegrees => Some(("arrow3-up-symbolic", 0.0)),
            ShapeField::ShadowDirectionDegrees => Some(("arrow3-up-symbolic", 90.0)),
            _ => None,
        },
        display: |value| value as f64,
        store: |value| value as f32,
        clamp: match field {
            ShapeField::StarInnerRadiusPercent
            | ShapeField::ArrowShaftWidthPercent
            | ShapeField::ArrowHeadLengthPercent
            | ShapeField::CrossArmThicknessPercent => |value| value.clamp(5.0, 95.0),
            ShapeField::EllipseInnerRadiusPercent => |value| value.clamp(0.0, 95.0),
            ShapeField::EllipseCompletionDegrees => |value| value.clamp(0.0, 360.0),
            ShapeField::OutlineWidth
            | ShapeField::CornerRadius
            | ShapeField::ShadowDistance
            | ShapeField::ShadowWidth
            | ShapeField::ShadowBlur => |value| value.max(0.0),
            ShapeField::RotationDegrees | ShapeField::ShadowDirectionDegrees => |value| value,
        },
    }
}

fn color_target(
    get_mut: fn(&mut Project, SelectedItem) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>>,
) -> ColorTarget {
    ColorTarget {
        access: crate::timeline_value::color::ColorAccess::Item(get_mut),
        scope_id: None,
        local_time: video_local_time_for_key,
        duration: video_duration_for_key,
        refresh: ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
        commit_name: "edit-shape-color",
    }
}

fn selected_shape(project: &Project, key: SelectedItem) -> Option<&ShapeItem> {
    let item = project.video_item(&key)?;
    let VideoItemContent::Shape(shape) = &item.content else {
        return None;
    };
    Some(shape)
}

fn selected_shape_mut(project: &mut Project, key: SelectedItem) -> Option<&mut ShapeItem> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Shape(shape) = &mut item.content else {
        return None;
    };
    Some(shape)
}

fn shape_size_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.size)
}

fn shape_size(project: &Project, key: SelectedItem) -> Option<&TimelineValue<glam::Vec2>> {
    selected_shape(project, key.clone()).map(|shape| &shape.size)
}

fn shape_rotation_degrees(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_transform(project, key.clone()).map(|transform| &transform.rotation_degrees)
}

fn shape_rotation_degrees_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_transform_mut(project, key.clone()).map(|transform| &mut transform.rotation_degrees)
}

fn selected_transform(
    project: &Project,
    key: SelectedItem,
) -> Option<&shrimply_project::project::Transform> {
    project.video_item(&key).map(|item| &item.transform)
}

fn selected_transform_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut shrimply_project::project::Transform> {
    project.video_item_mut(&key).map(|item| &mut item.transform)
}

fn shape_outline_width(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.outline_width)
}

fn shape_star_inner_radius_percent(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.star_inner_radius_percent)
}

fn shape_star_inner_radius_percent_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.star_inner_radius_percent)
}

fn shape_arrow_shaft_width_percent(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.arrow_shaft_width_percent)
}

fn shape_arrow_shaft_width_percent_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.arrow_shaft_width_percent)
}

fn shape_arrow_head_length_percent(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.arrow_head_length_percent)
}

fn shape_arrow_head_length_percent_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.arrow_head_length_percent)
}

fn shape_cross_arm_thickness_percent(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.cross_arm_thickness_percent)
}

fn shape_cross_arm_thickness_percent_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.cross_arm_thickness_percent)
}

fn shape_ellipse_inner_radius_percent(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.ellipse_inner_radius_percent)
}

fn shape_ellipse_inner_radius_percent_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.ellipse_inner_radius_percent)
}

fn shape_ellipse_completion_degrees(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.ellipse_completion_degrees)
}

fn shape_ellipse_completion_degrees_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.ellipse_completion_degrees)
}

fn shape_outline_width_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.outline_width)
}

fn shape_corner_radius(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.corner_radius)
}

fn shape_corner_radius_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.corner_radius)
}

fn shape_shadow_distance(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.shadow_distance)
}

fn shape_shadow_distance_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.shadow_distance)
}

fn shape_shadow_direction_degrees(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.shadow_direction_degrees)
}

fn shape_shadow_direction_degrees_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.shadow_direction_degrees)
}

fn shape_shadow_width(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.shadow_width)
}

fn shape_shadow_width_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.shadow_width)
}

fn shape_shadow_blur(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_shape(project, key.clone()).map(|shape| &shape.shadow_blur)
}

fn shape_shadow_blur_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.shadow_blur)
}

fn shape_fill(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.fill)
}

fn shape_outline_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.outline_color)
}

fn shape_shadow_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_shape_mut(project, key.clone()).map(|shape| &mut shape.shadow_color)
}

fn video_local_time_for_key(project: &Project, key: SelectedItem, position: Time) -> Option<Time> {
    crate::video::visual_local_time(project, key, position)
}

fn video_duration_for_key(project: &Project, key: SelectedItem) -> Option<Time> {
    let item = project.video_item(&key)?;
    generated_item_keyframe_span(item)
        .map(|(_, end)| end)
        .or_else(|| crate::video::visual_duration(project, key))
}
