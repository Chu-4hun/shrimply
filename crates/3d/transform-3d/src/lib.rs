mod math;

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use math::{
    MAX_EXPOSURE_EV, MAX_F_STOP, MIN_EXPOSURE_EV, MIN_F_STOP, focal_length_mm, vertical_fov_degrees,
};

pub trait Vector3Value {
    fn constant(value: Vec3) -> Self;
    fn fallback(&self) -> Vec3;
}

pub trait RotationOrderValue {
    fn constant(value: RotationOrder) -> Self;
    fn fallback(&self) -> RotationOrder;
}

pub trait ScalarValue {
    fn constant(value: f32) -> Self;
    fn fallback(&self) -> f32;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraSource {
    #[default]
    Custom,
    #[serde(alias = "colmap")]
    Tracking(TrackingCameraSource),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackingCameraSource {
    pub track_id: Uuid,
    #[serde(default)]
    pub settings: TrackingSettings,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackingSettings {
    #[serde(default = "default_tracking_model")]
    pub model: String,
    #[serde(default)]
    pub quality: ColmapQuality,
    #[serde(default = "default_analysis_fps")]
    pub analysis_fps: u32,
    #[serde(default)]
    pub camera_model: ColmapCameraModel,
}

impl Default for TrackingSettings {
    fn default() -> Self {
        Self {
            model: default_tracking_model(),
            quality: ColmapQuality::Medium,
            analysis_fps: default_analysis_fps(),
            camera_model: ColmapCameraModel::SimpleRadial,
        }
    }
}

pub const COLMAP_TRACKING_MODEL: &str = "colmap/colmap";
pub const VGGT_SLAM_TRACKING_MODEL: &str = "MIT-SPARK/VGGT-SLAM";

fn default_tracking_model() -> String {
    COLMAP_TRACKING_MODEL.to_string()
}

const fn default_analysis_fps() -> u32 {
    10
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum ColmapQuality {
    Low,
    #[default]
    Medium,
    High,
    Extreme,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum ColmapCameraModel {
    #[default]
    #[strum(to_string = "Simple Radial")]
    SimpleRadial,
    Pinhole,
    #[strum(to_string = "OpenCV")]
    OpenCv,
    #[strum(to_string = "OpenCV Fisheye")]
    OpenCvFisheye,
    Equirectangular,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic,
    Equirectangular,
    Cylindrical,
    Fisheye,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationOrder {
    #[default]
    Xyz,
    Xzy,
    Yxz,
    Yzx,
    Zxy,
    Zyx,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transform3D<V: Default, R: Default> {
    pub position: V,
    #[serde(default)]
    pub anchor: V,
    pub rotation_degrees: V,
    #[serde(default)]
    pub rotation_order: R,
    pub scale: V,
}

impl<V: Vector3Value + Default, R: RotationOrderValue + Default> Default for Transform3D<V, R> {
    fn default() -> Self {
        Self {
            position: V::constant(Vec3::ZERO),
            anchor: V::constant(Vec3::ZERO),
            rotation_degrees: V::constant(Vec3::ZERO),
            rotation_order: R::constant(RotationOrder::Xyz),
            scale: V::constant(Vec3::ONE),
        }
    }
}

impl<V: Vector3Value + Default, R: RotationOrderValue + Default> Transform3D<V, R> {
    pub fn fallback(&self) -> ResolvedTransform3D {
        ResolvedTransform3D {
            position: self.position.fallback(),
            anchor: self.anchor.fallback(),
            rotation_degrees: self.rotation_degrees.fallback(),
            rotation_order: self.rotation_order.fallback(),
            scale: self.scale.fallback(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera3D<V: Default, S: ScalarValue> {
    #[serde(default)]
    pub source: CameraSource,
    pub projection: Projection,
    pub position: V,
    pub rotation_degrees: V,
    pub vertical_fov_degrees: S,
    pub orthographic_height: S,
    #[serde(default = "default_focus_distance")]
    pub focus_distance: S,
    #[serde(default = "default_f_stop")]
    pub f_stop: S,
    pub exposure_ev: S,
}

fn default_focus_distance<S: ScalarValue>() -> S {
    S::constant(0.0)
}

fn default_f_stop<S: ScalarValue>() -> S {
    S::constant(2.8)
}

impl<V: Vector3Value + Default, S: ScalarValue> Default for Camera3D<V, S> {
    fn default() -> Self {
        let vertical_fov_degrees = 50.0f32;
        let distance = 1.1 / (vertical_fov_degrees * 0.5).to_radians().sin();
        Self {
            source: CameraSource::Custom,
            projection: Projection::Perspective,
            position: V::constant(Vec3::new(0.0, 0.0, distance)),
            rotation_degrees: V::constant(Vec3::ZERO),
            vertical_fov_degrees: S::constant(vertical_fov_degrees),
            orthographic_height: S::constant(2.2),
            focus_distance: default_focus_distance(),
            f_stop: default_f_stop(),
            exposure_ev: S::constant(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedTransform3D {
    pub position: Vec3,
    pub anchor: Vec3,
    pub rotation_degrees: Vec3,
    pub rotation_order: RotationOrder,
    pub scale: Vec3,
}

impl ResolvedTransform3D {
    pub fn matrix(self) -> Mat4 {
        self.matrix_with_source(Mat4::IDENTITY)
    }

    pub fn matrix_with_source(self, source: Mat4) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale,
            rotation(self.rotation_degrees, self.rotation_order),
            self.position,
        ) * source
            * Mat4::from_translation(-self.anchor)
    }
}

pub fn rotation(degrees: Vec3, order: RotationOrder) -> Quat {
    Quat::from_euler(
        euler(order),
        degrees.x.to_radians(),
        degrees.y.to_radians(),
        degrees.z.to_radians(),
    )
}

pub fn rotation_degrees(rotation: Quat, order: RotationOrder) -> Vec3 {
    let (x, y, z) = rotation.to_euler(euler(order));
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
}

pub fn camera_world(position: Vec3, rotation_degrees: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(rotation(rotation_degrees, RotationOrder::Xyz), position)
}

fn euler(order: RotationOrder) -> EulerRot {
    match order {
        RotationOrder::Xyz => EulerRot::XYZ,
        RotationOrder::Xzy => EulerRot::XZY,
        RotationOrder::Yxz => EulerRot::YXZ,
        RotationOrder::Yzx => EulerRot::YZX,
        RotationOrder::Zxy => EulerRot::ZXY,
        RotationOrder::Zyx => EulerRot::ZYX,
    }
}
