use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nFileFilterExt;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use shrimply_core::{
    Color,
    timeline_value::{
        TimelineBase, TimelineBool, TimelineExpression, TimelineValue, TimelineValueType,
    },
};
use shrimply_project::project::{
    PaintDrawing, PaintItem, PaintPaletteEntry, PaintStrokeOptions, PaintTaper,
    PaintTextureOptions, PaintTransform, Project, ResolvedTransform, Time, VideoItemContent,
};
use uuid::Uuid;

use crate::{
    InspectedItem as SelectedItem, InspectorContext,
    item::{DefaultInspectorItem, InspectorListItem},
    player_state::{self, ProjectChange},
    timeline_value::{
        boolean::{BoolTarget, bool_control},
        color::{ColorAccess, ColorTarget, color_control},
        scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control},
        step::{StepTarget, step_control},
    },
    transform::{self, TransformTarget},
    ui::control_row,
};

const MIN_TEXTURE_SCALE: f64 = 0.01;

pub(super) fn items(paint: &PaintItem) -> Vec<InspectorListItem> {
    vec![
        DefaultInspectorItem::new(
            "paint-palette",
            "Textures",
            PaintPalette(paint.palette.clone()),
            palette_controls,
            |context, palette: PaintPalette| {
                update_discrete(context, "reset-paint-palette", move |paint| {
                    paint.palette = palette.0;
                    let last = paint.palette.len() - 1;
                    visit_drawings_mut(&mut paint.drawing, |drawing| {
                        for stroke in &mut drawing.strokes {
                            stroke.color_index = stroke.color_index.min(last);
                        }
                        for fill in &mut drawing.fills {
                            fill.color_index = fill.color_index.min(last);
                        }
                    });
                    true
                });
            },
        )
        .boxed(),
        DefaultInspectorItem::new(
            "paint-strokes",
            "Strokes",
            paint.stroke.clone(),
            {
                let drawing = paint.drawing.clone();
                move |stroke, context| stroke_controls(stroke, &drawing, context)
            },
            |context, stroke: PaintStrokeOptions| {
                update_discrete(context, "reset-paint-strokes", move |paint| {
                    paint.stroke = stroke;
                    true
                });
            },
        )
        .boxed(),
        DefaultInspectorItem::new(
            "paint-stroke-transform",
            "Stroke Transform",
            StrokeTransform(paint.stroke_transform.clone()),
            |value, context| transform::controls(&value.0, context, TransformTarget::PaintStroke),
            |context, value: StrokeTransform| {
                update_discrete(context, "reset-paint-stroke-transform", move |paint| {
                    paint.stroke_transform = value.0;
                    true
                });
            },
        )
        .default_with(|context| {
            StrokeTransform(PaintTransform::fill(context.project.borrow().canvas_size))
        })
        .boxed(),
    ]
}

#[derive(Clone)]
struct PaintPalette(Vec<PaintPaletteEntry>);

impl Default for PaintPalette {
    fn default() -> Self {
        Self(PaintItem::default().palette)
    }
}

fn palette_controls(value: &PaintPalette, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = crate::section::InspectorSection::controls();
    for (display_index, entry) in value.0.iter().enumerate() {
        let color = &entry.color;
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let color_control = color_control(
            &shrimply_ui_foundation::i18n::text_args(
                "Color %{number}",
                &[("number", (display_index + 1).to_string())],
            ),
            color,
            context,
            ColorTarget {
                access: ColorAccess::PaintPalette { value_id: color.id },
                scope_id: Some(color.id),
                local_time: crate::video::visual_local_time,
                global_time: crate::video::visual_global_time,
                duration: crate::video::visual_duration,
                refresh: paint_refresh(),
                commit_name: "paint-palette-color",
            },
        );
        color_control.set_hexpand(true);
        row.append(&color_control);

        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(tr!("Remove color").as_ref())
            .css_classes(["flat"])
            .sensitive(value.0.len() > 1)
            .build();
        let remove_context = context.clone();
        let color_id = color.id;
        remove.connect_clicked(move |_| {
            update_discrete(
                &remove_context,
                "remove-paint-palette-color",
                move |paint| {
                    let Some(index) = paint
                        .palette
                        .iter()
                        .position(|entry| entry.color.id == color_id)
                        .filter(|_| paint.palette.len() > 1)
                    else {
                        return false;
                    };
                    paint.palette.remove(index);
                    let replacement = index.min(paint.palette.len() - 1);
                    visit_drawings_mut(&mut paint.drawing, |drawing| {
                        for stroke in &mut drawing.strokes {
                            stroke.color_index = match stroke.color_index.cmp(&index) {
                                std::cmp::Ordering::Less => stroke.color_index,
                                std::cmp::Ordering::Equal => replacement,
                                std::cmp::Ordering::Greater => stroke.color_index - 1,
                            };
                        }
                        for fill in &mut drawing.fills {
                            fill.color_index = match fill.color_index.cmp(&index) {
                                std::cmp::Ordering::Less => fill.color_index,
                                std::cmp::Ordering::Equal => replacement,
                                std::cmp::Ordering::Greater => fill.color_index - 1,
                            };
                        }
                    });
                    true
                },
            );
        });
        row.append(&remove);
        section.add_wide_control(&row);
        add_texture_controls(&section, entry, context);
    }

    let add = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .label(tr!("Add").as_ref())
        .halign(gtk::Align::End)
        .css_classes(["flat"])
        .build();
    let context = context.clone();
    add.connect_clicked(move |_| {
        update_discrete(&context, "add-paint-palette-color", |paint| {
            paint.palette.push(PaintPaletteEntry {
                color: TimelineValue::new_const(Color::<u8>::WHITE),
                texture: None,
            });
            true
        });
    });
    section.add_wide_control(&add);
    vec![section.into_widget()]
}

fn stroke_controls(
    value: &PaintStrokeOptions,
    drawing: &TimelineValue<PaintDrawing>,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let section = crate::section::InspectorSection::controls();
    section.add_wide_control(&drawing_control(drawing, context));
    add_scalar(
        &section,
        "Width",
        &value.width,
        context,
        ScalarKind::Pixels,
        "paint-stroke-width",
        stroke_width,
        stroke_width_mut,
    );
    add_scalar(
        &section,
        "Thinning",
        &value.thinning,
        context,
        ScalarKind::Factor,
        "paint-stroke-thinning",
        stroke_thinning,
        stroke_thinning_mut,
    );
    add_scalar(
        &section,
        "Smoothing",
        &value.smoothing,
        context,
        ScalarKind::Factor,
        "paint-stroke-smoothing",
        stroke_smoothing,
        stroke_smoothing_mut,
    );
    add_scalar(
        &section,
        "Streamline",
        &value.streamline,
        context,
        ScalarKind::Factor,
        "paint-stroke-streamline",
        stroke_streamline,
        stroke_streamline_mut,
    );
    add_scalar(
        &section,
        "Simplify tolerance",
        &value.simplification_tolerance,
        context,
        ScalarKind::Pixels,
        "paint-stroke-simplification",
        stroke_simplification,
        stroke_simplification_mut,
    );
    add_scalar(
        &section,
        "Subdivision spacing",
        &value.maximum_subdivision_spacing,
        context,
        ScalarKind::Pixels,
        "paint-stroke-subdivision",
        stroke_subdivision,
        stroke_subdivision_mut,
    );
    section.add_wide_control(&bool_control(
        "Start cap",
        &value.start.cap,
        value.start.cap.fallback().get(),
        context,
        BoolTarget::ItemValue {
            get: stroke_start_cap,
            get_mut: stroke_start_cap_mut,
            scope: "paint-stroke-start-cap",
            mutated: bump_revision_for_key,
        },
    ));
    section.add_wide_control(&taper_control(
        "Start taper",
        &value.start.taper,
        true,
        context,
    ));
    if value.start.taper.value_at(local_time(context)) == PaintTaper::Distance {
        add_scalar(
            &section,
            "Start taper distance",
            &value.start.taper_distance,
            context,
            ScalarKind::Pixels,
            "paint-stroke-start-taper-distance",
            stroke_start_taper_distance,
            stroke_start_taper_distance_mut,
        );
    }
    section.add_wide_control(&bool_control(
        "End cap",
        &value.end.cap,
        value.end.cap.fallback().get(),
        context,
        BoolTarget::ItemValue {
            get: stroke_end_cap,
            get_mut: stroke_end_cap_mut,
            scope: "paint-stroke-end-cap",
            mutated: bump_revision_for_key,
        },
    ));
    section.add_wide_control(&taper_control(
        "End taper",
        &value.end.taper,
        false,
        context,
    ));
    if value.end.taper.value_at(local_time(context)) == PaintTaper::Distance {
        add_scalar(
            &section,
            "End taper distance",
            &value.end.taper_distance,
            context,
            ScalarKind::Pixels,
            "paint-stroke-end-taper-distance",
            stroke_end_taper_distance,
            stroke_end_taper_distance_mut,
        );
    }
    vec![section.into_widget()]
}

fn drawing_control(value: &TimelineValue<PaintDrawing>, context: &InspectorContext) -> gtk::Widget {
    let time = local_time(context);
    let drawing = value.value_at(time);
    let summary = gtk::Label::builder()
        .label(shrimply_ui_foundation::i18n::text_args(
            "%{strokes} strokes, %{fills} fills",
            &[
                ("strokes", drawing.strokes.len().to_string()),
                ("fills", drawing.fills.len().to_string()),
            ],
        ))
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let mut body = Vec::new();
    if matches!(&value.base, TimelineBase::Keyframes(_)) {
        body.push(drawing_keyframe_graph(value, context));
    }
    if value
        .expression
        .as_ref()
        .is_some_and(|expression| expression.enabled)
    {
        body.push(drawing_expression_editor(value, context));
    }
    let keyframe_context = context.detached();
    let expression_context = context.detached();
    crate::timeline_value::layered::wide_control(
        "Drawing",
        value,
        summary.upcast(),
        body,
        move |enabled| toggle_drawing_keyframes(&keyframe_context, enabled),
        move |enabled| toggle_drawing_expression(&expression_context, enabled),
    )
}

fn drawing_keyframe_graph(
    value: &TimelineValue<PaintDrawing>,
    context: &InspectorContext,
) -> gtk::Widget {
    let Some(key) = context.selected_item.clone() else {
        return gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast();
    };
    let graph = drawing_graph(value);
    let duration =
        crate::video::visual_duration(&context.project.borrow(), key.clone()).unwrap_or(Time::ZERO);
    let built = crate::keyframe_editor::build(
        context,
        gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(),
        graph,
        (Time::ZERO, duration),
        format!("paint-drawing:{}", value.id),
        drawing_graph_actions(context, key.clone()),
    );
    let project = context.project.clone();
    crate::keyframe_editor::connect_graph_refresh(
        context,
        "inspector paint drawing keyframe graph refresh",
        built.update_graph.clone(),
        move || {
            selected_paint(&project.borrow(), key.clone())
                .map(|paint| drawing_graph(&paint.drawing))
        },
    );
    built.widget
}

fn drawing_graph(value: &TimelineValue<PaintDrawing>) -> crate::keyframe_editor::KeyframeGraph {
    let TimelineBase::Keyframes(keyframes) = &value.base else {
        return crate::keyframe_editor::KeyframeGraph::Speed {
            segments: Vec::new(),
            keys: Vec::new(),
            static_value: 0.0,
        };
    };
    crate::keyframe_editor::KeyframeGraph::Speed {
        segments: keyframes
            .windows(2)
            .filter_map(|pair| {
                let seconds = pair[1].time.signed_sub(pair[0].time).as_secs_f64();
                (seconds > f64::EPSILON).then(|| crate::keyframe_editor::SpeedSegment {
                    owner_id: pair[0].id,
                    start: pair[0].time,
                    end: pair[1].time,
                    value: 1.0 / seconds,
                    interpolation: pair[0].interpolation_to_next,
                })
            })
            .collect(),
        keys: keyframes.iter().map(|keyframe| keyframe.time).collect(),
        static_value: 0.0,
    }
}

fn drawing_graph_actions(
    context: &InspectorContext,
    key: SelectedItem,
) -> crate::keyframe_editor::KeyframeEditorActions {
    let project = context.project.clone();
    let player = context.player_state.clone();
    crate::keyframe_editor::KeyframeEditorActions {
        playhead: {
            let project = project.clone();
            let player = player.clone();
            let key = key.clone();
            Rc::new(move || {
                let position = player_state::snapshot(&player).position;
                crate::video::visual_animation_time(&project.borrow(), key.clone(), position)
                    .unwrap_or(Time::ZERO)
            })
        },
        select_time: {
            let project = project.clone();
            let player = player.clone();
            let key = key.clone();
            Rc::new(move |time| {
                if let Some(position) =
                    crate::video::visual_global_time(&project.borrow(), key.clone(), time)
                {
                    player_state::seek_time(&player, position);
                }
            })
        },
        add_at_time: {
            let context = context.detached();
            Rc::new(move |time| add_drawing_key_at(&context, time))
        },
        delete_at_time: {
            let context = context.detached();
            Rc::new(move |time| delete_drawing_key_at(&context, time))
        },
        update_point: {
            let context = context.detached();
            Rc::new(move |old, time, _| move_drawing_key(&context, old, time))
        },
        copy_keyframes: {
            let project = project.clone();
            let key = key.clone();
            Rc::new(move |times| {
                selected_paint(&project.borrow(), key.clone())
                    .and_then(|paint| crate::keyframe_model::copy_keyframes(&paint.drawing, times))
            })
        },
        paste_keyframes: {
            let project = project.clone();
            let player = player.clone();
            let key = key.clone();
            Rc::new(move |clipboard, time| {
                let mut project = project.borrow_mut();
                let step = crate::keyframe_editor::project_frame_step(&project);
                let paint = selected_paint_mut(&mut project, key.clone())?;
                let times = crate::keyframe_model::paste_keyframes(
                    &mut paint.drawing,
                    clipboard,
                    time,
                    step,
                )?;
                if let TimelineBase::Keyframes(keyframes) = &mut paint.drawing.base {
                    for keyframe in keyframes
                        .iter_mut()
                        .filter(|keyframe| times.contains(&keyframe.time))
                    {
                        regenerate_drawing_edit_ids(&mut keyframe.value);
                    }
                }
                bump_revision(paint);
                shrimply_project::project::commit_edit(&project, "paste-paint-drawing-keyframes");
                drop(project);
                player_state::refresh_project(&player, paint_refresh());
                Some(times)
            })
        },
        set_interpolation: Some({
            let context = context.detached();
            Rc::new(move |id, interpolation| {
                update_drawing(&context, "paint-drawing-easing", false, move |value| {
                    let TimelineBase::Keyframes(keyframes) = &mut value.base else {
                        return false;
                    };
                    let Some(keyframe) = keyframes.iter_mut().find(|keyframe| keyframe.id == id)
                    else {
                        return false;
                    };
                    if keyframe.interpolation_to_next == interpolation {
                        return false;
                    }
                    keyframe.interpolation_to_next = interpolation;
                    true
                });
            })
        }),
        text_interpolation: None,
        toggle_playback: Rc::new(move || player_state::toggle_playing(&player)),
    }
}

fn drawing_expression_editor(
    value: &TimelineValue<PaintDrawing>,
    context: &InspectorContext,
) -> gtk::Widget {
    let source = value.expression_source().map(str::to_string);
    let context = context.detached();
    crate::rhai_editor::editor(
        source,
        crate::rhai_editor::ExpressionValue::Drawing,
        move |source| update_drawing_expression(&context, source),
    )
}

fn toggle_drawing_keyframes(context: &InspectorContext, enabled: bool) {
    let step = crate::keyframe_editor::project_frame_step(&context.project.borrow());
    let time = crate::keyframe_model::snap_to_frame(local_time(context), step);
    update_drawing(context, "paint-drawing-keyframes", true, move |value| {
        let current = value.value_at(time);
        match (&mut value.base, enabled) {
            (TimelineBase::Const(_), false) | (TimelineBase::Keyframes(_), true) => false,
            (base @ TimelineBase::Const(_), true) => {
                *base = TimelineBase::Keyframes(vec![PaintDrawing::keyframe(time, current)]);
                true
            }
            (base @ TimelineBase::Keyframes(_), false) => {
                *base = TimelineBase::Const(current);
                true
            }
        }
    });
}

fn toggle_drawing_expression(context: &InspectorContext, enabled: bool) {
    update_drawing(
        context,
        "paint-drawing-expression",
        true,
        move |value| match &mut value.expression {
            Some(expression) if expression.enabled != enabled => {
                expression.enabled = enabled;
                true
            }
            Some(_) => false,
            None if enabled => {
                value.expression = Some(TimelineExpression {
                    id: Uuid::new_v4(),
                    enabled: true,
                    source: "value".to_string(),
                });
                true
            }
            None => false,
        },
    );
}

fn add_drawing_key_at(context: &InspectorContext, time: Time) {
    let step = crate::keyframe_editor::project_frame_step(&context.project.borrow());
    update_drawing(context, "add-paint-drawing-keyframe", true, move |value| {
        let time = crate::keyframe_model::snap_to_frame(time, step);
        let TimelineBase::Keyframes(keyframes) = &mut value.base else {
            return false;
        };
        if keyframes
            .iter()
            .any(|keyframe| crate::keyframe_model::same_frame(keyframe.time, time, step))
        {
            return false;
        }
        keyframes.push(PaintDrawing::keyframe(time, PaintDrawing::default()));
        keyframes.sort_by_key(|keyframe| keyframe.time);
        true
    });
}

fn delete_drawing_key_at(context: &InspectorContext, time: Time) {
    let step = crate::keyframe_editor::project_frame_step(&context.project.borrow());
    update_drawing(
        context,
        "delete-paint-drawing-keyframe",
        true,
        move |value| {
            let TimelineBase::Keyframes(keyframes) = &mut value.base else {
                return false;
            };
            let Some(index) = keyframes
                .iter()
                .position(|keyframe| crate::keyframe_model::same_frame(keyframe.time, time, step))
            else {
                return false;
            };
            let removed = keyframes.remove(index).value;
            if keyframes.is_empty() {
                value.base = TimelineBase::Const(removed);
            }
            true
        },
    );
}

fn move_drawing_key(context: &InspectorContext, old: Time, time: Time) {
    update_drawing(
        context,
        "move-paint-drawing-keyframe",
        false,
        move |value| {
            let TimelineBase::Keyframes(keyframes) = &mut value.base else {
                return false;
            };
            let Some(keyframe) = keyframes
                .iter_mut()
                .find(|keyframe| keyframe.time.approx_eq(old))
            else {
                return false;
            };
            keyframe.time = time;
            keyframes.sort_by_key(|keyframe| keyframe.time);
            true
        },
    );
}

fn update_drawing_expression(context: &InspectorContext, source: String) {
    update_drawing(context, "paint-drawing-expression", false, move |value| {
        let Some(expression) = &mut value.expression else {
            return false;
        };
        if expression.source == source {
            return false;
        }
        expression.source = source;
        true
    });
}

fn update_drawing(
    context: &InspectorContext,
    commit_name: &'static str,
    refresh_inspector: bool,
    update: impl FnOnce(&mut TimelineValue<PaintDrawing>) -> bool,
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(paint) = selected_paint_mut(&mut project, key) else {
        return;
    };
    if !update(&mut paint.drawing) {
        return;
    }
    bump_revision(paint);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: refresh_inspector,
            ..ProjectChange::default()
        },
    );
    if refresh_inspector {
        (context.refresh)();
    }
}

fn regenerate_drawing_edit_ids(drawing: &mut PaintDrawing) {
    for stroke in &mut drawing.strokes {
        stroke.id = Uuid::new_v4();
    }
    for fill in &mut drawing.fills {
        fill.id = Uuid::new_v4();
    }
}

fn visit_drawings_mut(
    value: &mut TimelineValue<PaintDrawing>,
    mut visit: impl FnMut(&mut PaintDrawing),
) {
    match &mut value.base {
        TimelineBase::Const(drawing) => visit(drawing),
        TimelineBase::Keyframes(keyframes) => {
            for keyframe in keyframes {
                visit(&mut keyframe.value);
            }
        }
    }
}

fn add_texture_controls(
    section: &crate::section::InspectorSection,
    entry: &PaintPaletteEntry,
    context: &InspectorContext,
) {
    section.add_wide_control(&texture_picker(entry, context));
    let Some(texture) = &entry.texture else {
        return;
    };
    add_palette_scalar(
        section,
        "Texture scale",
        &texture.repeat_scale,
        context,
        ScalarKind::TextureScale,
        "paint-texture-scale",
    );
    add_palette_scalar(
        section,
        "Texture rotation",
        &texture.rotation_degrees,
        context,
        ScalarKind::Degrees,
        "paint-texture-rotation",
    );
}

fn taper_control(
    label: &str,
    value: &TimelineValue<PaintTaper>,
    start: bool,
    context: &InspectorContext,
) -> gtk::Widget {
    let (get, get_mut, commit_name) = if start {
        (
            stroke_start_taper as TaperGet,
            stroke_start_taper_mut as TaperGetMut,
            "paint-stroke-start-taper",
        )
    } else {
        (
            stroke_end_taper as TaperGet,
            stroke_end_taper_mut as TaperGetMut,
            "paint-stroke-end-taper",
        )
    };
    step_control(
        label,
        value,
        context,
        StepTarget::new(get, get_mut, commit_name, paint_refresh())
            .mark_mutated(bump_revision_for_key)
            .refresh_inspector_on_value_change(),
    )
}

fn texture_picker(entry: &PaintPaletteEntry, context: &InspectorContext) -> gtk::Widget {
    let texture = entry.texture.as_ref();
    let color_id = entry.color.id;
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_hexpand(true);
    let filename = texture
        .and_then(|texture| texture.image_path.file_name())
        .and_then(|filename| filename.to_str())
        .unwrap_or("None");
    let filename_label = gtk::Label::builder()
        .label(filename)
        .hexpand(true)
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["dim-label"])
        .build();
    filename_label.set_tooltip_text(texture.and_then(|texture| texture.image_path.to_str()));
    row.append(&filename_label);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    actions.add_css_class("linked");
    let choose = gtk::Button::with_label(if texture.is_some() {
        "Replace"
    } else {
        "Select"
    });
    let clear = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text(tr!("Clear texture").as_ref())
        .sensitive(texture.is_some())
        .build();
    actions.append(&choose);
    actions.append(&clear);
    row.append(&actions);

    let choose_context = context.detached();
    choose.connect_clicked(move |_| {
        let label = "Select paint texture";
        let filter = gtk::FileFilter::new();
        filter.set_name_i18n("Images");
        filter.add_mime_type("image/*");
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
                let Ok(file) = result else { return };
                let Some(path) = file.path() else { return };
                update_texture_path(&context, color_id, path);
            },
        );
    });
    let context = context.detached();
    clear.connect_clicked(move |_| {
        update_discrete(&context, "paint-texture-clear", move |paint| {
            let Some(texture) = paint
                .palette
                .iter_mut()
                .find(|entry| entry.color.id == color_id)
                .map(|entry| &mut entry.texture)
            else {
                return false;
            };
            if texture.is_none() {
                return false;
            }
            *texture = None;
            true
        });
    });
    control_row("Texture", &row)
}

fn update_texture_path(context: &InspectorContext, color_id: uuid::Uuid, path: PathBuf) {
    update_discrete(context, "paint-texture-path", move |paint| {
        let Some(texture) = paint
            .palette
            .iter_mut()
            .find(|entry| entry.color.id == color_id)
            .map(|entry| &mut entry.texture)
        else {
            return false;
        };
        match texture {
            Some(texture) if texture.image_path.path() == path => return false,
            Some(texture) => texture.image_path = path.into(),
            texture @ None => *texture = Some(PaintTextureOptions::new(path)),
        }
        true
    });
}

type ScalarGet = for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<f32>>;
type ScalarGetMut = for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<f32>>;
type TaperGet = for<'a> fn(&'a Project, SelectedItem) -> Option<&'a TimelineValue<PaintTaper>>;
type TaperGetMut =
    for<'a> fn(&'a mut Project, SelectedItem) -> Option<&'a mut TimelineValue<PaintTaper>>;

#[allow(clippy::too_many_arguments)]
fn add_scalar(
    section: &crate::section::InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    kind: ScalarKind,
    commit_name: &'static str,
    get: ScalarGet,
    get_mut: ScalarGetMut,
) {
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        ScalarTarget {
            access: ScalarAccess::ItemWithMutation {
                get,
                get_mut,
                mutated: bump_revision_for_key,
            },
            scope_id: Some(value.id),
            local_time: crate::video::visual_local_time,
            global_time: crate::video::visual_global_time,
            duration: crate::video::visual_duration,
            refresh: paint_refresh(),
            commit_name,
        },
        kind.spec(),
    ));
}

fn add_palette_scalar(
    section: &crate::section::InspectorSection,
    label: &str,
    value: &TimelineValue<f32>,
    context: &InspectorContext,
    kind: ScalarKind,
    commit_name: &'static str,
) {
    section.add_wide_control(&scalar_control(
        label,
        value,
        context,
        ScalarTarget {
            access: ScalarAccess::PaintPalette { value_id: value.id },
            scope_id: Some(value.id),
            local_time: crate::video::visual_local_time,
            global_time: crate::video::visual_global_time,
            duration: crate::video::visual_duration,
            refresh: paint_refresh(),
            commit_name,
        },
        kind.spec(),
    ));
}

#[derive(Clone, Copy)]
enum ScalarKind {
    Pixels,
    Factor,
    TextureScale,
    Degrees,
}

impl ScalarKind {
    fn spec(self) -> ScalarSpec {
        let (drag_step, digits, minimum, maximum, unit_name, rotating_icon) = match self {
            Self::Pixels => (1.0, 1, Some(0.0), None, Some("px"), None),
            Self::Factor => (0.01, 2, Some(0.0), Some(1.0), None, None),
            Self::TextureScale => (0.01, 2, Some(MIN_TEXTURE_SCALE), None, None, None),
            Self::Degrees => (
                0.1,
                1,
                None,
                None,
                Some("°"),
                Some(("arrow3-up-symbolic", 0.0)),
            ),
        };
        ScalarSpec {
            drag_step,
            digits,
            integer: false,
            width_chars: 9,
            minimum,
            maximum,
            unit_name,
            rotating_icon,
            display: f64::from,
            store: |value| value as f32,
            clamp: match self {
                Self::Pixels => |value| value.max(0.0),
                Self::Factor => |value| value.clamp(0.0, 1.0),
                Self::TextureScale => |value| value.max(MIN_TEXTURE_SCALE as f32),
                Self::Degrees => |value| value,
            },
        }
    }
}

fn local_time(context: &InspectorContext) -> Time {
    let position = player_state::snapshot(&context.player_state).position;
    context
        .selected_item
        .clone()
        .and_then(|key| {
            crate::video::visual_animation_time(&context.project.borrow(), key, position)
        })
        .unwrap_or(Time::ZERO)
}

fn paint_refresh() -> ProjectChange {
    ProjectChange {
        video: true,
        inspector: true,
        ..ProjectChange::default()
    }
}

fn update_discrete(
    context: &InspectorContext,
    commit_name: &'static str,
    update: impl FnOnce(&mut PaintItem) -> bool,
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(paint) = selected_paint_mut(&mut project, key) else {
        return;
    };
    if !update(paint) {
        return;
    }
    bump_revision(paint);
    shrimply_project::project::commit_edit(&project, commit_name);
    drop(project);
    player_state::refresh_project(&context.player_state, paint_refresh());
    (context.refresh)();
}

pub(super) fn selected_paint(project: &Project, key: SelectedItem) -> Option<&PaintItem> {
    let item = project.video_item(&key)?;
    let VideoItemContent::Paint(paint) = &item.content else {
        return None;
    };
    Some(paint)
}

pub(super) fn selected_paint_mut(
    project: &mut Project,
    key: SelectedItem,
) -> Option<&mut PaintItem> {
    let item = project.video_item_mut(&key)?;
    let VideoItemContent::Paint(paint) = &mut item.content else {
        return None;
    };
    Some(paint)
}

pub(super) fn bump_revision_for_key(project: &mut Project, key: SelectedItem) {
    if let Some(paint) = selected_paint_mut(project, key) {
        bump_revision(paint);
    }
}

fn bump_revision(paint: &mut PaintItem) {
    paint.revision = paint
        .revision
        .checked_add(1)
        .expect("paint revision overflow");
}

macro_rules! timeline_accessors {
    ($get:ident, $get_mut:ident, $value:ty, $($field:ident).+) => {
        fn $get(project: &Project, key: SelectedItem) -> Option<&TimelineValue<$value>> {
            Some(&selected_paint(project, key)?$(.$field)+)
        }

        fn $get_mut(
            project: &mut Project,
            key: SelectedItem,
        ) -> Option<&mut TimelineValue<$value>> {
            Some(&mut selected_paint_mut(project, key)?$(.$field)+)
        }
    };
}

timeline_accessors!(stroke_width, stroke_width_mut, f32, stroke.width);
timeline_accessors!(stroke_thinning, stroke_thinning_mut, f32, stroke.thinning);
timeline_accessors!(
    stroke_smoothing,
    stroke_smoothing_mut,
    f32,
    stroke.smoothing
);
timeline_accessors!(
    stroke_streamline,
    stroke_streamline_mut,
    f32,
    stroke.streamline
);
timeline_accessors!(
    stroke_simplification,
    stroke_simplification_mut,
    f32,
    stroke.simplification_tolerance
);
timeline_accessors!(
    stroke_subdivision,
    stroke_subdivision_mut,
    f32,
    stroke.maximum_subdivision_spacing
);
timeline_accessors!(
    stroke_start_cap,
    stroke_start_cap_mut,
    TimelineBool,
    stroke.start.cap
);
timeline_accessors!(
    stroke_end_cap,
    stroke_end_cap_mut,
    TimelineBool,
    stroke.end.cap
);
timeline_accessors!(
    stroke_start_taper,
    stroke_start_taper_mut,
    PaintTaper,
    stroke.start.taper
);
timeline_accessors!(
    stroke_end_taper,
    stroke_end_taper_mut,
    PaintTaper,
    stroke.end.taper
);
timeline_accessors!(
    stroke_start_taper_distance,
    stroke_start_taper_distance_mut,
    f32,
    stroke.start.taper_distance
);
timeline_accessors!(
    stroke_end_taper_distance,
    stroke_end_taper_distance_mut,
    f32,
    stroke.end.taper_distance
);
struct StrokeTransform(PaintTransform);

impl Default for StrokeTransform {
    fn default() -> Self {
        Self(PaintTransform::from_resolved(ResolvedTransform::IDENTITY))
    }
}
