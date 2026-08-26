mod math;

use glam::{Vec2, Vec3};
pub use shrimply_transform_3d::{Projection, ResolvedTransform3D};

const ROTATION_SEGMENTS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct ControlInput {
    pub model: ResolvedTransform3D,
    pub camera_position: Vec3,
    pub camera_rotation_degrees: Vec3,
    pub projection: Projection,
    pub vertical_fov_degrees: f32,
    pub orthographic_height: f32,
    pub canvas_size: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct Control {
    anchor: Vec2,
    model: ResolvedTransform3D,
    camera_right: Vec3,
    camera_up: Vec3,
    world_units_per_canvas_pixel: f32,
    axis_world: [Vec3; 3],
    axis_canvas_per_world_unit: [Vec2; 3],
    camera_position: Vec3,
    camera_rotation_degrees: Vec3,
    minimum_camera_distance: f32,
    canvas_size: Vec2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    pub fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Plane {
    Xy,
    Yz,
    Zx,
}

impl Plane {
    pub const ALL: [Self; 3] = [Self::Xy, Self::Yz, Self::Zx];

    pub fn axes(self) -> (Axis, Axis) {
        match self {
            Self::Xy => (Axis::X, Axis::Y),
            Self::Yz => (Axis::Y, Axis::Z),
            Self::Zx => (Axis::Z, Axis::X),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Handle {
    Position,
    PositionAxis(Axis),
    PositionPlane(Plane),
    Rotation(Axis),
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Position(Vec3),
    Rotation(Vec3),
    Scale(Vec3),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraDrag {
    Orbit,
    Pan,
    Dolly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraEdit {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DragOptions {
    pub constrain_axis: bool,
}

pub struct Gizmo {
    pub axes: [GizmoAxis; 3],
    pub planes: [GizmoPlane; 3],
    pub scale_radius: f32,
}

pub struct GizmoAxis {
    pub axis: Axis,
    pub arrow: Vec2,
    pub rotation: Vec<Vec2>,
}

impl GizmoAxis {
    pub fn point_in_direction(&self, direction: Vec2) -> Option<Vec2> {
        math::point_in_direction(&self.rotation, direction)
    }
}

pub struct GizmoPlane {
    pub plane: Plane,
    pub corners: [Vec2; 4],
}

pub struct WorldCoordinateGizmo {
    pub axes: [WorldCoordinateAxis; 3],
}

pub struct WorldCoordinateAxis {
    pub axis: Axis,
    pub endpoint: Vec2,
    pub depth: f32,
}

impl Control {
    pub fn new(input: ControlInput) -> Option<Self> {
        math::control(input)
    }

    pub fn anchor(&self) -> Vec2 {
        self.anchor
    }

    pub fn with_canvas_anchor(mut self, anchor: Vec2) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn keep_camera_outside(mut self, model_radius: f32) -> Self {
        self.minimum_camera_distance = math::camera_clearance(model_radius);
        self
    }

    pub fn gizmo(
        &self,
        screen_pixels_per_canvas_pixel: f32,
        arrow_length: f32,
        rotation_radius: f32,
        scale_radius: f32,
    ) -> Gizmo {
        math::gizmo(
            *self,
            screen_pixels_per_canvas_pixel,
            arrow_length,
            rotation_radius,
            scale_radius,
        )
    }

    pub fn world_coordinate_gizmo(&self, axis_length: f32) -> WorldCoordinateGizmo {
        math::world_coordinate_gizmo(*self, axis_length)
    }

    pub fn drag(
        &self,
        handle: Handle,
        delta: Vec2,
        start_from_anchor: Vec2,
        options: DragOptions,
    ) -> Option<Edit> {
        math::drag(*self, handle, delta, start_from_anchor, options)
    }

    pub fn drag_camera(&self, kind: CameraDrag, delta: Vec2) -> Option<CameraEdit> {
        math::drag_camera(*self, kind, delta)
    }
}

impl Gizmo {
    pub fn hit(&self, point: Vec2, center_radius: f32, hit_width: f32) -> Option<Handle> {
        math::hit(self, point, center_radius, hit_width)
    }

    pub fn hit_position(&self, point: Vec2, center_radius: f32, hit_width: f32) -> Option<Handle> {
        math::hit_position(self, point, center_radius, hit_width)
    }

    pub fn hit_rotation(&self, point: Vec2, hit_width: f32) -> Option<Handle> {
        math::hit_rotation(self, point, hit_width)
    }
}
