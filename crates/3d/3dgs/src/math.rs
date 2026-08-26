use glam::{Mat4, Vec3};

use crate::{Error, GaussianCloud, Projection, RenderParams, shader};

const MIN_CAMERA_DISTANCE: f32 = 0.001;
const MAX_CAMERA_DISTANCE: f32 = 1_000_000.0;
const SH_C0: f32 = 0.282_094_8;

pub(crate) fn point_cloud_log_scale(source_radius: f32, point_count: usize) -> f32 {
    (source_radius / (point_count as f32).sqrt()).ln()
}

pub(crate) fn point_cloud_opacity_logit() -> f32 {
    const OPACITY: f32 = 0.99;
    (OPACITY / (1.0 - OPACITY)).ln()
}

pub(crate) fn point_cloud_sh_dc(color: [f32; 3]) -> [f32; 3] {
    color.map(|channel| (channel - 0.5) / SH_C0)
}

impl RenderParams {
    pub(crate) fn uniforms(
        &self,
        cloud: &GaussianCloud,
        width: u32,
        height: u32,
    ) -> Result<shader::GaussianUniforms, Error> {
        if width == 0 || height == 0 {
            return Err(Error::message("3DGS render dimensions must be nonzero"));
        }
        let camera_values = [
            self.camera.vertical_fov_degrees,
            self.camera.orthographic_height,
            self.camera.focus_distance,
            self.camera.f_stop,
            self.camera.exposure_ev,
        ];
        if camera_values.iter().any(|value| !value.is_finite()) {
            return Err(Error::message(
                "3DGS camera contains a non-finite evaluated value",
            ));
        }
        let source_to_scene = Mat4::from_scale(Vec3::new(1.0, -1.0, -1.0));
        let model = self.model.matrix_with_source(source_to_scene);
        let camera_world =
            shrimply_transform_3d::camera_world(self.camera.position, self.camera.rotation_degrees);
        let object_to_camera = camera_world.inverse() * model;
        let camera_in_object = model.inverse().transform_point3(self.camera.position);
        if !object_to_camera.is_finite() || !camera_in_object.is_finite() {
            return Err(Error::message("3DGS scene transform is not finite"));
        }
        let vertical_fov = self
            .camera
            .vertical_fov_degrees
            .clamp(1.0, 179.0)
            .to_radians();
        let focal = height as f32 / (2.0 * (vertical_fov * 0.5).tan());
        let aperture_radius = if self.camera.projection == Projection::Perspective
            && self.camera.focus_distance > 0.0
        {
            (shrimply_transform_3d::focal_length_mm(vertical_fov.to_degrees() as f64) as f32
                / 1_000.0)
                / (2.0
                    * self
                        .camera
                        .f_stop
                        .clamp(crate::MIN_F_STOP, crate::MAX_F_STOP))
        } else {
            0.0
        };
        let orthographic_height = self
            .camera
            .orthographic_height
            .clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE);
        let orthographic_width = orthographic_height * width as f32 / height as f32;
        let exposure = 2.0f32.powf(
            self.camera
                .exposure_ev
                .clamp(crate::MIN_EXPOSURE_EV, crate::MAX_EXPOSURE_EV),
        );
        if !focal.is_finite()
            || !aperture_radius.is_finite()
            || !orthographic_width.is_finite()
            || !exposure.is_finite()
        {
            return Err(Error::message(
                "3DGS camera settings exceed the renderable range",
            ));
        }
        Ok(shader::GaussianUniforms {
            object_to_camera: object_to_camera.to_cols_array(),
            camera_in_object: camera_in_object.extend(0.0).to_array(),
            viewport_projection: [
                width as f32,
                height as f32,
                focal,
                match self.camera.projection {
                    Projection::Perspective => 1.0,
                    Projection::Orthographic => 0.0,
                    Projection::Equirectangular => 2.0,
                    Projection::Cylindrical => 3.0,
                    Projection::Fisheye => 4.0,
                },
            ],
            orthographic_source: [
                orthographic_width,
                orthographic_height,
                cloud.source_radius,
                exposure,
            ],
            source_center_degree: [
                cloud.source_center.x,
                cloud.source_center.y,
                cloud.source_center.z,
                cloud.spherical_harmonic_degree as f32,
            ],
            camera_lens: [
                self.camera
                    .focus_distance
                    .clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE),
                aperture_radius.max(0.0),
                if self.camera.projection == Projection::Fisheye {
                    self.camera
                        .vertical_fov_degrees
                        .clamp(1.0, 360.0)
                        .to_radians()
                        * 0.5
                } else {
                    (vertical_fov * 0.5).tan()
                },
                if aperture_radius > 0.0 {
                    crate::DEPTH_OF_FIELD_SAMPLES as f32
                } else {
                    1.0
                },
            ],
        })
    }
}
