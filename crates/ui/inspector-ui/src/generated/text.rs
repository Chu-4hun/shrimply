use gtk::prelude::*;

use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange};
use crate::timeline_value::*;
use shrimply_project::project::{
    FontFamily, FontVariation, Project, TEXT_APPEARANCE_PREVIEW_FACET, TextDirection,
    TextFontStyle, TextHorizontalAlign, TextItem, Time, Transform, VerticalAlign, VideoItem,
    VideoItemContent, generated_item_keyframe_span,
};

use super::common::{resize_text_source, update_generated_live};
use crate::{
    InspectorContext,
    font_selector::font_selector_list,
    item::{DefaultInspectorItem, InspectorListItem},
    section::InspectorSection,
    timeline_value::color::{ColorTarget, color_control},
    timeline_value::scalar::{ScalarSpec, ScalarTarget},
    timeline_value::vector::vec2::{VecSpec, VecTarget, vec_control},
};

pub(crate) fn text_items(text: &TextItem, _context: &InspectorContext) -> Vec<InspectorListItem> {
    vec![
        DefaultInspectorItem::new(
            "text",
            "Text",
            TextContent {
                font_families: text.font_families.clone(),
                h_align: text.h_align.clone(),
                v_align: text.v_align.clone(),
                direction: text.direction.clone(),
                text: text.text.clone(),
            },
            text_content_controls,
            |context, value: TextContent| {
                apply_text_reset(context, "reset-text-content", move |text| {
                    text.font_families = value.font_families;
                    text.h_align = value.h_align;
                    text.v_align = value.v_align;
                    text.direction = value.direction;
                });
            },
        )
        .default_with(|context| TextContent::from(default_text(context)))
        .boxed(),
        DefaultInspectorItem::new(
            "text-appearance",
            "Appearance",
            TextAppearance::from(text.clone()),
            text_appearance_controls,
            |context, value: TextAppearance| {
                apply_text_reset(context, "reset-text-appearance", move |text| {
                    value.apply(text)
                });
            },
        )
        .default_with(|context| TextAppearance::from(default_text(context)))
        .preview_facet(TEXT_APPEARANCE_PREVIEW_FACET)
        .boxed(),
    ]
}

#[derive(Clone)]
struct TextContent {
    text: TimelineValue<String>,
    font_families: Vec<FontFamily>,
    h_align: TimelineValue<TextHorizontalAlign>,
    v_align: TimelineValue<VerticalAlign>,
    direction: TimelineValue<TextDirection>,
}

impl Default for TextContent {
    fn default() -> Self {
        Self::from(default_text_for_canvas(
            shrimply_project::project::CanvasSize {
                width: 1,
                height: 1,
            },
        ))
    }
}

impl From<TextItem> for TextContent {
    fn from(text: TextItem) -> Self {
        Self {
            text: text.text,
            font_families: text.font_families,
            h_align: text.h_align,
            v_align: text.v_align,
            direction: text.direction,
        }
    }
}

#[derive(Clone)]
struct TextAppearance {
    font_size: TimelineValue<f32>,
    font_weight: TimelineValue<f32>,
    tracking: TimelineValue<f32>,
    line_height: TimelineValue<f32>,
    font_style: TimelineValue<TextFontStyle>,
    font_variations: Vec<FontVariation>,
    color: TimelineValue<shrimply_core::Color<u8>>,
    background_color: TimelineValue<shrimply_core::Color<u8>>,
    background_roundness: TimelineValue<f32>,
    background_padding: TimelineValue<glam::Vec2>,
    outline_color: TimelineValue<shrimply_core::Color<u8>>,
    outline_width: TimelineValue<f32>,
    shadow_color: TimelineValue<shrimply_core::Color<u8>>,
    shadow_distance: TimelineValue<f32>,
    shadow_direction_degrees: TimelineValue<f32>,
    shadow_width: TimelineValue<f32>,
    shadow_blur: TimelineValue<f32>,
}

impl Default for TextAppearance {
    fn default() -> Self {
        Self::from(default_text_for_canvas(
            shrimply_project::project::CanvasSize {
                width: 1,
                height: 1,
            },
        ))
    }
}

impl From<TextItem> for TextAppearance {
    fn from(text: TextItem) -> Self {
        Self {
            font_size: text.font_size,
            font_weight: text.font_weight,
            tracking: text.tracking,
            line_height: text.line_height,
            font_style: text.font_style,
            font_variations: text.font_variations,
            color: text.color,
            background_color: text.background_color,
            background_roundness: text.background_roundness,
            background_padding: text.background_padding,
            outline_color: text.outline_color,
            outline_width: text.outline_width,
            shadow_color: text.shadow_color,
            shadow_distance: text.shadow_distance,
            shadow_direction_degrees: text.shadow_direction_degrees,
            shadow_width: text.shadow_width,
            shadow_blur: text.shadow_blur,
        }
    }
}

impl TextAppearance {
    fn apply(self, text: &mut TextItem) {
        text.font_size = self.font_size;
        text.font_weight = self.font_weight;
        text.tracking = self.tracking;
        text.line_height = self.line_height;
        text.font_style = self.font_style;
        text.font_variations = self.font_variations;
        text.color = self.color;
        text.background_color = self.background_color;
        text.background_roundness = self.background_roundness;
        text.background_padding = self.background_padding;
        text.outline_color = self.outline_color;
        text.outline_width = self.outline_width;
        text.shadow_color = self.shadow_color;
        text.shadow_distance = self.shadow_distance;
        text.shadow_direction_degrees = self.shadow_direction_degrees;
        text.shadow_width = self.shadow_width;
        text.shadow_blur = self.shadow_blur;
    }
}

fn text_content_controls(value: &TextContent, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    section.add_wide_control(&crate::timeline_value::text::text_control(
        "Text",
        &value.text,
        context,
    ));
    let fonts = crate::ui::control_row(
        "Fonts",
        &font_families_control(&value.font_families, context),
    );
    fonts
        .first_child()
        .expect("font control row has a label")
        .set_valign(gtk::Align::Start);
    section.add_wide_control(&fonts);
    section.add_wide_control(&text_step_button(
        "Align",
        &value.h_align,
        context,
        |text| &text.h_align,
        |text| &mut text.h_align,
        "edit-text-horizontal-align",
    ));
    section.add_wide_control(&text_step_button(
        "Vertical",
        &value.v_align,
        context,
        |text| &text.v_align,
        |text| &mut text.v_align,
        "edit-text-vertical-align",
    ));
    section.add_wide_control(&text_step_button(
        "Direction",
        &value.direction,
        context,
        |text| &text.direction,
        |text| &mut text.direction,
        "edit-text-direction",
    ));
    vec![section.into_widget()]
}

fn text_appearance_controls(
    value: &TextAppearance,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    section.add_wide_control(&scalar_control(
        "Font size",
        &value.font_size,
        context,
        TextField::FontSize,
    ));
    section.add_wide_control(&scalar_control(
        "Font weight",
        &value.font_weight,
        context,
        TextField::FontWeight,
    ));
    section.add_wide_control(&scalar_control(
        "Tracking",
        &value.tracking,
        context,
        TextField::Tracking,
    ));
    section.add_wide_control(&scalar_control(
        "Line height",
        &value.line_height,
        context,
        TextField::LineHeight,
    ));
    section.add_wide_control(&text_step_button(
        "Style",
        &value.font_style,
        context,
        |text| &text.font_style,
        |text| &mut text.font_style,
        "edit-text-font-style",
    ));
    for control in font_variation_controls(&value.font_variations, context) {
        section.add_wide_control(&control);
    }
    section.add_wide_control(&color_control(
        "Color",
        &value.color,
        context,
        color_target(text_color),
    ));
    section.add_wide_control(&color_control(
        "Background fill",
        &value.background_color,
        context,
        color_target(text_background_color),
    ));
    section.add_wide_control(&scalar_control(
        "Background roundness",
        &value.background_roundness,
        context,
        TextField::BackgroundRoundness,
    ));
    section.add_wide_control(&background_padding_control(
        &value.background_padding,
        context,
    ));
    section.add_wide_control(&color_control(
        "Outline",
        &value.outline_color,
        context,
        color_target(text_outline_color),
    ));
    section.add_wide_control(&scalar_control(
        "Outline width",
        &value.outline_width,
        context,
        TextField::OutlineWidth,
    ));
    section.add_wide_control(&color_control(
        "Shadow color",
        &value.shadow_color,
        context,
        color_target(text_shadow_color),
    ));
    section.add_wide_control(&scalar_control(
        "Shadow distance",
        &value.shadow_distance,
        context,
        TextField::ShadowDistance,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow direction",
        &value.shadow_direction_degrees,
        context,
        TextField::ShadowDirectionDegrees,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow width",
        &value.shadow_width,
        context,
        TextField::ShadowWidth,
    ));
    section.add_wide_control(&scalar_control(
        "Shadow blur",
        &value.shadow_blur,
        context,
        TextField::ShadowBlur,
    ));
    vec![section.into_widget()]
}

fn default_text(context: &InspectorContext) -> TextItem {
    let mut text = default_text_for_canvas(context.project.borrow().canvas_size);
    text.font_families =
        vec![shrimply_state::preferences::snapshot(&context.preferences).default_text_font_family];
    text
}

fn default_text_for_canvas(canvas_size: shrimply_project::project::CanvasSize) -> TextItem {
    let VideoItemContent::Text(text) =
        VideoItem::text_item(canvas_size, Time::ZERO, Time::ZERO).content
    else {
        unreachable!()
    };
    *text
}

fn apply_text_reset(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut TextItem),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let VideoItemContent::Text(text) = &mut item.content else {
        return;
    };
    update(text);
    resize_text_source(item);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
}

const PERCENT: f64 = 100.0;
const MIN_LINE_HEIGHT: f32 = 0.01;

#[derive(Clone, Copy)]
enum TextField {
    #[expect(dead_code, reason = "the shared visual transform owns rotation")]
    RotationDegrees,
    FontSize,
    FontWeight,
    Tracking,
    LineHeight,
    BackgroundRoundness,
    OutlineWidth,
    ShadowDistance,
    ShadowDirectionDegrees,
    ShadowWidth,
    ShadowBlur,
}

#[expect(dead_code, reason = "the shared visual transform owns vector controls")]
#[derive(Clone, Copy)]
enum TextVectorField {
    Position,
    Anchor,
    Scale,
}

fn font_families_control(value: &[FontFamily], context: &InspectorContext) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return font_selector_list(value, |_| {});
    };
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let refresh = context.refresh.clone();
    font_selector_list(value, move |next| {
        let mut project_state = project.borrow_mut();
        let Some(item) = project_state.video_item_mut(&key) else {
            return;
        };
        let VideoItemContent::Text(text) = &mut item.content else {
            return;
        };
        if text.font_families == next {
            return;
        }
        text.font_families = next;
        resize_text_source(item);
        shrimply_project::project::commit_edit(&project_state, "edit-text-font");
        drop(project_state);
        player_state::refresh_project(
            &player_state,
            ProjectChange {
                video: true,
                inspector: true,
                ..ProjectChange::default()
            },
        );
        refresh();
    })
}

fn font_variation_controls(
    values: &[FontVariation],
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let Some(key) = context.selected_item.clone() else {
        return Vec::new();
    };
    let Some(capabilities) = selected_font_capabilities(context, &key) else {
        return Vec::new();
    };
    capabilities
        .axes
        .into_iter()
        .filter(|axis| !matches!(axis.tag.as_str(), "wght" | "ital"))
        .map(|axis| {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::builder()
                .label(&axis.tag)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .tooltip_text(shrimply_ui_foundation::i18n::text_args(
                    "%{axis} variation axis",
                    &[("axis", axis.tag.clone())],
                ))
                .build();
            let step = ((axis.maximum - axis.minimum).abs() / 100.0).max(0.01);
            let spin =
                gtk::SpinButton::with_range(axis.minimum.into(), axis.maximum.into(), step.into());
            spin.set_digits(2);
            spin.set_width_chars(8);
            spin.set_value(
                values
                    .iter()
                    .find(|value| value.axis == axis.tag)
                    .map_or(axis.default, |value| value.value)
                    .into(),
            );
            spin.connect_value_changed({
                let project = context.project.clone();
                let player_state = context.player_state.clone();
                let key = key.clone();
                move |spin| {
                    let value = spin.value() as f32;
                    update_generated_live(&project, &player_state, key.clone(), |item| {
                        let VideoItemContent::Text(text) = &mut item.content else {
                            return false;
                        };
                        if let Some(variation) = text
                            .font_variations
                            .iter_mut()
                            .find(|variation| variation.axis == axis.tag)
                        {
                            if variation.value.to_bits() == value.to_bits() {
                                return false;
                            }
                            variation.value = value;
                        } else {
                            text.font_variations.push(FontVariation {
                                axis: axis.tag.clone(),
                                value,
                            });
                        }
                        true
                    });
                    shrimply_project::project::commit_edit(
                        &project.borrow(),
                        "edit-text-font-variation",
                    );
                }
            });
            row.append(&label);
            row.append(&spin);
            row.upcast()
        })
        .collect()
}

fn selected_font_capabilities(
    context: &InspectorContext,
    key: &SelectedItem,
) -> Option<crate::font_cache::FontCapabilities> {
    let family = {
        let project = context.project.borrow();
        let text = selected_text(&project, key.clone())?;
        text.font_families.first()?.clone()
    };
    Some(match family {
        FontFamily::GoogleFonts { name } => {
            crate::font_cache::cached_capabilities(&name).unwrap_or_default()
        }
        FontFamily::Local { name } => crate::font_cache::local_capabilities(&name),
    })
}

fn text_step_button<T: TimelineStep>(
    label: &str,
    value: &TimelineValue<T>,
    context: &InspectorContext,
    get: fn(&TextItem) -> &TimelineValue<T>,
    get_mut: fn(&mut TextItem) -> &mut TimelineValue<T>,
    commit_name: &'static str,
) -> gtk::Widget {
    crate::timeline_value::step::step_button_control(
        label,
        value,
        context,
        crate::timeline_value::step::StepTarget::new(
            move |project, key| selected_text(project, key).map(get),
            move |project, key| selected_text_mut(project, key).map(get_mut),
            commit_name,
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
    field: TextField,
) -> gtk::Widget {
    crate::timeline_value::scalar::scalar_control(
        label,
        value,
        context,
        text_scalar_target(field),
        text_scalar_spec(field),
    )
}

fn background_padding_control(
    value: &TimelineValue<glam::Vec2>,
    context: &InspectorContext,
) -> gtk::Widget {
    vec_control(
        "Background padding",
        value,
        context,
        VecTarget {
            access: crate::timeline_value::vector::vec2::VecAccess::Item {
                get: text_background_padding,
                get_mut: text_background_padding_mut,
            },
            scope_id: None,
            local_time: video_local_time_for_key,
            global_time: video_global_time_for_key,
            duration: video_duration_for_key,
            refresh: ProjectChange {
                video: true,
                inspector: true,
                ..ProjectChange::default()
            },
            commit_name: "edit-text-background-padding",
        },
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: 1.0,
            digits: 0,
            width_chars: 7,
            minimum: Some(0.0),
            maximum: None,
            unit_name: "px",
        },
    )
}

#[expect(dead_code, reason = "the shared visual transform owns vector controls")]
fn transform_vector_control(
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    field: TextVectorField,
    context: &InspectorContext,
) -> gtk::Widget {
    let scale = matches!(field, TextVectorField::Scale);
    vec_control(
        label,
        value,
        context,
        VecTarget {
            access: crate::timeline_value::vector::vec2::VecAccess::Item {
                get: match field {
                    TextVectorField::Position => text_position,
                    TextVectorField::Anchor => text_anchor,
                    TextVectorField::Scale => text_scale,
                },
                get_mut: match field {
                    TextVectorField::Position => text_position_mut,
                    TextVectorField::Anchor => text_anchor_mut,
                    TextVectorField::Scale => text_scale_mut,
                },
            },
            scope_id: None,
            local_time: video_local_time_for_key,
            global_time: video_global_time_for_key,
            duration: video_duration_for_key,
            refresh: ProjectChange {
                video: true,
                inspector: true,
                ..ProjectChange::default()
            },
            commit_name: "edit-text-transform",
        },
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: if scale { 0.01 } else { 1.0 },
            digits: if scale { 2 } else { 0 },
            width_chars: 7,
            minimum: None,
            maximum: None,
            unit_name: if scale { "x" } else { "px" },
        },
    )
}

fn text_scalar_target(field: TextField) -> ScalarTarget {
    ScalarTarget {
        access: crate::timeline_value::scalar::ScalarAccess::Item {
            get: match field {
                TextField::RotationDegrees => text_rotation_degrees,
                TextField::FontSize => text_font_size,
                TextField::FontWeight => text_font_weight,
                TextField::Tracking => text_tracking,
                TextField::LineHeight => text_line_height,
                TextField::BackgroundRoundness => text_background_roundness,
                TextField::OutlineWidth => text_outline_width,
                TextField::ShadowDistance => text_shadow_distance,
                TextField::ShadowDirectionDegrees => text_shadow_direction_degrees,
                TextField::ShadowWidth => text_shadow_width,
                TextField::ShadowBlur => text_shadow_blur,
            },
            get_mut: match field {
                TextField::RotationDegrees => text_rotation_degrees_mut,
                TextField::FontSize => text_font_size_mut,
                TextField::FontWeight => text_font_weight_mut,
                TextField::Tracking => text_tracking_mut,
                TextField::LineHeight => text_line_height_mut,
                TextField::BackgroundRoundness => text_background_roundness_mut,
                TextField::OutlineWidth => text_outline_width_mut,
                TextField::ShadowDistance => text_shadow_distance_mut,
                TextField::ShadowDirectionDegrees => text_shadow_direction_degrees_mut,
                TextField::ShadowWidth => text_shadow_width_mut,
                TextField::ShadowBlur => text_shadow_blur_mut,
            },
        },
        scope_id: None,
        local_time: video_local_time_for_key,
        global_time: video_global_time_for_key,
        duration: video_duration_for_key,
        refresh: ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
        commit_name: "edit-text-scalar",
    }
}

fn text_scalar_spec(field: TextField) -> ScalarSpec {
    ScalarSpec {
        drag_step: match field {
            TextField::FontWeight => 10.0,
            TextField::Tracking => 0.1,
            _ => 1.0,
        },
        digits: usize::from(matches!(field, TextField::Tracking)),
        integer: false,
        width_chars: 9,
        minimum: match field {
            TextField::RotationDegrees
            | TextField::ShadowDirectionDegrees
            | TextField::Tracking => None,
            TextField::FontSize | TextField::FontWeight | TextField::LineHeight => Some(1.0),
            TextField::OutlineWidth
            | TextField::BackgroundRoundness
            | TextField::ShadowDistance
            | TextField::ShadowWidth
            | TextField::ShadowBlur => Some(0.0),
        },
        maximum: matches!(field, TextField::FontWeight).then_some(1000.0),
        unit_name: match field {
            TextField::FontWeight => None,
            TextField::LineHeight => Some("%"),
            TextField::RotationDegrees | TextField::ShadowDirectionDegrees => Some("deg"),
            _ => Some("px"),
        },
        rotating_icon: match field {
            TextField::RotationDegrees => Some(("arrow3-up-symbolic", 0.0)),
            TextField::ShadowDirectionDegrees => Some(("arrow3-up-symbolic", 90.0)),
            _ => None,
        },
        display: match field {
            TextField::LineHeight => |value| value as f64 * PERCENT,
            _ => |value| value as f64,
        },
        store: match field {
            TextField::LineHeight => |value| (value / PERCENT) as f32,
            _ => |value| value as f32,
        },
        clamp: match field {
            TextField::FontSize => |value| value.max(1.0),
            TextField::FontWeight => |value| value.round().clamp(1.0, 1000.0),
            TextField::LineHeight => |value| value.max(MIN_LINE_HEIGHT),
            TextField::OutlineWidth
            | TextField::BackgroundRoundness
            | TextField::ShadowDistance
            | TextField::ShadowWidth
            | TextField::ShadowBlur => |value| value.max(0.0),
            TextField::RotationDegrees
            | TextField::ShadowDirectionDegrees
            | TextField::Tracking => |value| value,
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
        global_time: video_global_time_for_key,
        duration: video_duration_for_key,
        refresh: ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
        commit_name: "edit-text-color",
    }
}

fn selected_text(project: &Project, key: SelectedItem) -> Option<&TextItem> {
    let item = project.video_item(&key)?;
    let VideoItemContent::Text(text) = &item.content else {
        return None;
    };
    Some(text)
}

fn selected_text_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TextItem> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Text(text) = &mut item.content else {
        return None;
    };
    Some(text)
}

fn text_font_size(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.font_size)
}

fn text_font_size_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.font_size)
}

fn text_font_weight(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.font_weight)
}

fn text_font_weight_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.font_weight)
}

fn text_tracking(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.tracking)
}

fn text_tracking_mut(project: &mut Project, key: SelectedItem) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.tracking)
}

fn text_line_height(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.line_height)
}

fn text_line_height_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.line_height)
}

fn text_rotation_degrees(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_transform(project, key.clone()).map(|transform| &transform.rotation_degrees)
}

fn text_rotation_degrees_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_transform_mut(project, key.clone()).map(|transform| &mut transform.rotation_degrees)
}

fn text_position(project: &Project, key: SelectedItem) -> Option<&TimelineValue<glam::Vec2>> {
    selected_transform(project, key.clone()).map(|transform| &transform.position)
}

fn text_position_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    selected_transform_mut(project, key.clone()).map(|transform| &mut transform.position)
}

fn text_anchor(project: &Project, key: SelectedItem) -> Option<&TimelineValue<glam::Vec2>> {
    selected_transform(project, key.clone()).map(|transform| &transform.anchor)
}

fn text_anchor_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    selected_transform_mut(project, key.clone()).map(|transform| &mut transform.anchor)
}

fn text_scale(project: &Project, key: SelectedItem) -> Option<&TimelineValue<glam::Vec2>> {
    selected_transform(project, key.clone()).map(|transform| &transform.scale)
}

fn text_scale_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    selected_transform_mut(project, key.clone()).map(|transform| &mut transform.scale)
}

fn selected_transform(project: &Project, key: SelectedItem) -> Option<&Transform> {
    project.video_item(&key).map(|item| &item.transform)
}

fn selected_transform_mut(project: &mut Project, key: SelectedItem) -> Option<&mut Transform> {
    project.video_item_mut(&key).map(|item| &mut item.transform)
}

fn text_outline_width(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.outline_width)
}

fn text_background_roundness(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.background_roundness)
}

fn text_background_roundness_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.background_roundness)
}

fn text_background_padding(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<glam::Vec2>> {
    selected_text(project, key.clone()).map(|text| &text.background_padding)
}

fn text_background_padding_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.background_padding)
}

fn text_outline_width_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.outline_width)
}

fn text_shadow_distance(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.shadow_distance)
}

fn text_shadow_distance_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.shadow_distance)
}

fn text_shadow_direction_degrees(
    project: &Project,
    key: SelectedItem,
) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.shadow_direction_degrees)
}

fn text_shadow_direction_degrees_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.shadow_direction_degrees)
}

fn text_shadow_width(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.shadow_width)
}

fn text_shadow_width_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.shadow_width)
}

fn text_shadow_blur(project: &Project, key: SelectedItem) -> Option<&TimelineValue<f32>> {
    selected_text(project, key.clone()).map(|text| &text.shadow_blur)
}

fn text_shadow_blur_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<f32>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.shadow_blur)
}

fn text_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.color)
}

fn text_outline_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.outline_color)
}

fn text_background_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.background_color)
}

fn text_shadow_color(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    selected_text_mut(project, key.clone()).map(|text| &mut text.shadow_color)
}

fn video_local_time_for_key(project: &Project, key: SelectedItem, position: Time) -> Option<Time> {
    crate::video::visual_local_time(project, key, position)
}

fn video_global_time_for_key(project: &Project, key: SelectedItem, time: Time) -> Option<Time> {
    crate::video::visual_global_time(project, key, time)
}

fn video_duration_for_key(project: &Project, key: SelectedItem) -> Option<Time> {
    let item = project.video_item(&key)?;
    generated_item_keyframe_span(item)
        .map(|(_, end)| end)
        .or_else(|| crate::video::visual_duration(project, key))
}
