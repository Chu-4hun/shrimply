mod math;
mod mesh;

use glam::Vec3;
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use shrimply_core::{
    Color,
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use shrimply_scene_3d::{ObjMesh, PbrMaterial, Transform3d};
use uuid::Uuid;

pub const MIN_SMOOTHNESS: f32 = 1.0;
pub const MAX_SMOOTHNESS: f32 = 12.0;
pub const DEFAULT_SMOOTHNESS: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape3dKind {
    #[default]
    Box,
    Disk,
    Triangle,
    Star,
    Arrow,
    Diamond,
    Pentagon,
    Hexagon,
    Heart,
    Octagon,
    Cross,
    Sphere,
    Cone,
    Torus,
    Capsule,
}

impl Shape3dKind {
    pub const fn is_extruded(self) -> bool {
        matches!(
            self,
            Self::Box
                | Self::Disk
                | Self::Triangle
                | Self::Star
                | Self::Arrow
                | Self::Diamond
                | Self::Pentagon
                | Self::Hexagon
                | Self::Heart
                | Self::Octagon
                | Self::Cross
        )
    }

    pub const fn has_profile_corners(self) -> bool {
        matches!(
            self,
            Self::Box
                | Self::Triangle
                | Self::Star
                | Self::Arrow
                | Self::Diamond
                | Self::Pentagon
                | Self::Hexagon
                | Self::Octagon
                | Self::Cross
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape3dRoundingStrategy {
    #[default]
    Continuous,
    Circular,
    Chamfer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape3dModifier {
    pub shape: Shape3dKind,
    pub size: TimelineValue<Vec3>,
    pub corner_radius: TimelineValue<f32>,
    pub rounding_strategy: Shape3dRoundingStrategy,
    pub edge_roundness: TimelineValue<f32>,
    pub smoothness: TimelineValue<f32>,
    pub star_points: TimelineValue<f32>,
    pub star_inner_radius_percent: TimelineValue<f32>,
    pub arrow_shaft_width_percent: TimelineValue<f32>,
    pub arrow_head_length_percent: TimelineValue<f32>,
    pub cross_arm_thickness_percent: TimelineValue<f32>,
    pub disk_inner_radius_percent: TimelineValue<f32>,
    pub disk_completion_degrees: TimelineValue<f32>,
    pub torus_inner_radius_percent: TimelineValue<f32>,
    pub transform: Transform3d,
    pub material: PbrMaterial,
}

impl Default for Shape3dModifier {
    fn default() -> Self {
        Self {
            shape: Shape3dKind::Box,
            size: TimelineValue::new_const(Vec3::ONE),
            corner_radius: TimelineValue::new_const(0.0),
            rounding_strategy: Shape3dRoundingStrategy::Continuous,
            edge_roundness: TimelineValue::new_const(0.0),
            smoothness: TimelineValue::new_const(DEFAULT_SMOOTHNESS),
            star_points: TimelineValue::new_const(5.0),
            star_inner_radius_percent: TimelineValue::new_const(0.4),
            arrow_shaft_width_percent: TimelineValue::new_const(0.4),
            arrow_head_length_percent: TimelineValue::new_const(0.4),
            cross_arm_thickness_percent: TimelineValue::new_const(0.35),
            disk_inner_radius_percent: TimelineValue::new_const(0.0),
            disk_completion_degrees: TimelineValue::new_const(360.0),
            torus_inner_radius_percent: TimelineValue::new_const(0.5),
            transform: Transform3d::default(),
            material: PbrMaterial::default(),
        }
    }
}

impl ModifierModel for Shape3dModifier {
    fn display_name(&self) -> &'static str {
        "3D Shape"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["primitive", "geometry", "mesh"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        for value in self.numbers_mut() {
            ensure_timeline_value_ids(value, seen);
        }
        for value in self.number3s_mut() {
            ensure_timeline_value_ids(value, seen);
        }
        ensure_timeline_value_ids(&mut self.transform.rotation_order, seen);
        for value in shrimply_scene_3d::material_colors_mut(&mut self.material) {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine(
            self.numbers()
                .into_iter()
                .map(timeline_value_span)
                .chain(self.number3s().into_iter().map(timeline_value_span))
                .chain(std::iter::once(timeline_value_span(
                    &self.transform.rotation_order,
                )))
                .chain(
                    shrimply_scene_3d::material_colors(&self.material)
                        .into_iter()
                        .map(timeline_value_span),
                ),
        )
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        self.numbers().into_iter().find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        self.numbers_mut().into_iter().find(|value| value.id == id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<Vec3>> {
        self.number3s().into_iter().find(|value| value.id == id)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Vec3>> {
        self.number3s_mut().into_iter().find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        shrimply_scene_3d::material_colors_mut(&mut self.material)
            .into_iter()
            .find(|value| value.id == id)
    }
}

impl Shape3dModifier {
    fn numbers(&self) -> Vec<&TimelineValue<f32>> {
        vec![
            &self.corner_radius,
            &self.edge_roundness,
            &self.smoothness,
            &self.star_points,
            &self.star_inner_radius_percent,
            &self.arrow_shaft_width_percent,
            &self.arrow_head_length_percent,
            &self.cross_arm_thickness_percent,
            &self.disk_inner_radius_percent,
            &self.disk_completion_degrees,
            &self.torus_inner_radius_percent,
        ]
        .into_iter()
        .chain(shrimply_scene_3d::material_numbers(&self.material))
        .collect()
    }

    fn numbers_mut(&mut self) -> Vec<&mut TimelineValue<f32>> {
        vec![
            &mut self.corner_radius,
            &mut self.edge_roundness,
            &mut self.smoothness,
            &mut self.star_points,
            &mut self.star_inner_radius_percent,
            &mut self.arrow_shaft_width_percent,
            &mut self.arrow_head_length_percent,
            &mut self.cross_arm_thickness_percent,
            &mut self.disk_inner_radius_percent,
            &mut self.disk_completion_degrees,
            &mut self.torus_inner_radius_percent,
        ]
        .into_iter()
        .chain(shrimply_scene_3d::material_numbers_mut(&mut self.material))
        .collect()
    }

    fn number3s(&self) -> Vec<&TimelineValue<Vec3>> {
        vec![
            &self.size,
            &self.transform.position,
            &self.transform.anchor,
            &self.transform.rotation_degrees,
            &self.transform.scale,
        ]
    }

    fn number3s_mut(&mut self) -> Vec<&mut TimelineValue<Vec3>> {
        vec![
            &mut self.size,
            &mut self.transform.position,
            &mut self.transform.anchor,
            &mut self.transform.rotation_degrees,
            &mut self.transform.scale,
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub shape: Shape3dKind,
    pub size: Vec3,
    pub corner_radius: f32,
    pub rounding_strategy: Shape3dRoundingStrategy,
    pub edge_roundness: f32,
    pub smoothness: f32,
    pub star_points: f32,
    pub star_inner_radius_percent: f32,
    pub arrow_shaft_width_percent: f32,
    pub arrow_head_length_percent: f32,
    pub cross_arm_thickness_percent: f32,
    pub disk_inner_radius_percent: f32,
    pub disk_completion_degrees: f32,
    pub torus_inner_radius_percent: f32,
}

#[derive(Debug)]
pub struct Shape3dError(String);

impl std::fmt::Display for Shape3dError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Shape3dError {}

pub fn generate_mesh(geometry: Geometry) -> Result<ObjMesh, Shape3dError> {
    if !geometry.size.is_finite()
        || [
            geometry.corner_radius,
            geometry.edge_roundness,
            geometry.smoothness,
            geometry.star_points,
            geometry.star_inner_radius_percent,
            geometry.arrow_shaft_width_percent,
            geometry.arrow_head_length_percent,
            geometry.cross_arm_thickness_percent,
            geometry.disk_inner_radius_percent,
            geometry.disk_completion_degrees,
            geometry.torus_inner_radius_percent,
        ]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(Shape3dError("3D shape geometry is not finite".to_string()));
    }
    let size = geometry.size.abs().max(Vec3::splat(f32::EPSILON));
    let smoothness = geometry
        .smoothness
        .round()
        .clamp(MIN_SMOOTHNESS, MAX_SMOOTHNESS) as u32;
    match geometry.shape {
        Shape3dKind::Sphere => mesh::sphere(size, smoothness),
        Shape3dKind::Cone => mesh::cone(size, geometry.edge_roundness, smoothness),
        Shape3dKind::Torus => mesh::torus(
            size,
            geometry.torus_inner_radius_percent.clamp(0.05, 0.95),
            smoothness,
        ),
        Shape3dKind::Capsule => mesh::capsule(size, smoothness),
        _ => mesh::extrude(
            math::profile(&geometry, size.truncate(), smoothness),
            size.z,
            geometry.edge_roundness.max(0.0),
            smoothness,
        ),
    }
}
