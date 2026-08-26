use super::*;

pub(super) fn validate_item(item: &VisualItem) -> Result<(), String> {
    let VisualSource::Paint(paint) = &item.content else {
        return Ok(());
    };
    if paint.palette.is_empty() {
        return Err("paint palette must contain at least one color".into());
    }
    let drawings: Vec<_> = match &paint.drawing.base {
        TimelineBase::Const(drawing) => vec![drawing],
        TimelineBase::Keyframes(keyframes) => {
            keyframes.iter().map(|keyframe| &keyframe.value).collect()
        }
    };
    for drawing in drawings {
        for stroke in &drawing.strokes {
            if !stroke.width_scale.is_finite() || stroke.width_scale < 0.0 {
                return Err("stroke width scale must be finite and nonnegative".into());
            }
            if stroke.color_index >= paint.palette.len() {
                return Err("stroke palette index is out of bounds".into());
            }
            for point in &stroke.points {
                if !point.position.is_finite() {
                    return Err("stroke point must be finite".into());
                }
                if point.pressure.is_some_and(|pressure| {
                    !pressure.is_finite() || !(0.0..=1.0).contains(&pressure)
                }) {
                    return Err("stroke pressure must be between 0 and 1".into());
                }
            }
        }
        if drawing.fills.iter().any(|fill| !fill.seed.is_finite()) {
            return Err("fill seed must be finite".into());
        }
        if drawing
            .fills
            .iter()
            .any(|fill| fill.color_index >= paint.palette.len())
        {
            return Err("fill palette index is out of bounds".into());
        }
        if drawing.fills.iter().any(|fill| {
            fill.loops.iter().any(|boundary| {
                boundary.len() < 3 || boundary.iter().any(|point| !point.is_finite())
            })
        }) {
            return Err("fill boundaries must contain at least three finite points".into());
        }
    }
    for (name, value) in [
        ("stroke width", &paint.stroke.width),
        ("stroke thinning", &paint.stroke.thinning),
        ("stroke smoothing", &paint.stroke.smoothing),
        ("stroke streamline", &paint.stroke.streamline),
        (
            "simplification tolerance",
            &paint.stroke.simplification_tolerance,
        ),
        (
            "maximum subdivision spacing",
            &paint.stroke.maximum_subdivision_spacing,
        ),
        ("fill closure tolerance", &paint.fill.closure_tolerance),
    ] {
        if !scalar_values_are(value, |value| value.is_finite() && value >= 0.0) {
            return Err(format!("{name} must be finite and nonnegative"));
        }
    }
    for (name, distance) in [
        ("start taper distance", &paint.stroke.start.taper_distance),
        ("end taper distance", &paint.stroke.end.taper_distance),
    ] {
        if !scalar_values_are(distance, |value| value.is_finite() && value >= 0.0) {
            return Err(format!("{name} must be finite and nonnegative"));
        }
    }
    for texture in paint
        .palette
        .iter()
        .filter_map(|entry| entry.texture.as_ref())
    {
        if texture.image_path.as_os_str().is_empty() {
            return Err("texture image path must not be empty".into());
        }
        if !scalar_values_are(&texture.repeat_scale, |value| {
            value.is_finite() && value > 0.0
        }) {
            return Err("texture repeat scale must be finite and positive".into());
        }
        if !scalar_values_are(&texture.rotation_degrees, f32::is_finite) {
            return Err("texture rotation must be finite".into());
        }
    }
    Ok(())
}

pub(super) fn keyframe_span(paint: &PaintItem) -> Option<(Time, Time)> {
    keyframe_span_from_iter(
        [
            timeline_value_keyframe_span(&paint.drawing),
            timeline_value_keyframe_span(&paint.stroke.width),
            timeline_value_keyframe_span(&paint.stroke.thinning),
            timeline_value_keyframe_span(&paint.stroke.smoothing),
            timeline_value_keyframe_span(&paint.stroke.streamline),
            timeline_value_keyframe_span(&paint.stroke.simplification_tolerance),
            timeline_value_keyframe_span(&paint.stroke.maximum_subdivision_spacing),
            timeline_value_keyframe_span(&paint.stroke.start.cap),
            timeline_value_keyframe_span(&paint.stroke.start.taper),
            timeline_value_keyframe_span(&paint.stroke.start.taper_distance),
            timeline_value_keyframe_span(&paint.stroke.end.cap),
            timeline_value_keyframe_span(&paint.stroke.end.taper),
            timeline_value_keyframe_span(&paint.stroke.end.taper_distance),
            timeline_value_keyframe_span(&paint.fill.closure_tolerance),
            timeline_value_keyframe_span(&paint.stroke_transform.position),
            timeline_value_keyframe_span(&paint.stroke_transform.anchor),
            timeline_value_keyframe_span(&paint.stroke_transform.scale),
            timeline_value_keyframe_span(&paint.stroke_transform.rotation_degrees),
        ]
        .into_iter()
        .chain(paint.palette.iter().flat_map(|entry| {
            [
                timeline_value_keyframe_span(&entry.color),
                entry
                    .texture
                    .as_ref()
                    .and_then(|texture| timeline_value_keyframe_span(&texture.repeat_scale)),
                entry
                    .texture
                    .as_ref()
                    .and_then(|texture| timeline_value_keyframe_span(&texture.rotation_degrees)),
            ]
        })),
    )
}

pub(super) fn ensure_drawing_ids(drawing: &mut PaintDrawing, seen: &mut HashSet<Uuid>) {
    for stroke in &mut drawing.strokes {
        ensure_unique_id(&mut stroke.id, seen);
    }
    for fill in &mut drawing.fills {
        ensure_unique_id(&mut fill.id, seen);
    }
}

fn scalar_values_are(value: &TimelineValue<f32>, predicate: impl Fn(f32) -> bool) -> bool {
    match &value.base {
        TimelineBase::Const(value) => predicate(*value),
        TimelineBase::Keyframes(keyframes) => {
            if keyframes.is_empty() {
                predicate(f32::default())
            } else {
                keyframes
                    .iter()
                    .all(|keyframe| predicate(*keyframe.value()))
            }
        }
    }
}
