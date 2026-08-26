use serde::{Deserialize, Serialize};
use shrimply_asset::Asset;
use shrimply_core::{
    Color,
    modifier_model::{
        KeyframeSpan, ModifierModel, combine, ensure_timeline_value_ids, timeline_value_span,
    },
    timeline_value::TimelineValue,
};
use shrimply_scene_3d::{PbrMaterial, Transform3d};

use hashbrown::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Object3dModifier {
    pub file: Option<Asset>,
    pub transform: Transform3d,
    pub material: PbrMaterial,
}

impl Object3dModifier {
    pub fn with_file(file: impl Into<Asset>) -> Self {
        Self {
            file: Some(file.into()),
            ..Self::default()
        }
    }
}

impl ModifierModel for Object3dModifier {
    fn display_name(&self) -> &'static str {
        "3D Object"
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["model", "mesh", "OBJ", "geometry"]
    }

    fn ensure_ids(&mut self, seen: &mut HashSet<Uuid>) {
        for value in [
            &mut self.transform.position,
            &mut self.transform.anchor,
            &mut self.transform.rotation_degrees,
            &mut self.transform.scale,
        ] {
            ensure_timeline_value_ids(value, seen);
        }
        ensure_timeline_value_ids(&mut self.transform.rotation_order, seen);
        for value in numbers_mut(&mut self.material) {
            ensure_timeline_value_ids(value, seen);
        }
        for value in colors_mut(&mut self.material) {
            ensure_timeline_value_ids(value, seen);
        }
    }

    fn keyframe_span(&self) -> KeyframeSpan {
        combine(
            [
                timeline_value_span(&self.transform.position),
                timeline_value_span(&self.transform.anchor),
                timeline_value_span(&self.transform.rotation_degrees),
                timeline_value_span(&self.transform.rotation_order),
                timeline_value_span(&self.transform.scale),
            ]
            .into_iter()
            .chain(numbers(&self.material).into_iter().map(timeline_value_span))
            .chain(colors(&self.material).into_iter().map(timeline_value_span)),
        )
    }

    fn number(&self, id: Uuid) -> Option<&TimelineValue<f32>> {
        numbers(&self.material)
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<f32>> {
        numbers_mut(&mut self.material)
            .into_iter()
            .find(|value| value.id == id)
    }

    fn number3(&self, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
        [
            &self.transform.position,
            &self.transform.anchor,
            &self.transform.rotation_degrees,
            &self.transform.scale,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn number3_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<glam::Vec3>> {
        [
            &mut self.transform.position,
            &mut self.transform.anchor,
            &mut self.transform.rotation_degrees,
            &mut self.transform.scale,
        ]
        .into_iter()
        .find(|value| value.id == id)
    }

    fn color_mut(&mut self, id: Uuid) -> Option<&mut TimelineValue<Color<u8>>> {
        colors_mut(&mut self.material)
            .into_iter()
            .find(|value| value.id == id)
    }
}

fn numbers(material: &PbrMaterial) -> Vec<&TimelineValue<f32>> {
    vec![
        &material.metallic,
        &material.roughness,
        &material.subsurface,
        &material.clearcoat,
        &material.sheen,
        &material.transmission,
        &material.ior,
        &material.toon.bands,
        &material.toon.color_levels,
        &material.toon.kuwahara_radius,
        &material.toon.kuwahara_strength,
        &material.toon.shadow_strength,
        &material.toon.shadow_darkest_tone,
        &material.toon.shadow_dot_size,
        &material.toon.shadow_dot_density,
        &material.toon.shadow_dot_distribution_randomness,
        &material.toon.shadow_dot_size_randomness,
        &material.toon.shadow_line_direction_degrees,
        &material.toon.shadow_line_width,
        &material.toon.shadow_line_density,
        &material.toon.shadow_line_distribution_randomness,
        &material.toon.shadow_line_width_randomness,
        &material.toon.shadow_pattern_softness,
        &material.toon.shadow_crosshatch_angle_degrees,
        &material.toon.shadow_crosshatch_max_directions,
        &material.toon.rim_strength,
        &material.toon.rim_power,
        &material.toon.specular_size,
        &material.toon.specular_strength,
        &material.toon.outline.width,
        &material.toon.outline.opacity,
        &material.toon.outline.depth_threshold,
        &material.toon.outline.normal_angle_degrees,
        &material.toon.outline.dog_inner_radius,
        &material.toon.outline.dog_radius_ratio,
        &material.toon.outline.dog_threshold,
        &material.toon.outline.dog_sharpness,
        &material.toon.outline.offset_variation,
        &material.toon.outline.width_variation,
        &material.toon.outline.offset_frequency,
        &material.toon.outline.width_frequency,
        &material.toon.outline.aggressiveness,
        &material.toon.outline.noise_seed,
        &material.toon.outline.noise_evolution,
    ]
}

fn numbers_mut(material: &mut PbrMaterial) -> Vec<&mut TimelineValue<f32>> {
    vec![
        &mut material.metallic,
        &mut material.roughness,
        &mut material.subsurface,
        &mut material.clearcoat,
        &mut material.sheen,
        &mut material.transmission,
        &mut material.ior,
        &mut material.toon.bands,
        &mut material.toon.color_levels,
        &mut material.toon.kuwahara_radius,
        &mut material.toon.kuwahara_strength,
        &mut material.toon.shadow_strength,
        &mut material.toon.shadow_darkest_tone,
        &mut material.toon.shadow_dot_size,
        &mut material.toon.shadow_dot_density,
        &mut material.toon.shadow_dot_distribution_randomness,
        &mut material.toon.shadow_dot_size_randomness,
        &mut material.toon.shadow_line_direction_degrees,
        &mut material.toon.shadow_line_width,
        &mut material.toon.shadow_line_density,
        &mut material.toon.shadow_line_distribution_randomness,
        &mut material.toon.shadow_line_width_randomness,
        &mut material.toon.shadow_pattern_softness,
        &mut material.toon.shadow_crosshatch_angle_degrees,
        &mut material.toon.shadow_crosshatch_max_directions,
        &mut material.toon.rim_strength,
        &mut material.toon.rim_power,
        &mut material.toon.specular_size,
        &mut material.toon.specular_strength,
        &mut material.toon.outline.width,
        &mut material.toon.outline.opacity,
        &mut material.toon.outline.depth_threshold,
        &mut material.toon.outline.normal_angle_degrees,
        &mut material.toon.outline.dog_inner_radius,
        &mut material.toon.outline.dog_radius_ratio,
        &mut material.toon.outline.dog_threshold,
        &mut material.toon.outline.dog_sharpness,
        &mut material.toon.outline.offset_variation,
        &mut material.toon.outline.width_variation,
        &mut material.toon.outline.offset_frequency,
        &mut material.toon.outline.width_frequency,
        &mut material.toon.outline.aggressiveness,
        &mut material.toon.outline.noise_seed,
        &mut material.toon.outline.noise_evolution,
    ]
}

fn colors(material: &PbrMaterial) -> Vec<&TimelineValue<Color<u8>>> {
    vec![
        &material.base_color,
        &material.toon.shadow_color,
        &material.toon.rim_color,
        &material.toon.outline.color,
    ]
}

fn colors_mut(material: &mut PbrMaterial) -> Vec<&mut TimelineValue<Color<u8>>> {
    vec![
        &mut material.base_color,
        &mut material.toon.shadow_color,
        &mut material.toon.rim_color,
        &mut material.toon.outline.color,
    ]
}
