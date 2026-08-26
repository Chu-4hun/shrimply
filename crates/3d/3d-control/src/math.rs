use glam::{Quat, Vec2, Vec3};
use shrimply_math_geometry::distance_to_segment;

use super::{
    Axis, CameraDrag, CameraEdit, Control, ControlInput, DragOptions, Edit, Gizmo, GizmoAxis,
    GizmoPlane, Handle, Plane, Projection, ROTATION_SEGMENTS, WorldCoordinateAxis,
    WorldCoordinateGizmo,
};

const PLANE_INNER_FRACTION: f32 = 0.28;
const PLANE_OUTER_FRACTION: f32 = 0.45;
const MIN_SCALE: f32 = 0.001;
const MIN_CAMERA_DISTANCE: f32 = 0.001;
const CAMERA_CLEARANCE_SCALE: f32 = 1.05;
const PROJECTION_DERIVATIVE_FRACTION: f32 = 0.001;

pub(super) fn control(input: ControlInput) -> Option<Control> {
    let camera_world =
        shrimply_transform_3d::camera_world(input.camera_position, input.camera_rotation_degrees);
    let camera_rotation = camera_world.to_scale_rotation_translation().1;
    let world_to_camera_rotation = camera_rotation.inverse();
    let camera_space = world_to_camera_rotation * (input.model.position - input.camera_position);
    let depth = -camera_space.z;
    if !camera_space.is_finite() {
        return None;
    }

    let canvas_height = input.canvas_size.y.max(1.0);
    let aspect = input.canvas_size.x.max(1.0) / canvas_height;
    let axis_world = Axis::ALL.map(Axis::vector);
    let (anchor, world_units_per_canvas_pixel, axis_canvas_per_world_unit) = match input.projection
    {
        Projection::Perspective => {
            if depth <= f32::EPSILON {
                return None;
            }
            let tangent = (input.vertical_fov_degrees.clamp(1.0, 179.0).to_radians() * 0.5).tan();
            let normalized_x = camera_space.x / (depth * tangent * aspect);
            let normalized_y = camera_space.y / (depth * tangent);
            let anchor = normalized_anchor(normalized_x, normalized_y, input.canvas_size);
            let focal = canvas_height / (2.0 * tangent);
            let axes = axis_world.map(|axis| {
                let camera_axis = world_to_camera_rotation * axis;
                Vec2::new(
                    focal * (camera_axis.x * depth + camera_space.x * camera_axis.z)
                        / depth.powi(2),
                    -focal * (camera_axis.y * depth + camera_space.y * camera_axis.z)
                        / depth.powi(2),
                )
            });
            (anchor, 2.0 * depth * tangent / canvas_height, axes)
        }
        Projection::Orthographic => {
            if depth <= f32::EPSILON {
                return None;
            }
            let height = input.orthographic_height.max(0.001);
            let world_units_per_canvas_pixel = height / canvas_height;
            let anchor = normalized_anchor(
                camera_space.x * 2.0 / (height * aspect),
                camera_space.y * 2.0 / height,
                input.canvas_size,
            );
            let axes = axis_world.map(|axis| {
                let camera_axis = world_to_camera_rotation * axis;
                Vec2::new(
                    camera_axis.x / world_units_per_canvas_pixel,
                    -camera_axis.y / world_units_per_canvas_pixel,
                )
            });
            (anchor, world_units_per_canvas_pixel, axes)
        }
        Projection::Equirectangular | Projection::Cylindrical | Projection::Fisheye => {
            let anchor = project_camera_point(&input, camera_space)?;
            let derivative_step = camera_space.length().max(1.0) * PROJECTION_DERIVATIVE_FRACTION;
            let axes = axis_world.map(|axis| {
                let endpoint = project_camera_point(
                    &input,
                    camera_space + world_to_camera_rotation * axis * derivative_step,
                )
                .unwrap_or(anchor);
                let mut delta = endpoint - anchor;
                if matches!(
                    input.projection,
                    Projection::Equirectangular | Projection::Cylindrical
                ) {
                    delta.x -= (delta.x / input.canvas_size.x.max(1.0)).round()
                        * input.canvas_size.x.max(1.0);
                }
                delta / derivative_step
            });
            let pixels_per_world_unit = axes.iter().map(|axis| axis.length()).fold(0.0, f32::max);
            if pixels_per_world_unit <= f32::EPSILON {
                return None;
            }
            (anchor, 1.0 / pixels_per_world_unit, axes)
        }
    };
    if !anchor.is_finite() || !world_units_per_canvas_pixel.is_finite() {
        return None;
    }

    Some(Control {
        anchor,
        model: input.model,
        camera_right: camera_rotation * Vec3::X,
        camera_up: camera_rotation * Vec3::Y,
        world_units_per_canvas_pixel,
        axis_world,
        axis_canvas_per_world_unit,
        camera_position: input.camera_position,
        camera_rotation_degrees: input.camera_rotation_degrees,
        minimum_camera_distance: MIN_CAMERA_DISTANCE,
        canvas_size: input.canvas_size,
    })
}

pub(super) fn camera_clearance(model_radius: f32) -> f32 {
    model_radius.max(MIN_CAMERA_DISTANCE) * CAMERA_CLEARANCE_SCALE
}

fn normalized_anchor(x: f32, y: f32, canvas_size: Vec2) -> Vec2 {
    Vec2::new(
        (x + 1.0) * canvas_size.x * 0.5,
        (1.0 - y) * canvas_size.y * 0.5,
    )
}

fn project_camera_point(input: &ControlInput, point: Vec3) -> Option<Vec2> {
    let distance = point.length();
    if distance <= f32::EPSILON {
        return None;
    }
    let canvas = input.canvas_size.max(Vec2::ONE);
    match input.projection {
        Projection::Equirectangular => {
            let longitude = point.x.atan2(-point.z);
            let latitude = (point.y / distance).clamp(-1.0, 1.0).asin();
            Some(Vec2::new(
                canvas.x * (0.5 + longitude / std::f32::consts::TAU),
                canvas.y * (0.5 - latitude / std::f32::consts::PI),
            ))
        }
        Projection::Cylindrical => {
            let radial_distance = Vec2::new(point.x, point.z).length();
            if radial_distance <= f32::EPSILON {
                return None;
            }
            let longitude = point.x.atan2(-point.z);
            let tangent = (input.vertical_fov_degrees.clamp(1.0, 179.0).to_radians() * 0.5).tan();
            Some(Vec2::new(
                canvas.x * (0.5 + longitude / std::f32::consts::TAU),
                canvas.y * (0.5 - 0.5 * point.y / (radial_distance * tangent)),
            ))
        }
        Projection::Fisheye => {
            let maximum_angle = input.vertical_fov_degrees.clamp(1.0, 360.0).to_radians() * 0.5;
            let angle = (-point.z / distance).clamp(-1.0, 1.0).acos();
            if angle > maximum_angle {
                return None;
            }
            let radial = Vec2::new(point.x, point.y);
            let radial_distance = radial.length();
            let direction = if radial_distance > f32::EPSILON {
                radial / radial_distance
            } else {
                Vec2::ZERO
            };
            let pixel_radius = angle / maximum_angle * canvas.min_element() * 0.5;
            Some(canvas * 0.5 + Vec2::new(direction.x, -direction.y) * pixel_radius)
        }
        Projection::Perspective | Projection::Orthographic => None,
    }
}

pub(super) fn drag_camera(control: Control, kind: CameraDrag, delta: Vec2) -> Option<CameraEdit> {
    if !delta.is_finite() {
        return None;
    }
    let current_rotation = shrimply_transform_3d::rotation(
        control.camera_rotation_degrees,
        shrimply_transform_3d::RotationOrder::Xyz,
    );
    match kind {
        CameraDrag::Orbit => {
            let radians_per_canvas_pixel =
                std::f32::consts::PI / control.canvas_size.min_element().max(1.0);
            let yaw = -delta.x * radians_per_canvas_pixel;
            let pitch = -delta.y * radians_per_canvas_pixel;
            let orbit = Quat::from_rotation_y(yaw)
                * Quat::from_axis_angle(current_rotation * Vec3::X, pitch);
            let rotation = orbit * current_rotation;
            Some(CameraEdit {
                position: control.model.position
                    + orbit * (control.camera_position - control.model.position),
                rotation_degrees: shrimply_transform_3d::rotation_degrees(
                    rotation,
                    shrimply_transform_3d::RotationOrder::Xyz,
                ),
            })
        }
        CameraDrag::Pan => Some(CameraEdit {
            position: control.camera_position
                - control.camera_right * delta.x * control.world_units_per_canvas_pixel
                + control.camera_up * delta.y * control.world_units_per_canvas_pixel,
            rotation_degrees: control.camera_rotation_degrees,
        }),
        CameraDrag::Dolly => {
            let forward = current_rotation * Vec3::NEG_Z;
            let mut distance = -delta.y * control.world_units_per_canvas_pixel;
            let maximum_closer = (control.model.position - control.camera_position).dot(forward)
                - control.minimum_camera_distance;
            if distance > 0.0 {
                distance = distance.min(maximum_closer.max(0.0));
            }
            Some(CameraEdit {
                position: control.camera_position + forward * distance,
                rotation_degrees: control.camera_rotation_degrees,
            })
        }
    }
}

pub(super) fn gizmo(
    control: Control,
    screen_pixels_per_canvas_pixel: f32,
    arrow_length: f32,
    rotation_radius: f32,
    scale_radius: f32,
) -> Gizmo {
    let screen_axes = control
        .axis_canvas_per_world_unit
        .map(|axis| axis * screen_pixels_per_canvas_pixel);
    let maximum_axis = screen_axes
        .iter()
        .map(|axis| axis.length())
        .fold(0.0, f32::max)
        .max(f32::EPSILON);
    let arrow_world_scale = arrow_length / maximum_axis;
    let arrows = screen_axes.map(|axis| axis * arrow_world_scale);
    let axes = Axis::ALL.map(|axis| {
        let (first, second) = rotation_plane_axes(axis);
        let first = screen_axes[first.index()];
        let second = screen_axes[second.index()];
        let mut rotation = (0..=ROTATION_SEGMENTS)
            .map(|index| {
                let angle = index as f32 / ROTATION_SEGMENTS as f32 * std::f32::consts::TAU;
                first * angle.cos() + second * angle.sin()
            })
            .collect::<Vec<_>>();
        let projected_radius = rotation
            .iter()
            .map(|point| point.length())
            .fold(0.0, f32::max)
            .max(f32::EPSILON);
        for point in &mut rotation {
            *point *= rotation_radius / projected_radius;
        }
        GizmoAxis {
            axis,
            arrow: arrows[axis.index()],
            rotation,
        }
    });
    let planes = Plane::ALL.map(|plane| {
        let (first, second) = plane.axes();
        let first = arrows[first.index()];
        let second = arrows[second.index()];
        GizmoPlane {
            plane,
            corners: [
                first * PLANE_INNER_FRACTION + second * PLANE_INNER_FRACTION,
                first * PLANE_OUTER_FRACTION + second * PLANE_INNER_FRACTION,
                first * PLANE_OUTER_FRACTION + second * PLANE_OUTER_FRACTION,
                first * PLANE_INNER_FRACTION + second * PLANE_OUTER_FRACTION,
            ],
        }
    });
    Gizmo {
        axes,
        planes,
        scale_radius,
    }
}

pub(super) fn world_coordinate_gizmo(control: Control, axis_length: f32) -> WorldCoordinateGizmo {
    let camera_backward = control.camera_right.cross(control.camera_up);
    let projected = Axis::ALL.map(|axis| {
        Vec2::new(
            axis.vector().dot(control.camera_right),
            -axis.vector().dot(control.camera_up),
        )
    });
    let scale = axis_length
        / projected
            .iter()
            .map(|axis| axis.length())
            .fold(0.0, f32::max)
            .max(f32::EPSILON);
    WorldCoordinateGizmo {
        axes: Axis::ALL.map(|axis| WorldCoordinateAxis {
            axis,
            endpoint: projected[axis.index()] * scale,
            depth: axis.vector().dot(camera_backward),
        }),
    }
}

pub(super) fn hit(
    gizmo: &Gizmo,
    point: Vec2,
    center_radius: f32,
    hit_width: f32,
) -> Option<Handle> {
    if (point.length() - gizmo.scale_radius).abs() <= hit_width {
        return Some(Handle::Scale);
    }
    hit_position(gizmo, point, center_radius, hit_width)
        .or_else(|| hit_rotation(gizmo, point, hit_width))
}

pub(super) fn hit_position(
    gizmo: &Gizmo,
    point: Vec2,
    center_radius: f32,
    hit_width: f32,
) -> Option<Handle> {
    if point.length_squared() <= center_radius * center_radius {
        return Some(Handle::Position);
    }
    if let Some(plane) = gizmo
        .planes
        .iter()
        .find(|plane| point_in_convex_polygon(point, &plane.corners))
    {
        return Some(Handle::PositionPlane(plane.plane));
    }
    if let Some(axis) = gizmo
        .axes
        .iter()
        .filter(|axis| axis.arrow.length_squared() > f32::EPSILON)
        .find(|axis| distance_to_segment(point, Vec2::ZERO, axis.arrow) <= hit_width)
    {
        return Some(Handle::PositionAxis(axis.axis));
    }
    None
}

pub(super) fn hit_rotation(gizmo: &Gizmo, point: Vec2, hit_width: f32) -> Option<Handle> {
    gizmo.axes.iter().find_map(|axis| {
        axis.rotation
            .windows(2)
            .any(|points| distance_to_segment(point, points[0], points[1]) <= hit_width)
            .then_some(Handle::Rotation(axis.axis))
    })
}

pub(super) fn drag(
    control: Control,
    handle: Handle,
    mut delta: Vec2,
    start_from_anchor: Vec2,
    options: DragOptions,
) -> Option<Edit> {
    if !delta.is_finite() || !start_from_anchor.is_finite() {
        return None;
    }
    match handle {
        Handle::Position => {
            if options.constrain_axis {
                if delta.x.abs() >= delta.y.abs() {
                    delta.y = 0.0;
                } else {
                    delta.x = 0.0;
                }
            }
            Some(Edit::Position(
                control.model.position
                    + control.camera_right * delta.x * control.world_units_per_canvas_pixel
                    - control.camera_up * delta.y * control.world_units_per_canvas_pixel,
            ))
        }
        Handle::PositionAxis(axis) => {
            let projected = control.axis_canvas_per_world_unit[axis.index()];
            let amount = delta.dot(projected) / projected.length_squared();
            amount.is_finite().then_some(Edit::Position(
                control.model.position + control.axis_world[axis.index()] * amount,
            ))
        }
        Handle::PositionPlane(plane) => {
            let (first, second) = plane.axes();
            let first_projected = control.axis_canvas_per_world_unit[first.index()];
            let second_projected = control.axis_canvas_per_world_unit[second.index()];
            let determinant = first_projected.perp_dot(second_projected);
            if determinant.abs() <= f32::EPSILON {
                return None;
            }
            let first_amount = delta.perp_dot(second_projected) / determinant;
            let second_amount = first_projected.perp_dot(delta) / determinant;
            Some(Edit::Position(
                control.model.position
                    + control.axis_world[first.index()] * first_amount
                    + control.axis_world[second.index()] * second_amount,
            ))
        }
        Handle::Rotation(axis) => {
            let current = start_from_anchor + delta;
            if start_from_anchor.length_squared() <= f32::EPSILON
                || current.length_squared() <= f32::EPSILON
            {
                return None;
            }
            let start_angle = rotation_angle(control, axis, start_from_anchor)?;
            let current_angle = rotation_angle(control, axis, current)?;
            let mut degrees = (current_angle - start_angle).to_degrees();
            if degrees > 180.0 {
                degrees -= 360.0;
            } else if degrees < -180.0 {
                degrees += 360.0;
            }
            let current_rotation = shrimply_transform_3d::rotation(
                control.model.rotation_degrees,
                control.model.rotation_order,
            );
            let rotation =
                Quat::from_axis_angle(axis.vector(), degrees.to_radians()) * current_rotation;
            Some(Edit::Rotation(shrimply_transform_3d::rotation_degrees(
                rotation,
                control.model.rotation_order,
            )))
        }
        Handle::Scale => {
            let start_radius = start_from_anchor.length();
            if start_radius <= f32::EPSILON {
                return None;
            }
            let factor = (start_from_anchor + delta).length() / start_radius;
            factor.is_finite().then_some(Edit::Scale(
                (control.model.scale * factor).max(Vec3::splat(MIN_SCALE)),
            ))
        }
    }
}

fn rotation_plane_axes(axis: Axis) -> (Axis, Axis) {
    match axis {
        Axis::X => (Axis::Y, Axis::Z),
        Axis::Y => (Axis::Z, Axis::X),
        Axis::Z => (Axis::X, Axis::Y),
    }
}

fn rotation_angle(control: Control, axis: Axis, point: Vec2) -> Option<f32> {
    let (first, second) = rotation_plane_axes(axis);
    let first = control.axis_canvas_per_world_unit[first.index()];
    let second = control.axis_canvas_per_world_unit[second.index()];
    let determinant = first.perp_dot(second);
    if determinant.abs() <= f32::EPSILON {
        return None;
    }
    let first_amount = point.perp_dot(second) / determinant;
    let second_amount = first.perp_dot(point) / determinant;
    Some(second_amount.atan2(first_amount))
}

pub(super) fn point_in_direction(points: &[Vec2], direction: Vec2) -> Option<Vec2> {
    let direction = direction.try_normalize()?;
    points.iter().copied().max_by(|first, second| {
        first
            .normalize_or_zero()
            .dot(direction)
            .total_cmp(&second.normalize_or_zero().dot(direction))
    })
}

fn point_in_convex_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut sign = 0.0;
    for index in 0..polygon.len() {
        let cross = (polygon[(index + 1) % polygon.len()] - polygon[index])
            .perp_dot(point - polygon[index]);
        if cross.abs() <= f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    sign != 0.0
}
