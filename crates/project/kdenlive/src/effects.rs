use std::{collections::BTreeSet, error::Error};

use glam::{Mat3, Vec2};
use shrimply_audio_modifiers::{AudioModifier, AudioModifierEffect, GainModifier};
use shrimply_core::timeline_value::{TimelineBase, TimelineCurveKeyframe, TimelineValue};
use shrimply_math_core::Fraction;
use shrimply_project::{
    AlphaMaskShape, AudioItem, AudioTransition, Color, LayerBlendMode, Time, TransitionSide,
    VideoItem, VisualAlphaMask, VisualModifier, VisualTransition,
};
use shrimply_video_modifiers::{
    ModifierEffect, RasterModifierEffect,
    chroma_key::ChromaKeyModifier,
    color_correction::ColorCorrectionModifier,
    crop::{CropEdges, CropModifier},
    gaussian_blur::{GaussianBlurChannels, GaussianBlurModifier},
};
use uuid::Uuid;

use super::{Converter, entry_in, frame_time, invalid, math, xml::Element};

const ALPHA_SPOT_SHAPE_COUNT: u32 = 4;
const ALPHA_SPOT_RECTANGLE: u32 = 0;
const ALPHA_SPOT_ELLIPSE: u32 = 1;
const ALPHA_SPOT_SIZE_SCALE: f32 = 2.0;
const ALPHA_SPOT_OPERATION_COUNT: u32 = 5;
const ALPHA_SPOT_WRITE_ON_CLEAR: u32 = 0;
const ALPHA_SPOT_MINIMUM: u32 = 2;
const ALPHA_SPOT_SUBTRACT: u32 = 4;

type CropAnimation = [Vec<math::Keyframe<f32>>; 4];

struct QtblendGeometry {
    rects: Vec<math::Keyframe<math::RectValue>>,
    rotations: Vec<math::Keyframe<f32>>,
    rotation_anchors: Vec<math::Keyframe<Vec2>>,
    distort: bool,
}

#[derive(Clone, Copy)]
struct SourceGeometry {
    size: Vec2,
    oriented_size: Vec2,
    anchor: Vec2,
    orientation: Mat3,
    rotation_degrees: f32,
}

impl<'a> Converter<'a> {
    pub(super) fn apply_visual_effects(
        &mut self,
        entry: &Element,
        item: &mut VideoItem,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut imported_crop = self.apply_qtblend(entry, item)?;
        let mut inside_mask = false;
        let mut active_mask = None;
        let filters = entry
            .children_named("filter")
            .filter(|filter| filter.property("disable") != Some("1"))
            .collect::<Vec<_>>();
        for (index, filter) in filters.iter().copied().enumerate() {
            let id = filter_id(filter);
            if id.starts_with("mask_start") {
                if inside_mask {
                    active_mask = None;
                    self.warnings.insert(
                        "A malformed nested Kdenlive effect-mask group was skipped.".to_owned(),
                    );
                    continue;
                }
                inside_mask = true;
                let Some(group_end) = filters[index + 1..]
                    .iter()
                    .position(|candidate| filter_id(candidate) == "mask_apply")
                    .map(|offset| index + 1 + offset)
                else {
                    active_mask = None;
                    self.warnings.insert(
                        "An unterminated Kdenlive effect-mask group was skipped.".to_owned(),
                    );
                    continue;
                };
                let group = &filters[index + 1..group_end];
                if let Some(unsupported) = group
                    .iter()
                    .copied()
                    .find(|candidate| !supports_modifier_alpha_mask(filter_id(candidate)))
                {
                    active_mask = None;
                    let unsupported = filter_id(unsupported);
                    self.warnings.insert(if unsupported.is_empty() {
                        "A Kdenlive effect-mask group containing an unidentified effect was skipped."
                            .to_owned()
                    } else {
                        format!(
                            "A Kdenlive effect-mask group containing unsupported or non-raster effect {unsupported} was skipped."
                        )
                    });
                    continue;
                }
                if !is_alpha_spot_mask(filter) {
                    active_mask = None;
                    self.warnings.insert(format!(
                        "Kdenlive effect mask {id} cannot be represented by Shrimply; its grouped effects were skipped."
                    ));
                    continue;
                }
                if self.alpha_spot_mask(filter, entry)?.is_none() {
                    active_mask = None;
                    self.warnings.insert(
                        "A Kdenlive Alpha Shapes effect mask used an unsupported shape or alpha operation; its grouped effects were skipped."
                            .to_owned(),
                    );
                } else {
                    active_mask = Some(filter);
                    if group.len() > 1 {
                        self.warnings.insert(
                            "A Kdenlive multi-effect mask group was approximated by applying its mask to each Shrimply modifier."
                                .to_owned(),
                        );
                    }
                }
                continue;
            }
            if id == "mask_apply" {
                inside_mask = false;
                active_mask = None;
                continue;
            }
            if (inside_mask && active_mask.is_none()) || id == "qtblend" {
                continue;
            }
            let modifier_start = item.modifiers.len();
            match id {
                "fade_from_black" => {
                    item.transitions.intro = Some(VisualTransition::new(
                        TransitionSide::Intro,
                        filter_duration(filter, item.start, item.end, self.fps),
                        self.canvas_size,
                    ));
                }
                "fade_to_black" => {
                    item.transitions.outro = Some(VisualTransition::new(
                        TransitionSide::Outro,
                        filter_duration(filter, item.start, item.end, self.fps),
                        self.canvas_size,
                    ));
                }
                "qtcrop" => {
                    if active_mask.is_some() {
                        let mut masked_crop = None;
                        self.apply_crop(filter, entry, item, &mut masked_crop)?;
                    } else {
                        self.apply_crop(filter, entry, item, &mut imported_crop)?;
                    }
                }
                "avfilter.gblur" => self.apply_blur(filter, entry, item)?,
                "chroma" => self.apply_chroma(filter, entry, item)?,
                "frei0r.saturat0r" => {
                    let effect = ColorCorrectionModifier {
                        saturation: animated_scalar(
                            filter.property("Saturation").unwrap_or("0=0.125"),
                            self.fps,
                            entry,
                            |value| value * 8.0,
                        )?,
                        ..Default::default()
                    };
                    push_raster(
                        item,
                        RasterModifierEffect::ColorCorrection(Box::new(effect)),
                    );
                }
                "frei0r.hueshift0r" => {
                    let effect = ColorCorrectionModifier {
                        hue_degrees: animated_scalar(
                            filter.property("Hue").unwrap_or("0=0"),
                            self.fps,
                            entry,
                            |value| value * 360.0,
                        )?,
                        ..Default::default()
                    };
                    push_raster(
                        item,
                        RasterModifierEffect::ColorCorrection(Box::new(effect)),
                    );
                }
                "lift_gamma_gain" => {
                    let effect = ColorCorrectionModifier {
                        brightness: animated_scalar(
                            filter.property("lift_r").unwrap_or("0=0"),
                            self.fps,
                            entry,
                            |value| value,
                        )?,
                        gamma: animated_scalar(
                            filter.property("gamma_r").unwrap_or("0=1"),
                            self.fps,
                            entry,
                            |value| value,
                        )?,
                        value: animated_scalar(
                            filter.property("gain_r").unwrap_or("0=1"),
                            self.fps,
                            entry,
                            |value| value,
                        )?,
                        ..Default::default()
                    };
                    push_raster(
                        item,
                        RasterModifierEffect::ColorCorrection(Box::new(effect)),
                    );
                    self.warnings.insert(
                        "Kdenlive lift/gamma/gain was approximated with Shrimply color correction; per-channel differences cannot be represented."
                            .to_owned(),
                    );
                }
                "avfilter.colorcorrect" => {
                    self.warnings.insert(
                        "Kdenlive selective color correction is unsupported and was skipped."
                            .to_owned(),
                    );
                }
                "" => {}
                unsupported => {
                    self.warnings.insert(format!(
                        "Kdenlive video effect {unsupported} is unsupported and was skipped."
                    ));
                }
            }
            if let Some(mask_filter) = active_mask {
                let modifiers = &mut item.modifiers[modifier_start..];
                assert!(
                    !modifiers.is_empty(),
                    "supported masked Kdenlive effect did not create a modifier"
                );
                for modifier in modifiers {
                    modifier.alpha_mask = self.alpha_spot_mask(mask_filter, entry)?;
                }
            }
        }
        if let (Some(intro), Some(outro)) =
            (&mut item.transitions.intro, &mut item.transitions.outro)
        {
            math::fit_durations(
                &mut intro.duration,
                &mut outro.duration,
                Time {
                    seconds: item.end.seconds - item.start.seconds,
                },
            );
        }
        Ok(())
    }

    fn alpha_spot_mask(
        &self,
        filter: &Element,
        entry: &Element,
    ) -> Result<Option<VisualAlphaMask>, Box<dyn Error + Send + Sync>> {
        let shape = match math::frei0r_parameter_index(
            filter.property("filter.Shape").unwrap_or("0").parse()?,
            ALPHA_SPOT_SHAPE_COUNT,
        ) {
            ALPHA_SPOT_RECTANGLE => AlphaMaskShape::Rectangle,
            ALPHA_SPOT_ELLIPSE => AlphaMaskShape::Ellipse,
            _ => return Ok(None),
        };
        let outside_high = match (
            constant_scalar_animation(filter.property("filter.Min").unwrap_or("0=0"), self.fps)
                .map_err(invalid)?,
            constant_scalar_animation(filter.property("filter.Max").unwrap_or("0=1"), self.fps)
                .map_err(invalid)?,
        ) {
            (Some(0.0), Some(1.0)) => false,
            (Some(1.0), Some(0.0)) => true,
            _ => return Ok(None),
        };
        let operation = math::frei0r_parameter_index(
            filter.property("filter.Operation").unwrap_or("0").parse()?,
            ALPHA_SPOT_OPERATION_COUNT,
        );
        let invert = match operation {
            ALPHA_SPOT_WRITE_ON_CLEAR | ALPHA_SPOT_MINIMUM => outside_high,
            ALPHA_SPOT_SUBTRACT => !outside_high,
            _ => return Ok(None),
        };
        Ok(Some(VisualAlphaMask {
            enabled: true,
            shape,
            center: animated_vec2(
                filter.property("filter.Position X").unwrap_or("0=0.5"),
                filter.property("filter.Position Y").unwrap_or("0=0.5"),
                self.fps,
                entry,
                |value| value,
            )?,
            size: animated_vec2(
                filter.property("filter.Size X").unwrap_or("0=0.1"),
                filter.property("filter.Size Y").unwrap_or("0=0.1"),
                self.fps,
                entry,
                |value| value * ALPHA_SPOT_SIZE_SCALE,
            )?,
            rotation_degrees: animated_scalar(
                filter.property("filter.Tilt").unwrap_or("0=0.5"),
                self.fps,
                entry,
                math::frei0r_tilt_degrees,
            )?,
            feather: animated_scalar(
                filter
                    .property("filter.Transition width")
                    .unwrap_or("0=0.2"),
                self.fps,
                entry,
                |value| value.clamp(0.0, 1.0),
            )?,
            rounding: TimelineValue::new_const(0.0),
            vertices: vec![
                glam::Vec2::new(-0.5, -0.5),
                glam::Vec2::new(0.5, -0.5),
                glam::Vec2::new(0.5, 0.5),
                glam::Vec2::new(-0.5, 0.5),
            ],
            invert,
        }))
    }

    fn apply_qtblend(
        &mut self,
        entry: &Element,
        item: &mut VideoItem,
    ) -> Result<Option<CropAnimation>, Box<dyn Error + Send + Sync>> {
        let qtblends = qtblends_outside_effect_masks(entry, None);
        if qtblends.is_empty() {
            return Ok(None);
        }
        if qtblends.len() > 1 {
            self.warnings.insert(
                "Stacked Kdenlive transforms were matrix-composed; shear created by the composition is approximated."
                    .to_owned(),
            );
        }

        let canvas_size = Vec2::new(
            self.canvas_size.width.max(1) as f32,
            self.canvas_size.height.max(1) as f32,
        );
        let geometries = qtblend_geometries(&qtblends, self.fps, canvas_size)?;
        let mut frames = BTreeSet::new();
        for geometry in &geometries {
            frames.extend(geometry.rects.iter().map(|keyframe| keyframe.frame));
            frames.extend(geometry.rotations.iter().map(|keyframe| keyframe.frame));
            frames.extend(
                geometry
                    .rotation_anchors
                    .iter()
                    .map(|keyframe| keyframe.frame),
            );
        }

        let source = source_geometry(item);
        let entry_in = entry_in(entry, self.fps)?;
        let mut positions = Vec::new();
        let mut scales = Vec::new();
        let mut rotations = Vec::new();
        let mut opacities = Vec::new();
        for &frame in &frames {
            let (matrix, expected_rotation, opacity) =
                qtblend_matrix_at(&geometries, source, canvas_size, frame);
            let x_axis = matrix.x_axis.truncate();
            let y_axis = matrix.y_axis.truncate();
            let scale = Vec2::new(x_axis.length(), y_axis.length());
            let rotation = math::equivalent_angle_near(
                x_axis.y.atan2(x_axis.x).to_degrees(),
                expected_rotation,
            );
            let interpolation = geometries
                .last()
                .map(|geometry| interpolation_at(&geometry.rects, frame))
                .unwrap_or(shrimply_project::Interpolation::Linear);
            let time = Time {
                seconds: frame_time(frame - entry_in, self.fps).seconds,
            };
            positions.push((time, matrix.transform_point2(source.anchor), interpolation));
            scales.push((time, scale, interpolation));
            let rotation_interpolation = geometries
                .iter()
                .rev()
                .find(|geometry| geometry.rotations.len() > 1)
                .or_else(|| geometries.last())
                .map(|geometry| interpolation_at(&geometry.rotations, frame))
                .unwrap_or(shrimply_project::Interpolation::Linear);
            rotations.push((time, rotation, rotation_interpolation));
            opacities.push((time, opacity, interpolation));
        }
        item.transform.position = vec2_timeline(positions);
        item.transform.anchor = TimelineValue::new_const(source.anchor);
        item.transform.scale = vec2_timeline(scales);
        item.transform.shear = TimelineValue::new_const(Vec2::ZERO);
        item.transform.rotation_degrees = scalar_timeline(rotations);
        item.compositing.opacity = scalar_timeline(opacities);
        item.compositing.blend_mode = TimelineValue::new_const(
            match qtblends
                .last()
                .and_then(|filter| filter.property("compositing"))
                .unwrap_or("0")
            {
                "0" => LayerBlendMode::Normal,
                "14" => LayerBlendMode::Screen,
                unsupported => {
                    self.warnings.insert(format!(
                    "Kdenlive blend mode {unsupported} is unsupported and was imported as Normal."
                ));
                    LayerBlendMode::Normal
                }
            },
        );
        if geometries.len() == 1 {
            return Ok(None);
        }

        let canvas =
            shrimply_math_geometry::Rect::from_xywh(0.0, 0.0, canvas_size.x, canvas_size.y);
        let mut crop: CropAnimation = std::array::from_fn(|_| Vec::new());
        let mut approximated = false;
        for frame in frames {
            let mut percentages = [0.0_f32; 4];
            for prefix in 1..geometries.len() {
                let (matrix, _, _) =
                    qtblend_matrix_at(&geometries[..prefix], source, canvas_size, frame);
                let (current, axis_aligned) =
                    math::transformed_crop_percentages(canvas, matrix, source.size)
                        .ok_or_else(|| invalid("stacked Kdenlive transform is not invertible"))?;
                percentages = std::array::from_fn(|index| percentages[index].max(current[index]));
                approximated |= !axis_aligned;
            }
            let interpolation = geometries
                .iter()
                .find_map(|geometry| {
                    geometry
                        .rects
                        .iter()
                        .find(|keyframe| keyframe.frame == frame)
                        .map(|keyframe| keyframe.interpolation)
                        .or_else(|| {
                            geometry
                                .rotations
                                .iter()
                                .find(|keyframe| keyframe.frame == frame)
                                .map(|keyframe| keyframe.interpolation)
                        })
                        .or_else(|| {
                            geometry
                                .rotation_anchors
                                .iter()
                                .find(|keyframe| keyframe.frame == frame)
                                .map(|keyframe| keyframe.interpolation)
                        })
                })
                .unwrap_or(shrimply_project::Interpolation::Linear);
            for (values, value) in crop.iter_mut().zip(percentages) {
                values.push(math::Keyframe {
                    frame,
                    value,
                    interpolation,
                });
            }
        }
        if approximated {
            self.warnings.insert(
                "Intermediate clipping between rotated or sheared Kdenlive transforms was approximated with a source-aligned crop."
                    .to_owned(),
            );
        }
        if !crop.iter().flatten().any(|keyframe| keyframe.value > 0.0) {
            return Ok(None);
        }
        let [top, right, bottom, left] = crop.clone().map(|values| {
            scalar_timeline(
                values
                    .into_iter()
                    .map(|keyframe| {
                        (
                            frame_time(keyframe.frame - entry_in, self.fps),
                            keyframe.value,
                            keyframe.interpolation,
                        )
                    })
                    .collect(),
            )
        });
        push_raster(
            item,
            RasterModifierEffect::Crop(CropModifier::Percentage(CropEdges {
                top,
                right,
                bottom,
                left,
            })),
        );
        Ok(Some(crop))
    }

    fn apply_crop(
        &mut self,
        filter: &Element,
        entry: &Element,
        item: &mut VideoItem,
        imported_crop: &mut Option<CropAnimation>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let circle = filter.property("circle").unwrap_or("0") != "0";
        let radius = constant_scalar_animation(filter.property("radius").unwrap_or("0"), self.fps)
            .map_err(invalid)?;
        if circle
            || radius != Some(0.0)
            || !transparent_color_animation(filter.property("color").unwrap_or("0x00000000"))
        {
            self.warnings.insert(
                "Rounded, circular, or colored Kdenlive crop padding was approximated as a rectangular crop."
                    .to_owned(),
            );
        }
        let canvas_size = Vec2::new(
            self.canvas_size.width.max(1) as f32,
            self.canvas_size.height.max(1) as f32,
        );
        let rects = math::rect_animation(
            filter
                .property("rect")
                .ok_or_else(|| invalid("crop has no rectangle"))?,
            self.fps,
            canvas_size,
        )
        .map_err(invalid)?;
        let qtblends = qtblends_outside_effect_masks(entry, Some(filter));
        let geometries = qtblend_geometries(&qtblends, self.fps, canvas_size)?;
        let mut frames = rects
            .iter()
            .map(|keyframe| keyframe.frame)
            .collect::<BTreeSet<_>>();
        for geometry in &geometries {
            frames.extend(geometry.rects.iter().map(|keyframe| keyframe.frame));
            frames.extend(geometry.rotations.iter().map(|keyframe| keyframe.frame));
            frames.extend(
                geometry
                    .rotation_anchors
                    .iter()
                    .map(|keyframe| keyframe.frame),
            );
        }
        let canvas_bounds = math::RectValue {
            rect: shrimply_math_geometry::Rect::from_xywh(0.0, 0.0, canvas_size.x, canvas_size.y),
            opacity: 1.0,
        };
        let source = source_geometry(item);
        let mut requested: CropAnimation = std::array::from_fn(|_| Vec::new());
        for frame in frames {
            let crop = math::value_at(&rects, frame, lerp_rect);
            let percentages = if geometries.is_empty() {
                math::crop_percentages(crop.rect, canvas_bounds.rect)
            } else {
                let (matrix, _, _) = qtblend_matrix_at(&geometries, source, canvas_size, frame);
                let (percentages, axis_aligned) =
                    math::transformed_crop_percentages(crop.rect, matrix, source.size).ok_or_else(
                        || invalid("crop follows a non-invertible Kdenlive transform"),
                    )?;
                if !axis_aligned {
                    self.warnings.insert(
                        "A Kdenlive canvas-aligned crop after rotation or shear was approximated with a source-aligned crop."
                            .to_owned(),
                    );
                }
                percentages
            };
            let interpolation = rects
                .iter()
                .find(|keyframe| keyframe.frame == frame)
                .map(|keyframe| keyframe.interpolation)
                .or_else(|| {
                    geometries.iter().find_map(|geometry| {
                        geometry
                            .rects
                            .iter()
                            .find(|keyframe| keyframe.frame == frame)
                            .map(|keyframe| keyframe.interpolation)
                            .or_else(|| {
                                geometry
                                    .rotations
                                    .iter()
                                    .find(|keyframe| keyframe.frame == frame)
                                    .map(|keyframe| keyframe.interpolation)
                                    .or_else(|| {
                                        geometry
                                            .rotation_anchors
                                            .iter()
                                            .find(|keyframe| keyframe.frame == frame)
                                            .map(|keyframe| keyframe.interpolation)
                                    })
                            })
                    })
                })
                .expect("crop frame must belong to the crop or preceding transforms");
            for (timeline, value) in requested.iter_mut().zip(percentages) {
                timeline.push(math::Keyframe {
                    frame,
                    value,
                    interpolation,
                });
            }
        }
        let mut all_frames = requested
            .iter()
            .flatten()
            .map(|keyframe| keyframe.frame)
            .collect::<BTreeSet<_>>();
        if let Some(existing) = imported_crop.as_ref() {
            all_frames.extend(existing.iter().flatten().map(|keyframe| keyframe.frame));
        }
        let existing = imported_crop.take();
        let mut relative: CropAnimation = std::array::from_fn(|_| Vec::new());
        let mut combined: CropAnimation = std::array::from_fn(|_| Vec::new());
        for frame in all_frames {
            let requested_values = requested
                .each_ref()
                .map(|values| math::value_at(values, frame, lerp_f32));
            let existing_values = existing.as_ref().map_or([0.0; 4], |existing| {
                existing
                    .each_ref()
                    .map(|values| math::value_at(values, frame, lerp_f32))
            });
            let (relative_values, combined_values) =
                math::relative_crop(existing_values, requested_values);
            let interpolation = requested
                .iter()
                .flatten()
                .find(|keyframe| keyframe.frame == frame)
                .or_else(|| {
                    existing
                        .as_ref()?
                        .iter()
                        .flatten()
                        .find(|keyframe| keyframe.frame == frame)
                })
                .map_or(shrimply_project::Interpolation::Linear, |keyframe| {
                    keyframe.interpolation
                });
            for ((relative, combined), (relative_value, combined_value)) in relative
                .iter_mut()
                .zip(combined.iter_mut())
                .zip(relative_values.into_iter().zip(combined_values))
            {
                relative.push(math::Keyframe {
                    frame,
                    value: relative_value,
                    interpolation,
                });
                combined.push(math::Keyframe {
                    frame,
                    value: combined_value,
                    interpolation,
                });
            }
        }
        *imported_crop = Some(combined);
        let entry_in = entry_in(entry, self.fps)?;
        let [top, right, bottom, left] = relative.map(|values| {
            scalar_timeline(
                values
                    .into_iter()
                    .map(|keyframe| {
                        (
                            frame_time(keyframe.frame - entry_in, self.fps),
                            keyframe.value,
                            keyframe.interpolation,
                        )
                    })
                    .collect(),
            )
        });
        let effect = CropModifier::Percentage(CropEdges {
            top,
            right,
            bottom,
            left,
        });
        push_raster(item, RasterModifierEffect::Crop(effect));
        Ok(())
    }

    fn apply_blur(
        &mut self,
        filter: &Element,
        entry: &Element,
        item: &mut VideoItem,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let channels = match filter.property("av.planes").unwrap_or("7") {
            "7" => GaussianBlurChannels::Rgb,
            "8" => GaussianBlurChannels::Alpha,
            _ => {
                self.warnings.insert(
                    "Kdenlive Gaussian blur with partial color-plane selection was applied to all RGBA channels."
                        .to_owned(),
                );
                GaussianBlurChannels::Rgba
            }
        };
        let sigma = filter.property("av.sigma").unwrap_or("0=10");
        let effect = GaussianBlurModifier {
            radius: animated_vec2(
                sigma,
                filter.property("av.sigmaV").unwrap_or(sigma),
                self.fps,
                entry,
                |value| Vec2::new(value.x, if value.y < 0.0 { value.x } else { value.y }),
            )?,
            channels,
        };
        push_raster(item, RasterModifierEffect::GaussianBlur(effect));
        Ok(())
    }

    fn apply_chroma(
        &mut self,
        filter: &Element,
        entry: &Element,
        item: &mut VideoItem,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let effect = ChromaKeyModifier {
            key_color: filter.property("key").map_or_else(
                || Ok(ChromaKeyModifier::default().key_color),
                |key| {
                    parse_mlt_color(key.rsplit('=').next().unwrap_or(key))
                        .map(TimelineValue::new_const)
                },
            )?,
            similarity: animated_scalar(
                filter.property("variance").unwrap_or("0=0.35"),
                self.fps,
                entry,
                |value| value,
            )?,
            ..Default::default()
        };
        push_raster(item, RasterModifierEffect::ChromaKey(effect));
        Ok(())
    }

    pub(super) fn apply_audio_effects(
        &mut self,
        entry: &Element,
        mut item: AudioItem,
    ) -> Result<AudioItem, Box<dyn Error + Send + Sync>> {
        for filter in entry
            .children_named("filter")
            .filter(|filter| filter.property("disable") != Some("1"))
        {
            match filter_id(filter) {
                "fadein" => {
                    item.transitions.intro = Some(AudioTransition::new(
                        TransitionSide::Intro,
                        filter_duration(filter, item.start, item.end, self.fps),
                    ));
                }
                "fadeout" => {
                    item.transitions.outro = Some(AudioTransition::new(
                        TransitionSide::Outro,
                        filter_duration(filter, item.start, item.end, self.fps),
                    ));
                }
                "gain" => {
                    let gain = filter.property("gain").unwrap_or("1").parse::<f32>()?;
                    item.modifiers
                        .push(AudioModifier::new(AudioModifierEffect::Gain(
                            GainModifier {
                                decibels: TimelineValue::new_const(
                                    20.0 * gain.max(f32::MIN_POSITIVE).log10(),
                                ),
                            },
                        )));
                }
                "volume" => {
                    let decibels = animated_scalar(
                        filter.property("level").unwrap_or("0=0"),
                        self.fps,
                        entry,
                        |value| value,
                    )?;
                    item.modifiers
                        .push(AudioModifier::new(AudioModifierEffect::Gain(
                            GainModifier { decibels },
                        )));
                }
                "" => {}
                unsupported => {
                    self.warnings.insert(format!(
                        "Kdenlive audio effect {unsupported} is unsupported and was skipped."
                    ));
                }
            }
        }
        if let (Some(intro), Some(outro)) =
            (&mut item.transitions.intro, &mut item.transitions.outro)
        {
            math::fit_durations(
                &mut intro.duration,
                &mut outro.duration,
                Time {
                    seconds: item.end.seconds - item.start.seconds,
                },
            );
        }
        Ok(item)
    }
}

fn qtblend_geometries(
    filters: &[&Element],
    fps: Fraction,
    canvas_size: Vec2,
) -> Result<Vec<QtblendGeometry>, Box<dyn Error + Send + Sync>> {
    filters
        .iter()
        .map(|filter| {
            let rects = filter.property("rect").map_or_else(
                || {
                    Ok(vec![math::Keyframe {
                        frame: 0,
                        value: math::RectValue {
                            rect: shrimply_math_geometry::Rect::from_xywh(
                                0.0,
                                0.0,
                                canvas_size.x,
                                canvas_size.y,
                            ),
                            opacity: 1.0,
                        },
                        interpolation: shrimply_project::Interpolation::Linear,
                    }])
                },
                |value| math::rect_animation(value, fps, canvas_size).map_err(invalid),
            )?;
            let rotations =
                math::scalar_animation(filter.property("rotation").unwrap_or("0=0"), fps)
                    .map_err(invalid)?;
            let rotation_anchors = math::point_animation(
                filter.property("rotate_anchor").unwrap_or(
                    if filter
                        .property("rotate_center")
                        .is_some_and(|value| value != "0")
                    {
                        "0.5 0.5"
                    } else {
                        "0 0"
                    },
                ),
                fps,
            )
            .map_err(invalid)?;
            Ok(QtblendGeometry {
                rects,
                rotations,
                rotation_anchors,
                distort: filter.property("distort") == Some("1"),
            })
        })
        .collect()
}

fn source_geometry(item: &VideoItem) -> SourceGeometry {
    let size = Vec2::new(
        item.source_width.max(1) as f32,
        item.source_height.max(1) as f32,
    );
    let anchor = size * 0.5;
    let rotation_degrees = item
        .default_transform
        .as_ref()
        .unwrap_or(&item.transform)
        .rotation_degrees
        .fallback();
    let quarter_turn = ((rotation_degrees / 90.0).round() as i32).rem_euclid(2) != 0;
    let oriented_size = if quarter_turn {
        Vec2::new(size.y, size.x)
    } else {
        size
    };
    SourceGeometry {
        size,
        oriented_size,
        anchor,
        orientation: Mat3::from_translation(oriented_size * 0.5)
            * Mat3::from_angle(rotation_degrees.to_radians())
            * Mat3::from_translation(-anchor),
        rotation_degrees,
    }
}

fn qtblend_matrix_at(
    geometries: &[QtblendGeometry],
    source: SourceGeometry,
    canvas_size: Vec2,
    frame: i64,
) -> (Mat3, f32, f32) {
    let mut matrix = source.orientation;
    let mut rotation_degrees = source.rotation_degrees;
    let mut opacity = 1.0;
    for (index, geometry) in geometries.iter().enumerate() {
        let rect = math::value_at(&geometry.rects, frame, lerp_rect);
        let rotation = math::value_at(&geometry.rotations, frame, lerp_f32);
        let rotation_anchor = math::value_at(&geometry.rotation_anchors, frame, Vec2::lerp);
        let input_size = if index == 0 {
            source.oriented_size
        } else {
            canvas_size
        };
        matrix = math::qtblend_transform(
            input_size,
            rect.rect,
            rotation,
            geometry.distort,
            rotation_anchor,
        ) * matrix;
        rotation_degrees += rotation;
        opacity *= rect.opacity;
    }
    (matrix, rotation_degrees, opacity)
}

fn transparent_color_animation(value: &str) -> bool {
    value
        .split(';')
        .filter(|keyframe| !keyframe.trim().is_empty())
        .all(|keyframe| {
            let value = keyframe
                .rsplit_once('=')
                .map_or(keyframe, |(_, value)| value)
                .trim()
                .trim_start_matches("0x")
                .trim_start_matches('#');
            value.len() == 8 && value[6..].eq_ignore_ascii_case("00")
        })
}

fn lerp_f32(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn lerp_rect(from: math::RectValue, to: math::RectValue, progress: f32) -> math::RectValue {
    math::RectValue {
        rect: shrimply_math_geometry::Rect::from_min_max(
            from.rect.min.lerp(to.rect.min, progress),
            from.rect.max.lerp(to.rect.max, progress),
        ),
        opacity: lerp_f32(from.opacity, to.opacity, progress),
    }
}

fn interpolation_at<T>(
    keyframes: &[math::Keyframe<T>],
    frame: i64,
) -> shrimply_project::Interpolation {
    keyframes
        .iter()
        .rev()
        .find(|keyframe| keyframe.frame <= frame)
        .unwrap_or(&keyframes[0])
        .interpolation
}

fn scalar_timeline(
    values: Vec<(Time, f32, shrimply_project::Interpolation)>,
) -> TimelineValue<f32> {
    if values.len() == 1 {
        return TimelineValue::new_const(values[0].1);
    }
    TimelineValue::new(TimelineBase::Keyframes(
        values
            .into_iter()
            .map(
                |(time, value, interpolation_to_next)| TimelineCurveKeyframe {
                    id: Uuid::new_v4(),
                    time,
                    value,
                    interpolation_to_next,
                },
            )
            .collect(),
    ))
}

fn vec2_timeline(
    values: Vec<(Time, Vec2, shrimply_project::Interpolation)>,
) -> TimelineValue<Vec2> {
    if values.len() == 1 {
        return TimelineValue::new_const(values[0].1);
    }
    TimelineValue::new(TimelineBase::Keyframes(
        values
            .into_iter()
            .map(
                |(time, value, interpolation_to_next)| TimelineCurveKeyframe {
                    id: Uuid::new_v4(),
                    time,
                    value,
                    interpolation_to_next,
                },
            )
            .collect(),
    ))
}

fn filter_id(filter: &Element) -> &str {
    filter
        .property("kdenlive_id")
        .or_else(|| filter.property("mlt_service"))
        .unwrap_or_default()
}

fn is_alpha_spot_mask(filter: &Element) -> bool {
    matches!(
        filter_id(filter),
        "mask_start-frei0r.alphaspot" | "mask_start-frei0r.alpha0ps_alphaspot"
    ) || matches!(
        filter.property("filter"),
        Some("frei0r.alphaspot" | "frei0r.alpha0ps_alphaspot")
    )
}

fn supports_modifier_alpha_mask(id: &str) -> bool {
    matches!(
        id,
        "qtcrop"
            | "avfilter.gblur"
            | "chroma"
            | "frei0r.saturat0r"
            | "frei0r.hueshift0r"
            | "lift_gamma_gain"
    )
}

fn qtblends_outside_effect_masks<'a>(
    entry: &'a Element,
    stop: Option<&Element>,
) -> Vec<&'a Element> {
    let mut inside_mask = false;
    let mut qtblends = Vec::new();
    for filter in entry.children_named("filter") {
        if stop.is_some_and(|stop| std::ptr::eq(filter, stop)) {
            break;
        }
        if filter.property("disable") == Some("1") {
            continue;
        }
        let id = filter_id(filter);
        if id.starts_with("mask_start") {
            inside_mask = true;
            continue;
        }
        if id == "mask_apply" {
            inside_mask = false;
            continue;
        }
        if !inside_mask && filter.property("mlt_service") == Some("qtblend") {
            qtblends.push(filter);
        }
    }
    qtblends
}

fn constant_scalar_animation(value: &str, fps: Fraction) -> Result<Option<f32>, String> {
    let values = math::scalar_animation(value, fps)?;
    let first = values[0].value;
    Ok(values
        .iter()
        .all(|keyframe| keyframe.value == first)
        .then_some(first))
}

fn push_raster(item: &mut VideoItem, effect: RasterModifierEffect) {
    item.modifiers
        .push(VisualModifier::new(ModifierEffect::Raster(Box::new(
            effect,
        ))));
}

fn animated_vec2(
    first: &str,
    second: &str,
    fps: Fraction,
    entry: &Element,
    map: impl Fn(Vec2) -> Vec2,
) -> Result<TimelineValue<Vec2>, Box<dyn Error + Send + Sync>> {
    let entry_in = entry_in(entry, fps)?;
    Ok(vec2_timeline(
        math::scalar_pair_animation(first, second, fps)
            .map_err(invalid)?
            .into_iter()
            .map(|keyframe| {
                (
                    frame_time(keyframe.frame - entry_in, fps),
                    map(keyframe.value),
                    keyframe.interpolation,
                )
            })
            .collect(),
    ))
}

fn animated_scalar(
    value: &str,
    fps: Fraction,
    entry: &Element,
    map: impl Fn(f32) -> f32,
) -> Result<TimelineValue<f32>, Box<dyn Error + Send + Sync>> {
    let entry_in = entry_in(entry, fps)?;
    let values = math::scalar_animation(value, fps)
        .map_err(invalid)?
        .into_iter()
        .map(|keyframe| {
            (
                Time {
                    seconds: frame_time(keyframe.frame - entry_in, fps).seconds,
                },
                map(keyframe.value),
                keyframe.interpolation,
            )
        })
        .collect();
    Ok(scalar_timeline(values))
}

fn filter_duration(filter: &Element, start: Time, end: Time, fps: Fraction) -> Time {
    let Some(filter_end) = filter.attribute("out") else {
        return Time {
            seconds: end.seconds - start.seconds,
        };
    };
    match (
        math::parse_frame(filter.attribute("in").unwrap_or("0"), fps),
        math::parse_frame(filter_end, fps),
    ) {
        (Ok(filter_start), Ok(filter_end)) => frame_time(filter_end - filter_start + 1, fps),
        _ => Time {
            seconds: end.seconds - start.seconds,
        },
    }
}

fn parse_mlt_color(value: &str) -> Result<Color<u8>, Box<dyn Error + Send + Sync>> {
    let hex = value
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches('#');
    if hex.len() != 8 {
        return Err(invalid("MLT color must contain RGBA bytes"));
    }
    Ok(Color::<u8>::from_rgba(
        u8::from_str_radix(&hex[0..2], 16)?,
        u8::from_str_radix(&hex[2..4], 16)?,
        u8::from_str_radix(&hex[4..6], 16)?,
        u8::from_str_radix(&hex[6..8], 16)?,
    ))
}
