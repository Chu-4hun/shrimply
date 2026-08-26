use glam::Vec2;

pub(super) fn decorated_bounds(
    bounds: shrimply_preview_core::Rect,
    outline_width: f32,
    shadow_offset: Vec2,
    shadow_width: f32,
    shadow_blur: f32,
    padding: Vec2,
) -> shrimply_preview_core::Rect {
    let footprint = shadow_width.max(0.0) + shadow_blur.max(0.0);
    bounds
        .expand(outline_width.max(0.0))
        .union(bounds.expand(footprint).translated(shadow_offset))
        .union(bounds.outset(padding.max(Vec2::ZERO)))
}
