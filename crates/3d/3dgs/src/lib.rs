mod math;
mod ply;
mod renderer;

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use glam::Vec3;
use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_core::timeline_value::TimelineValue;
pub use shrimply_transform_3d::{
    COLMAP_TRACKING_MODEL, Camera3D, CameraSource, ColmapCameraModel, ColmapQuality,
    MAX_EXPOSURE_EV, MAX_F_STOP, MIN_EXPOSURE_EV, MIN_F_STOP, Projection,
    ResolvedTransform3D as Transform, RotationOrder, TrackingCameraSource, TrackingSettings,
    Transform3D, VGGT_SLAM_TRACKING_MODEL, focal_length_mm, vertical_fov_degrees,
};

pub type AnimatedVec3 = TimelineValue<Vec3>;
pub type AnimatedTransform3d = Transform3D<AnimatedVec3, TimelineValue<RotationOrder>>;
pub type Camera3d = Camera3D<AnimatedVec3, TimelineValue<f32>>;
pub type CameraProjection = Projection;

pub(crate) const DEPTH_OF_FIELD_SAMPLES: u32 = 8;

pub use ply::{Gaussian, GaussianCloud, PlyError, load_gaussian_ply};
pub use renderer::{RenderContext, Renderer};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GaussianScene {
    pub model: AnimatedTransform3d,
    pub camera: Camera3d,
}

include!(concat!(env!("OUT_DIR"), "/slang_bindings.rs"));
pub use gaussian as shader;

#[derive(Debug)]
pub struct Error(String);

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub struct RenderSession {
    identity: AssetSnapshot,
    cloud: Arc<GaussianCloud>,
}

static CLOUD_CACHE: Mutex<Option<(AssetSnapshot, Arc<GaussianCloud>)>> = Mutex::new(None);

impl RenderSession {
    pub fn load(asset: &Asset) -> Result<Self, Error> {
        let identity = asset.snapshot().map_err(Error::message)?;
        if let Some((_, cloud)) = CLOUD_CACHE
            .lock()
            .expect("3DGS CPU cache lock poisoned")
            .as_ref()
            .filter(|(cached, _)| cached == &identity)
        {
            return Ok(Self {
                identity,
                cloud: cloud.clone(),
            });
        }
        let cloud = Arc::new(load_gaussian_ply(identity.path()).map_err(|error| {
            Error::message(format!("parse {}: {error}", identity.path().display()))
        })?);
        identity.verify_current().map_err(Error::message)?;
        *CLOUD_CACHE.lock().expect("3DGS CPU cache lock poisoned") =
            Some((identity.clone(), cloud.clone()));
        Ok(Self { identity, cloud })
    }

    pub fn matches_asset(&self, asset: &Asset) -> Result<bool, Error> {
        Ok(self.identity == asset.snapshot().map_err(Error::message)?)
    }

    pub fn path(&self) -> &Path {
        self.identity.path()
    }

    pub fn identity(&self) -> &AssetSnapshot {
        &self.identity
    }

    pub fn cloud(&self) -> &GaussianCloud {
        &self.cloud
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub projection: Projection,
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub vertical_fov_degrees: f32,
    pub orthographic_height: f32,
    pub focus_distance: f32,
    pub f_stop: f32,
    pub exposure_ev: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderParams {
    pub model: Transform,
    pub camera: Camera,
}

impl RenderParams {
    pub fn preview_control(&self, canvas_size: glam::Vec2) -> Option<shrimply_3d_control::Control> {
        let control = shrimply_3d_control::Control::new(shrimply_3d_control::ControlInput {
            model: self.model,
            camera_position: self.camera.position,
            camera_rotation_degrees: self.camera.rotation_degrees,
            projection: self.camera.projection,
            vertical_fov_degrees: self.camera.vertical_fov_degrees,
            orthographic_height: self.camera.orthographic_height,
            canvas_size,
        })?;
        Some(control.keep_camera_outside(self.model.scale.abs().max_element()))
    }
}

impl shader::GaussianSource {
    pub(crate) fn from_gaussian(gaussian: &Gaussian) -> Self {
        Self {
            position_opacity: gaussian.position.extend(gaussian.opacity).to_array(),
            scale: gaussian.scale.extend(0.0).to_array(),
            rotation: gaussian.rotation,
            dc: [
                gaussian.spherical_harmonics[0],
                gaussian.spherical_harmonics[1],
                gaussian.spherical_harmonics[2],
                0.0,
            ],
        }
    }
}
