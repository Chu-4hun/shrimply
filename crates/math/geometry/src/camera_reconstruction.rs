use glam::{DMat4, DQuat, DVec3, DVec4, EulerRot, Quat, Vec3};
use serde::Deserialize;

const UNIT_SCALE_TOLERANCE: f64 = 1.0e-6;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct CameraPose {
    pub camera_from_world_rotation: DQuat,
    pub camera_from_world_translation: DVec3,
}

#[derive(Clone, Copy, Debug)]
pub struct NormalizedCameraPose {
    pub position: DVec3,
    pub rotation: DQuat,
}

#[derive(Clone, Copy, Debug)]
pub struct InterpolatedCameraMotion {
    pub position: DVec3,
    pub rotation: DQuat,
    pub vertical_fov_degrees: f64,
}

pub fn apply_reconstructed_camera_motion(
    tracked_position: Vec3,
    tracked_rotation: Quat,
    custom_position: Vec3,
    custom_rotation_degrees: Vec3,
) -> (Vec3, Quat) {
    let tracked_rotation = tracked_rotation.normalize();
    let custom_rotation = Quat::from_euler(
        EulerRot::XYZ,
        custom_rotation_degrees.x.to_radians(),
        custom_rotation_degrees.y.to_radians(),
        custom_rotation_degrees.z.to_radians(),
    );
    (
        custom_position + custom_rotation * tracked_position,
        (custom_rotation * tracked_rotation).normalize(),
    )
}

pub fn relative_camera_poses(poses: &[CameraPose]) -> Result<Vec<NormalizedCameraPose>, String> {
    let basis = DMat4::from_diagonal(DVec4::new(1.0, -1.0, -1.0, 1.0));
    let mut cameras = Vec::with_capacity(poses.len());
    for pose in poses {
        let rotation = pose.camera_from_world_rotation;
        let translation = pose.camera_from_world_translation;
        if !rotation.is_finite() || !translation.is_finite() || rotation.length_squared() == 0.0 {
            return Err("3D tracking returned an invalid camera pose".to_string());
        }
        let camera_from_world = DMat4::from_rotation_translation(rotation.normalize(), translation);
        let camera_to_world = camera_from_world.inverse();
        if !camera_to_world
            .to_cols_array()
            .into_iter()
            .all(f64::is_finite)
        {
            return Err("3D tracking camera pose is not invertible".to_string());
        }
        cameras.push(basis * camera_to_world * basis);
    }
    let Some(first) = cameras.first().copied() else {
        return Ok(Vec::new());
    };
    let reference_inverse = first.inverse();
    cameras
        .into_iter()
        .enumerate()
        .map(|(index, camera)| {
            if index == 0 {
                return Ok(NormalizedCameraPose {
                    position: DVec3::ZERO,
                    rotation: DQuat::IDENTITY,
                });
            }
            let normalized = reference_inverse * camera;
            let (scale, rotation, position) = normalized.to_scale_rotation_translation();
            if !scale.is_finite()
                || !rotation.is_finite()
                || !position.is_finite()
                || (scale - DVec3::ONE).abs().max_element() > UNIT_SCALE_TOLERANCE
                || rotation.length_squared() == 0.0
            {
                return Err("normalized 3D tracking camera pose is invalid".to_string());
            }
            Ok(NormalizedCameraPose {
                position,
                rotation: rotation.normalize(),
            })
        })
        .collect()
}

pub fn interpolate_camera_motion(
    from: InterpolatedCameraMotion,
    to: InterpolatedCameraMotion,
    progress: f64,
) -> InterpolatedCameraMotion {
    let progress = progress.clamp(0.0, 1.0);
    let position = from.position.lerp(to.position, progress);
    let from_rotation = from.rotation.normalize();
    let mut to_rotation = to.rotation.normalize();
    if from_rotation.dot(to_rotation) < 0.0 {
        to_rotation = -to_rotation;
    }
    InterpolatedCameraMotion {
        position,
        rotation: from_rotation.slerp(to_rotation, progress).normalize(),
        vertical_fov_degrees: from.vertical_fov_degrees
            + (to.vertical_fov_degrees - from.vertical_fov_degrees) * progress,
    }
}

pub fn vertical_fov_degrees_from_focal_length(
    image_height: u32,
    focal_y: f64,
) -> Result<f64, String> {
    if image_height == 0 || !focal_y.is_finite() || focal_y <= 0.0 {
        return Err("3D tracking returned invalid camera intrinsics".to_string());
    }
    let fov = 2.0
        * (f64::from(image_height) / (2.0 * focal_y))
            .atan()
            .to_degrees();
    if fov.is_finite() {
        Ok(fov)
    } else {
        Err("3D tracking vertical FOV is not finite".to_string())
    }
}
