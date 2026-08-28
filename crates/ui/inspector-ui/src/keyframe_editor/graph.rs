use super::*;
use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nWidgetExt;

pub(crate) fn connect_graph_refresh_impl(
    context: &InspectorContext,
    label: &'static str,
    update_graph: Rc<dyn Fn(KeyframeGraph)>,
    graph: impl Fn() -> Option<KeyframeGraph> + 'static,
) {
    let alive = Rc::downgrade(&context.listener_scope);
    player_state::connect_while_alive_named(
        &context.player_state,
        label,
        move || alive.upgrade().is_some(),
        move |event| {
            if matches!(
                event,
                player_state::PlayerEvent::State(_) | player_state::PlayerEvent::Project(_)
            ) && let Some(graph) = graph()
            {
                update_graph(graph);
            }
        },
    );
}

pub(super) fn graph_content_height(height: f64) -> f64 {
    (height - GRAPH_SLIDER_HEIGHT).max(1.0)
}

pub(super) fn current_graph_domain(
    view: &mut GraphViewState,
    item_range: GraphDomain,
    width: f64,
) -> GraphDomain {
    view.initialize(item_range, width);
    view.clamp(item_range, width);
    view.domain(item_range, width)
}

pub(super) fn graph_plot_width(width: f64) -> f64 {
    (width - GRAPH_PAD * 2.0).max(1.0)
}

pub(super) fn graph_duration_seconds((start, end): GraphDomain) -> f64 {
    end.signed_sub(start).as_secs_f64().max(0.001)
}

pub(super) fn clip_bounded_visible_area(
    project: &Project,
    selected_item: Option<&crate::InspectedItem>,
    (start, end): GraphDomain,
) -> GraphDomain {
    let clip_duration = selected_item
        .and_then(|key| project.item(key))
        .map(|item| {
            let (start, end) = item.times();
            end.saturating_sub(start).max(start.saturating_sub(end))
        })
        .unwrap_or(Time::ZERO);
    (start, end.max(start.saturating_add(clip_duration)))
}

pub(super) fn min_graph_seconds_per_pixel(duration_seconds: f64) -> f64 {
    (duration_seconds / 10_000.0).max(0.000001)
}

pub(super) fn update_graph_scroll(
    view: &mut GraphViewState,
    item_range: GraphDomain,
    width: f64,
    pointer_x: f64,
    dx: f64,
    dy: f64,
    ctrl: bool,
) -> Option<(GraphOverscrollEdge, f64)> {
    view.initialize(item_range, width);
    view.clamp(item_range, width);
    let delta = if dx.abs() > f64::EPSILON { dx } else { dy };
    if delta.abs() <= f64::EPSILON {
        return None;
    }

    if ctrl {
        let pointer_time = time_at_x(pointer_x, width, view.domain(item_range, width));
        let zoom = if delta < 0.0 { 0.8 } else { 1.25 };
        let pointer_plot_x = (pointer_x - GRAPH_PAD).clamp(0.0, graph_plot_width(width));
        view.seconds_per_pixel *= zoom;
        view.scroll_seconds = pointer_time.as_secs_f64() - pointer_plot_x * view.seconds_per_pixel;
        view.clamp(item_range, width);
        None
    } else {
        let target = view.scroll_seconds + delta * view.seconds_per_pixel * 120.0;
        set_graph_scroll_seconds(view, item_range, width, target)
    }
}

pub(super) fn graph_scroll_should_propagate(
    view: GraphViewState,
    item_range: GraphDomain,
    width: f64,
    delta: f64,
) -> bool {
    if !graph_can_scroll(view, item_range, width) {
        return true;
    }
    let min_scroll = item_range.0.as_secs_f64();
    let max_scroll = max_graph_scroll_seconds(item_range, width, view.seconds_per_pixel);
    let edge_tolerance = view.seconds_per_pixel * 0.5;
    (delta < 0.0 && view.scroll_seconds <= min_scroll + edge_tolerance)
        || (delta > 0.0 && view.scroll_seconds >= max_scroll - edge_tolerance)
}

pub(super) fn drag_cursor_time(
    view: &mut GraphViewState,
    item_range: GraphDomain,
    width: f64,
    x: f64,
) -> (Time, Option<(GraphOverscrollEdge, f64)>) {
    view.initialize(item_range, width);
    view.clamp(item_range, width);
    let left = GRAPH_PAD;
    let right = (width - GRAPH_PAD).max(left);
    let target = if x < left {
        view.scroll_seconds - (left - x) * view.seconds_per_pixel
    } else if x > right {
        view.scroll_seconds + (x - right) * view.seconds_per_pixel
    } else {
        view.scroll_seconds
    };
    let edge = set_graph_scroll_seconds(view, item_range, width, target);
    let time = clamp_graph_time(
        time_at_x(x.clamp(left, right), width, view.domain(item_range, width)),
        item_range,
    );
    (time, edge)
}

pub(super) fn snap_keyframe_time(
    time: Time,
    item_range: GraphDomain,
    enabled: bool,
    radius_px: f64,
    seconds_per_pixel: f64,
    timeline_cursor: Time,
) -> Time {
    let time = clamp_graph_time(time, item_range);
    if !enabled {
        return time;
    }
    let radius = radius_px * seconds_per_pixel;
    if timeline_cursor >= item_range.0
        && timeline_cursor <= item_range.1
        && (timeline_cursor.as_secs_f64() - time.as_secs_f64()).abs() <= radius
    {
        timeline_cursor
    } else {
        time
    }
}

pub(super) fn clamp_graph_time(time: Time, (start, end): GraphDomain) -> Time {
    time.clamp(start, end)
}

pub(super) fn set_graph_scroll_seconds(
    view: &mut GraphViewState,
    item_range: GraphDomain,
    width: f64,
    target: f64,
) -> Option<(GraphOverscrollEdge, f64)> {
    view.initialize(item_range, width);
    if !graph_can_scroll(*view, item_range, width) {
        view.clamp(item_range, width);
        return None;
    }
    let min_scroll = item_range.0.as_secs_f64();
    let max_scroll = max_graph_scroll_seconds(item_range, width, view.seconds_per_pixel);
    let overscroll = if target < min_scroll {
        Some((
            GraphOverscrollEdge::Left,
            ((min_scroll - target) / view.seconds_per_pixel)
                .clamp(1.0, shrimply_skia_adw_ui::OVERSHOOT_MAX_DISTANCE),
        ))
    } else if target > max_scroll {
        Some((
            GraphOverscrollEdge::Right,
            ((target - max_scroll) / view.seconds_per_pixel)
                .clamp(1.0, shrimply_skia_adw_ui::OVERSHOOT_MAX_DISTANCE),
        ))
    } else {
        None
    };
    view.scroll_seconds = target;
    view.clamp(item_range, width);
    overscroll
}

pub(super) fn graph_can_scroll(view: GraphViewState, item_range: GraphDomain, width: f64) -> bool {
    graph_plot_width(width) * view.seconds_per_pixel < graph_duration_seconds(item_range)
}

pub(super) fn max_graph_scroll_seconds(
    item_range: GraphDomain,
    width: f64,
    seconds_per_pixel: f64,
) -> f64 {
    let visible_seconds = (graph_plot_width(width) * seconds_per_pixel)
        .clamp(0.0, graph_duration_seconds(item_range));
    (item_range.1.as_secs_f64() - visible_seconds).max(item_range.0.as_secs_f64())
}

pub(super) fn update_graph_overscroll(
    overscroll: &Rc<RefCell<Option<GraphOverscroll>>>,
    edge: Option<(GraphOverscrollEdge, f64)>,
) -> bool {
    if let Some((edge, distance)) = edge {
        *overscroll.borrow_mut() = Some(GraphOverscroll {
            edge,
            started_at: Instant::now(),
            distance,
        });
        true
    } else {
        overscroll.borrow_mut().take();
        false
    }
}

pub(super) fn apply_graph_scroll_animation(
    view: &mut GraphViewState,
    item_range: GraphDomain,
    scrollbar_lifecycle: &Rc<RefCell<shrimply_skia_adw_ui::slider::Lifecycle>>,
) -> bool {
    scrollbar_lifecycle.borrow_mut().apply_scroll(|value| {
        view.scroll_seconds = item_range.0.as_secs_f64() + value;
    })
}

pub(super) fn start_graph_animation_tick(
    area: &gtk::GLArea,
    overscroll: Rc<RefCell<Option<GraphOverscroll>>>,
    scrollbar_lifecycle: Rc<RefCell<shrimply_skia_adw_ui::slider::Lifecycle>>,
    active: Rc<RefCell<bool>>,
) {
    {
        let mut active = active.borrow_mut();
        if *active {
            return;
        }
        *active = true;
    }

    area.add_tick_callback(move |area, _| {
        let overscroll_active = (*overscroll.borrow()).is_some_and(|overscroll| {
            shrimply_skia_adw_ui::overshoot_distance(
                overscroll.distance,
                overscroll.started_at.elapsed(),
            ) > shrimply_skia_adw_ui::OVERSHOOT_VISIBLE_DISTANCE
        });
        if !overscroll_active {
            overscroll.borrow_mut().take();
        }
        let should_continue = overscroll_active || scrollbar_lifecycle.borrow().animating();
        if !should_continue {
            *active.borrow_mut() = false;
        }

        area.queue_render();
        if should_continue {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });
}

pub(super) fn graph_scrollbar(
    view: GraphViewState,
    item_range: GraphDomain,
    width: f64,
    height: f64,
) -> Option<shrimply_skia_adw_ui::Scrollbar> {
    if !graph_can_scroll(view, item_range, width) {
        return None;
    }
    let visible_seconds = graph_plot_width(width) * view.seconds_per_pixel;
    Some(shrimply_skia_adw_ui::Scrollbar {
        axis: shrimply_skia_adw_ui::Axis::Horizontal,
        bounds: graph_scrollbar_bounds(width, height),
        content_length: graph_duration_seconds(item_range).max(visible_seconds),
        viewport_length: visible_seconds,
        value: (view.scroll_seconds - item_range.0.as_secs_f64()).max(0.0),
        color: Color::LIGHT1,
        outline_color: Color::<f32>::from_rgb8_alpha(0x00, 0x00, 0x0c, 0.95),
        state: shrimply_skia_adw_ui::slider::idle_state(),
    })
}

pub(super) fn graph_scrollbar_bounds(width: f64, height: f64) -> shrimply_skia_adw_ui::Rect {
    shrimply_skia_adw_ui::Rect::from_xywh(
        0.0,
        graph_content_height(height) as f32,
        width.max(0.0) as f32,
        GRAPH_SLIDER_HEIGHT as f32,
    )
}

pub(super) fn sync_keyframe_button(button: &gtk::Button, selected_time: Option<Time>) {
    if selected_time.is_some() {
        button.set_icon_name("list-remove-symbolic");
        button.set_tooltip_i18n("Delete selected keyframe");
    } else {
        button.set_icon_name("list-add-symbolic");
        button.set_tooltip_i18n("Add keyframe at playhead");
    }
}

pub(super) fn sync_keyframe_controls(
    previous: &gtk::Button,
    add: &gtk::Button,
    next: &gtk::Button,
    graph: &KeyframeGraph,
    navigation_playhead: Time,
    edit_playhead: Time,
    frame_step: Time,
) {
    let times = graph.key_times();
    previous.set_sensitive(
        keyframe_model::previous_key(&times, navigation_playhead, frame_step).is_some(),
    );
    next.set_sensitive(keyframe_model::next_key(&times, navigation_playhead, frame_step).is_some());
    sync_keyframe_button(
        add,
        keyframe_model::key_at(&times, edit_playhead, frame_step),
    );
}

pub(super) fn set_key_selection(
    selection: &mut KeyframeSelection,
    mut selected: Vec<Time>,
    focused: Option<Time>,
) {
    selected.sort();
    selected.dedup_by(|left, right| left.approx_eq(*right));
    selection.focused =
        focused.filter(|time| selected.iter().any(|selected| selected.approx_eq(*time)));
    selection.selected = selected;
}

pub(super) fn select_single_key(selection: &mut KeyframeSelection, time: Time) {
    set_key_selection(selection, vec![time], Some(time));
}

pub(super) fn add_key_to_selection(selection: &mut KeyframeSelection, time: Time) {
    if !selection
        .selected
        .iter()
        .any(|selected| selected.approx_eq(time))
    {
        selection.selected.push(time);
        selection.selected.sort();
    }
    selection.focused = Some(time);
}

pub(super) fn key_is_selected(selection: &KeyframeSelection, time: Time) -> bool {
    selection
        .selected
        .iter()
        .any(|selected| selected.approx_eq(time))
}

pub(super) fn select_keys_in_box(
    graph: &KeyframeGraph,
    domain: GraphDomain,
    width: f64,
    height: f64,
    frame_step: Time,
    selection_box: GraphSelectionBox,
    previous_selection: &[Time],
) -> Vec<Time> {
    let left = selection_box.start_x.min(selection_box.end_x);
    let right = selection_box.start_x.max(selection_box.end_x);
    let top = selection_box.start_y.min(selection_box.end_y);
    let bottom = selection_box.start_y.max(selection_box.end_y);
    let range = graph_range(graph);
    let mut selected = if selection_box.add_to_selection {
        previous_selection.to_vec()
    } else {
        Vec::new()
    };
    for point in graph_key_points(graph) {
        let (x, y) = if matches!(graph, KeyframeGraph::Step { .. }) {
            (
                shrimply_discrete_keyframe_graph_ui::key_x(point.time, width, domain, frame_step),
                shrimply_discrete_keyframe_graph_ui::key_y(height, CURSOR_LANE_HEIGHT),
            )
        } else {
            raw_point(point, width, height, domain, range)
        };
        if x >= left
            && x <= right
            && y >= top
            && y <= bottom
            && !selected
                .iter()
                .any(|selected| selected.approx_eq(point.time))
        {
            selected.push(point.time);
        }
    }
    selected.sort();
    selected.dedup_by(|left, right| left.approx_eq(*right));
    selected
}

pub(super) fn graph_key_points(graph: &KeyframeGraph) -> Vec<KeyframePoint> {
    match graph {
        KeyframeGraph::Step { points } => points.clone(),
        KeyframeGraph::RawValue { points, .. } => points.clone(),
        KeyframeGraph::Speed {
            segments,
            keys,
            static_value,
            ..
        } if segments.is_empty() => keys
            .iter()
            .map(|time| KeyframePoint {
                time: *time,
                value: *static_value,
            })
            .collect(),
        KeyframeGraph::Speed { segments, .. } => {
            let mut points = Vec::new();
            for segment in segments {
                points.push(KeyframePoint {
                    time: segment.start,
                    value: segment_speed_at(segment, 0.0).unwrap_or(0.0),
                });
                points.push(KeyframePoint {
                    time: segment.end,
                    value: segment_speed_at(segment, 1.0).unwrap_or(0.0),
                });
            }
            points.sort_by_key(|point| point.time);
            points.dedup_by_key(|point| point.time);
            points
        }
    }
}

pub(super) fn graph_key_point(graph: &KeyframeGraph, time: Time) -> Option<KeyframePoint> {
    graph_key_points(graph)
        .into_iter()
        .find(|point| point.time.approx_eq(time))
}

pub(super) fn move_selected_graph_points(
    graph: &mut KeyframeGraph,
    selected_times: &[Time],
    focus_time: Time,
    requested_time: Time,
    requested_value: f64,
    item_range: GraphDomain,
) -> (Vec<(Time, Time, f64)>, Vec<Time>, Time) {
    let Some(focus_point) = graph_key_point(graph, focus_time) else {
        return (Vec::new(), selected_times.to_vec(), focus_time);
    };
    let selected_times = if selected_times
        .iter()
        .any(|selected| selected.approx_eq(focus_time))
    {
        selected_times.to_vec()
    } else {
        vec![focus_time]
    };
    let delta = constrained_key_delta(&selected_times, focus_time, requested_time, item_range);
    let delta_value = if matches!(graph, KeyframeGraph::RawValue { .. }) {
        requested_value - focus_point.value
    } else {
        0.0
    };
    let mut updates = Vec::new();
    for old_time in &selected_times {
        let Some(point) = graph_key_point(graph, *old_time) else {
            continue;
        };
        let next_time = Time {
            seconds: point.time.seconds + delta.seconds,
        };
        let next_value = graph_edit_value(graph, point.value + delta_value);
        updates.push((point.time, next_time, next_value));
    }
    if delta > Time::ZERO {
        updates.sort_by_key(|(old_time, _, _)| std::cmp::Reverse(*old_time));
    } else {
        updates.sort_by_key(|(old_time, _, _)| *old_time);
    }
    for (old_time, next_time, next_value) in &updates {
        update_graph_point(graph, *old_time, *next_time, *next_value);
    }
    let mut next_selected: Vec<_> = updates.iter().map(|(_, next_time, _)| *next_time).collect();
    next_selected.sort();
    next_selected.dedup_by(|left, right| left.approx_eq(*right));
    let next_focus = Time {
        seconds: focus_time.seconds + delta.seconds,
    };
    (updates, next_selected, next_focus)
}

pub(super) fn constrained_key_delta(
    selected_times: &[Time],
    focus_time: Time,
    requested_time: Time,
    item_range: GraphDomain,
) -> Time {
    let requested = Time {
        seconds: requested_time.seconds - focus_time.seconds,
    };
    let Some(min_time) = selected_times.iter().min() else {
        return requested;
    };
    let Some(max_time) = selected_times.iter().max() else {
        return requested;
    };
    let min_delta = Time {
        seconds: item_range.0.seconds - min_time.seconds,
    };
    let max_delta = Time {
        seconds: item_range.1.seconds - max_time.seconds,
    };
    requested.clamp(min_delta, max_delta)
}

pub(super) fn draw_selection_box(
    painter: &TimelinePainter,
    selection_box: GraphSelectionBox,
    content_height: f64,
) {
    let left = selection_box.start_x.min(selection_box.end_x);
    let right = selection_box.start_x.max(selection_box.end_x);
    let top = selection_box
        .start_y
        .min(selection_box.end_y)
        .clamp(CURSOR_LANE_HEIGHT, content_height);
    let bottom = selection_box
        .start_y
        .max(selection_box.end_y)
        .clamp(CURSOR_LANE_HEIGHT, content_height);
    if right <= left || bottom <= top {
        return;
    }
    let rect = Rect::from_min_size(
        vec2(left as f32, top as f32),
        vec2((right - left) as f32, (bottom - top) as f32),
    );
    painter.rect_filled(
        rect,
        0,
        Color::<f32>::from_rgb8_alpha(0x61, 0xa7, 0xff, 0.18),
    );
    painter.rect_stroke(
        rect,
        0,
        Stroke::new(1.0, Color::<f32>::from_rgb8_alpha(0x8c, 0xc3, 0xff, 0.78)),
        StrokeKind::Inside,
    );
}

pub(super) fn hit_graph_segment(
    graph: &KeyframeGraph,
    domain: GraphDomain,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
) -> Option<(Uuid, Interpolation)> {
    let mut closest = None;
    match graph {
        KeyframeGraph::Step { .. } => return None,
        KeyframeGraph::RawValue {
            points, segments, ..
        } => {
            let range = raw_range(points, segments);
            for segment in segments {
                let mut previous = None;
                for progress in curve_sample_progresses(segment.interpolation) {
                    let time = Time::from_seconds_f64(
                        segment.start.as_secs_f64()
                            + segment.end.signed_sub(segment.start).as_secs_f64() * progress,
                    );
                    let value = segment.start_value
                        + (segment.end_value - segment.start_value)
                            * segment.interpolation.value(progress);
                    let point = glam::DVec2::new(
                        time_x(time, width, domain),
                        value_y(value, height, range),
                    );
                    if let Some(start) = previous {
                        keep_closest_segment(
                            &mut closest,
                            shrimply_math_geometry::distance_to_dsegment(
                                glam::DVec2::new(x, y),
                                start,
                                point,
                            ),
                            segment.owner_id,
                            segment.interpolation,
                        );
                    }
                    previous = Some(point);
                }
            }
        }
        KeyframeGraph::Speed { segments, .. } => {
            let range = speed_range(segments);
            for segment in segments {
                let mut previous = None;
                for progress in curve_sample_progresses(segment.interpolation) {
                    let Some(speed) = segment_speed_at(segment, progress) else {
                        previous = None;
                        continue;
                    };
                    let time = Time::from_seconds_f64(
                        segment.start.as_secs_f64()
                            + segment.end.signed_sub(segment.start).as_secs_f64() * progress,
                    );
                    let point = glam::DVec2::new(
                        time_x(time, width, domain),
                        value_y(speed, height, range),
                    );
                    if let Some(start) = previous {
                        keep_closest_segment(
                            &mut closest,
                            shrimply_math_geometry::distance_to_dsegment(
                                glam::DVec2::new(x, y),
                                start,
                                point,
                            ),
                            segment.owner_id,
                            segment.interpolation,
                        );
                    }
                    previous = Some(point);
                }
            }
        }
    }
    closest
        .filter(|(distance, _, _)| *distance <= HIT_RADIUS)
        .map(|(_, owner_id, interpolation)| (owner_id, interpolation))
}

pub(super) fn graph_segment_at_x(
    graph: &KeyframeGraph,
    domain: GraphDomain,
    width: f64,
    x: f64,
) -> Option<(Uuid, Interpolation)> {
    let contains = |start, end| {
        let start = time_x(start, width, domain);
        let end = time_x(end, width, domain);
        x > start.min(end) && x < start.max(end)
    };
    match graph {
        KeyframeGraph::Step { .. } => None,
        KeyframeGraph::RawValue { segments, .. } => segments
            .iter()
            .find(|segment| contains(segment.start, segment.end))
            .map(|segment| (segment.owner_id, segment.interpolation)),
        KeyframeGraph::Speed { segments, .. } => segments
            .iter()
            .find(|segment| contains(segment.start, segment.end))
            .map(|segment| (segment.owner_id, segment.interpolation)),
    }
}

pub(super) fn keep_closest_segment(
    closest: &mut Option<(f64, Uuid, Interpolation)>,
    distance: f64,
    owner_id: Uuid,
    interpolation: Interpolation,
) {
    if closest
        .as_ref()
        .is_none_or(|(closest, _, _)| distance < *closest)
    {
        *closest = Some((distance, owner_id, interpolation));
    }
}

pub(super) fn show_interpolation_popover(
    graph: &gtk::GLArea,
    x: f64,
    y: f64,
    selected: Interpolation,
    owner_id: Uuid,
    changed: Option<Rc<dyn Fn(Uuid, Interpolation)>>,
    text_interpolation: Option<TextInterpolationSelection>,
) {
    let curve_picker = changed.is_some();
    let search = gtk::SearchEntry::builder()
        .placeholder_text(tr!("Search interpolations").as_ref())
        .hexpand(true)
        .build();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(280)
        .min_content_height(180)
        .max_content_height(240)
        .build();
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    let popover = gtk::Popover::builder()
        .child(&content)
        .autohide(true)
        .build();
    if let Some((selected, set)) = text_interpolation {
        for mode in TextInterpolation::ALL {
            let label = gtk::Label::builder()
                .label(tr!(mode.label()).as_ref())
                .halign(gtk::Align::Start)
                .xalign(0.0)
                .hexpand(true)
                .build();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&label);
            if mode == selected {
                row.append(&gtk::Image::from_icon_name("object-select-symbolic"));
            }
            let button = gtk::Button::builder()
                .child(&row)
                .halign(gtk::Align::Fill)
                .hexpand(true)
                .tooltip_text(match mode {
                    TextInterpolation::Jump => "Change all at once",
                    TextInterpolation::Type => "Clear and rewrite the whole text",
                    TextInterpolation::Append => "Edit after the shared beginning",
                    TextInterpolation::Insert => "Edit between the shared ends",
                    TextInterpolation::Diff => "Edit only the changed characters",
                    TextInterpolation::Decode => "Scramble, resize, then reveal the new text",
                })
                .build();
            button.add_css_class("flat");
            let set = set.clone();
            let popover = popover.clone();
            button.connect_clicked(move |_| {
                set(owner_id, mode);
                popover.popdown();
            });
            content.append(&button);
        }
    }
    if curve_picker {
        content.append(&search);
        content.append(&scroller);
    }
    popover.add_css_class("menu");
    popover.set_parent(graph);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    if let Some(changed) = changed {
        populate_interpolation_list(&list, "", selected, owner_id, &changed, &popover);
        search.connect_search_changed({
            let list = list.clone();
            let popover = popover.clone();
            move |search| {
                populate_interpolation_list(
                    &list,
                    search.text().as_str(),
                    selected,
                    owner_id,
                    &changed,
                    &popover,
                )
            }
        });
    }
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
    if curve_picker {
        search.grab_focus();
    }
}

pub(super) fn populate_interpolation_list(
    list: &gtk::Box,
    query: &str,
    selected: Interpolation,
    owner_id: Uuid,
    changed: &Rc<dyn Fn(Uuid, Interpolation)>,
    popover: &gtk::Popover,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let query = query.trim().to_lowercase();
    for interpolation in Interpolation::KEYFRAME {
        if !interpolation.label().to_lowercase().contains(&query) {
            continue;
        }
        let label = gtk::Label::builder()
            .label(tr!(interpolation.label()).as_ref())
            .halign(gtk::Align::Start)
            .xalign(0.0)
            .hexpand(true)
            .build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&label);
        if interpolation == selected {
            row.append(&gtk::Image::from_icon_name("object-select-symbolic"));
        }
        let button = gtk::Button::builder()
            .child(&row)
            .halign(gtk::Align::Fill)
            .hexpand(true)
            .build();
        button.add_css_class("flat");
        button.connect_clicked({
            let changed = changed.clone();
            let popover = popover.clone();
            move |_| {
                changed(owner_id, interpolation);
                popover.popdown();
            }
        });
        list.append(&button);
    }
}

pub(super) fn hit_graph_point(
    graph: &KeyframeGraph,
    domain: GraphDomain,
    width: f64,
    height: f64,
    frame_step: Time,
    x: f64,
    y: f64,
) -> Option<KeyframePoint> {
    match graph {
        KeyframeGraph::Step { points } => {
            hit_discrete_points(points, width, height, domain, frame_step, x, y)
        }
        KeyframeGraph::RawValue {
            points, segments, ..
        } => {
            let range = raw_range(points, segments);
            hit_points(points, width, height, domain, range, x, y)
        }
        KeyframeGraph::Speed {
            segments,
            keys,
            static_value,
            ..
        } => {
            if segments.is_empty() {
                let range = (0.0, static_value.max(1.0));
                let points: Vec<_> = keys
                    .iter()
                    .map(|time| KeyframePoint {
                        time: *time,
                        value: *static_value,
                    })
                    .collect();
                return hit_points(&points, width, height, domain, range, x, y);
            }
            let range = speed_range(segments);
            let points: Vec<_> = segments
                .iter()
                .flat_map(|segment| {
                    [
                        KeyframePoint {
                            time: segment.start,
                            value: segment_speed_at(segment, 0.0).unwrap_or(0.0),
                        },
                        KeyframePoint {
                            time: segment.end,
                            value: segment_speed_at(segment, 1.0).unwrap_or(0.0),
                        },
                    ]
                })
                .collect();
            hit_points(&points, width, height, domain, range, x, y)
        }
    }
}

pub(super) fn hit_discrete_points(
    points: &[KeyframePoint],
    width: f64,
    height: f64,
    domain: GraphDomain,
    frame_step: Time,
    x: f64,
    y: f64,
) -> Option<KeyframePoint> {
    let key_y = shrimply_discrete_keyframe_graph_ui::key_y(height, CURSOR_LANE_HEIGHT);
    points
        .iter()
        .copied()
        .filter_map(|point| {
            let key_x =
                shrimply_discrete_keyframe_graph_ui::key_x(point.time, width, domain, frame_step);
            let distance = (key_x - x).hypot(key_y - y);
            (distance <= HIT_RADIUS).then_some((distance, point))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, point)| point)
}

pub(super) fn hit_points(
    points: &[KeyframePoint],
    width: f64,
    height: f64,
    domain: GraphDomain,
    range: (f64, f64),
    x: f64,
    y: f64,
) -> Option<KeyframePoint> {
    points
        .iter()
        .copied()
        .filter_map(|point| {
            let (px, py) = raw_point(point, width, height, domain, range);
            let distance = (px - x).hypot(py - y);
            (distance <= HIT_RADIUS).then_some((distance, point))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, point)| point)
}

pub(super) fn update_graph_point(
    graph: &mut KeyframeGraph,
    old_time: Time,
    time: Time,
    value: f64,
) {
    match graph {
        KeyframeGraph::Step { points } => {
            if let Some(point) = points
                .iter_mut()
                .find(|point| point.time.approx_eq(old_time))
            {
                point.time = time;
            }
            points.sort_by_key(|point| point.time);
        }
        KeyframeGraph::RawValue {
            points, segments, ..
        } => {
            for point in &mut *points {
                if point.time.approx_eq(old_time) {
                    point.time = time;
                    point.value = value;
                }
            }
            for segment in &mut *segments {
                if segment.start.approx_eq(old_time) {
                    segment.start = time;
                    segment.start_value = value;
                }
                if segment.end.approx_eq(old_time) {
                    segment.end = time;
                    segment.end_value = value;
                }
            }
            points.sort_by_key(|point| point.time);
            segments.sort_by_key(|segment| segment.start);
        }
        KeyframeGraph::Speed { segments, keys, .. } => {
            for key in &mut *keys {
                if key.approx_eq(old_time) {
                    *key = time;
                }
            }
            for segment in &mut *segments {
                if segment.start.approx_eq(old_time) {
                    segment.start = time;
                }
                if segment.end.approx_eq(old_time) {
                    segment.end = time;
                }
            }
            keys.sort();
            segments.sort_by_key(|segment| segment.start);
        }
    }
}

pub(super) fn update_graph_interpolation(
    graph: &mut KeyframeGraph,
    owner_id: Uuid,
    interpolation: Interpolation,
) {
    match graph {
        KeyframeGraph::RawValue { segments, .. } => {
            if let Some(segment) = segments
                .iter_mut()
                .find(|segment| segment.owner_id == owner_id)
            {
                segment.interpolation = interpolation;
            }
        }
        KeyframeGraph::Speed { segments, .. } => {
            if let Some(segment) = segments
                .iter_mut()
                .find(|segment| segment.owner_id == owner_id)
            {
                segment.interpolation = interpolation;
            }
        }
        KeyframeGraph::Step { .. } => {}
    }
}

pub(super) fn raw_point(
    point: KeyframePoint,
    width: f64,
    height: f64,
    domain: GraphDomain,
    range: (f64, f64),
) -> (f64, f64) {
    (
        time_x(point.time, width, domain),
        value_y(point.value, height, range),
    )
}

pub(super) fn time_x(time: Time, width: f64, domain: GraphDomain) -> f64 {
    let (start, duration) = domain;
    let duration = duration
        .saturating_sub(start)
        .as_secs_f64()
        .max(f64::EPSILON);
    GRAPH_PAD + (time.as_secs_f64() - start.as_secs_f64()) / duration * (width - GRAPH_PAD * 2.0)
}

pub(super) fn time_at_x(x: f64, width: f64, domain: GraphDomain) -> Time {
    let (start, duration) = domain;
    let duration = duration
        .saturating_sub(start)
        .as_secs_f64()
        .max(f64::EPSILON);
    let progress = ((x - GRAPH_PAD) / (width - GRAPH_PAD * 2.0)).clamp(0.0, 1.0);
    Time::from_seconds_f64(start.as_secs_f64() + progress * duration)
}

pub(super) fn value_y(value: f64, height: f64, (min_value, max_value): (f64, f64)) -> f64 {
    let span = (max_value - min_value).max(1.0);
    height - GRAPH_PAD - (value - min_value) / span * (height - GRAPH_PAD * 2.0)
}

pub(super) fn value_at_y(y: f64, height: f64, (min_value, max_value): (f64, f64)) -> f64 {
    let span = (max_value - min_value).max(1.0);
    min_value + (height - GRAPH_PAD - y) / (height - GRAPH_PAD * 2.0) * span
}

pub(super) fn graph_domain((visible_start, visible_end): (Time, Time)) -> GraphDomain {
    let visible_end = visible_end.max(visible_start);
    let duration = visible_end.signed_sub(visible_start).as_secs_f64().max(1.0);
    let right_headroom = duration.mul_add(0.25, 0.0).max(1.0);
    (
        visible_start,
        visible_end.saturating_add(Time::from_seconds_f64(right_headroom)),
    )
}

pub(super) fn graph_range(graph: &KeyframeGraph) -> (f64, f64) {
    match graph {
        KeyframeGraph::Step { .. } => STEP_GRAPH_RANGE,
        KeyframeGraph::RawValue {
            points, segments, ..
        } => raw_range(points, segments),
        KeyframeGraph::Speed { segments, .. } => speed_range(segments),
    }
}

pub(super) fn graph_edit_value(graph: &KeyframeGraph, value: f64) -> f64 {
    match graph {
        KeyframeGraph::Step { .. } => value.clamp(0.0, 1.0),
        KeyframeGraph::RawValue { .. } => value,
        KeyframeGraph::Speed { .. } => value,
    }
}

pub(super) fn raw_range(points: &[KeyframePoint], segments: &[RawSegment]) -> (f64, f64) {
    let samples = segments.iter().flat_map(|segment| {
        curve_sample_progresses(segment.interpolation)
            .into_iter()
            .map(move |progress| {
                segment.start_value
                    + (segment.end_value - segment.start_value)
                        * segment.interpolation.value(progress)
            })
    });
    let min_value = points
        .iter()
        .map(|point| point.value)
        .chain(samples.clone())
        .fold(f64::INFINITY, f64::min);
    let max_value = points
        .iter()
        .map(|point| point.value)
        .chain(samples)
        .fold(f64::NEG_INFINITY, f64::max);
    if !min_value.is_finite() || !max_value.is_finite() {
        return (-1.0, 1.0);
    }
    if (max_value - min_value).abs() <= f64::EPSILON {
        let padding = min_value.abs().max(1.0) * 0.5;
        return (min_value - padding, max_value + padding);
    }
    let padding = (max_value - min_value) * 0.08;
    (min_value - padding, max_value + padding)
}

pub(super) fn speed_range(segments: &[SpeedSegment]) -> (f64, f64) {
    let mut minimum = 0.0_f64;
    let mut maximum = 0.0_f64;
    for speed in segments.iter().flat_map(|segment| {
        curve_sample_progresses(segment.interpolation)
            .into_iter()
            .filter_map(move |progress| segment_speed_at(segment, progress))
    }) {
        minimum = minimum.min(speed);
        maximum = maximum.max(speed);
    }
    if (maximum - minimum).abs() <= f64::EPSILON {
        return (-1.0, 1.0);
    }
    let padding = (maximum - minimum) * 0.08;
    (minimum - padding, maximum + padding)
}

pub(super) fn curve_sample_progresses(interpolation: Interpolation) -> Vec<f64> {
    let mut samples: Vec<_> = (0..=SPEED_CURVE_STEPS)
        .map(|step| step as f64 / SPEED_CURVE_STEPS as f64)
        .collect();
    for breakpoint in interpolation.derivative_breakpoints() {
        samples.extend([
            (breakpoint - CURVE_BREAK_OFFSET).max(0.0),
            *breakpoint,
            (breakpoint + CURVE_BREAK_OFFSET).min(1.0),
        ]);
    }
    samples.sort_by(f64::total_cmp);
    samples.dedup();
    samples
}

pub(super) fn segment_speed_at(segment: &SpeedSegment, progress: f64) -> Option<f64> {
    if segment.interpolation == Interpolation::Jump {
        return Some(0.0);
    }
    segment
        .interpolation
        .derivative(progress)
        .map(|derivative| segment.value * derivative)
}

pub(super) fn delete_graph_key(graph: &mut KeyframeGraph, time: Time) {
    match graph {
        KeyframeGraph::Step { points } => {
            points.retain(|point| !point.time.approx_eq(time));
        }
        KeyframeGraph::RawValue {
            points, segments, ..
        } => {
            points.retain(|point| !point.time.approx_eq(time));
            segments
                .retain(|segment| !segment.start.approx_eq(time) && !segment.end.approx_eq(time));
        }
        KeyframeGraph::Speed { segments, keys, .. } => {
            keys.retain(|key| !key.approx_eq(time));
            segments
                .retain(|segment| !segment.start.approx_eq(time) && !segment.end.approx_eq(time));
        }
    }
}

pub(super) fn previous_key_time(
    graph: &KeyframeGraph,
    playhead: Time,
    frame_step: Time,
) -> Option<Time> {
    keyframe_model::previous_key(&graph.key_times(), playhead, frame_step)
}

pub(super) fn next_key_time(
    graph: &KeyframeGraph,
    playhead: Time,
    frame_step: Time,
) -> Option<Time> {
    keyframe_model::next_key(&graph.key_times(), playhead, frame_step)
}
