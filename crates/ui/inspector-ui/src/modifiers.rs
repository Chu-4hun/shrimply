use gtk::prelude::*;
use shrimply_ui_foundation::tr;
use std::rc::Rc;
use uuid::Uuid;

use super::timeline_value::color::{ColorAccess, ColorTarget, color_control};
use super::timeline_value::scalar::{ScalarAccess, ScalarSpec, ScalarTarget, scalar_control};
use super::timeline_value::vector::vec2::{VecAccess, VecSpec, VecTarget, vec_control};
use super::timeline_value::vector::vec3::{Vec3Target, control as vec3_control};
use crate::InspectedItem as SelectedItem;
use crate::player_state::{self, ProjectChange};
use crate::preview_focus::PreviewTarget;
use shrimply_core::timeline_value::{TimelineStep, TimelineValue};
use shrimply_project::project::{Project, Time};
use shrimply_project::project::{VisualAlphaMaskTarget, VisualItem, VisualModifier};
use shrimply_video_modifiers::MODIFIER_PREVIEW_FACET;
use shrimply_video_modifiers::{
    ModifierEffect, ModifierModel, RasterModifierEffect, VectorModifierEffect, VisualKind,
    scene_3d::Scene3dModifierEffect,
};

use super::{
    InspectorContext,
    item::{DefaultInspectorItem, HeaderAction, HeaderToggle, InspectorListItem, flat},
};

fn step_row<T: TimelineStep>(
    label: &str,
    value: &TimelineValue<T>,
    modifier_id: Uuid,
    context: &InspectorContext,
    commit_name: &'static str,
    get: fn(&ModifierEffect) -> Option<&TimelineValue<T>>,
    get_mut: fn(&mut ModifierEffect) -> Option<&mut TimelineValue<T>>,
) -> gtk::Widget {
    crate::timeline_value::step::step_control(
        label,
        value,
        context,
        crate::timeline_value::step::StepTarget::new(
            move |project, key| {
                project
                    .video_item(&key)?
                    .modifiers
                    .iter()
                    .find(|modifier| modifier.id == modifier_id)
                    .and_then(|modifier| get(&modifier.effect))
            },
            move |project, key| {
                project
                    .video_item_mut(&key)?
                    .modifiers
                    .iter_mut()
                    .find(|modifier| modifier.id == modifier_id)
                    .and_then(|modifier| get_mut(&mut modifier.effect))
            },
            commit_name,
            ProjectChange {
                video: true,
                inspector: true,
                ..Default::default()
            },
        ),
    )
}

#[path = "modifiers/add_menu.rs"]
mod add_menu;

#[path = "modifiers/alpha_outline.rs"]
mod alpha_outline;
#[path = "modifiers/bulge_pinch.rs"]
mod bulge_pinch;
#[path = "modifiers/cache.rs"]
mod cache;
#[path = "modifiers/channel_mixer.rs"]
mod channel_mixer;
#[path = "modifiers/chroma_key.rs"]
mod chroma_key;
#[path = "modifiers/chromatic_aberration.rs"]
mod chromatic_aberration;
#[path = "modifiers/color_correction.rs"]
mod color_correction;
#[path = "modifiers/colorize_duotone.rs"]
mod colorize_duotone;
#[path = "modifiers/corner_pin.rs"]
mod corner_pin;
#[path = "modifiers/crop.rs"]
mod crop;
#[path = "modifiers/directional_blur.rs"]
mod directional_blur;
#[path = "modifiers/displacement_map.rs"]
mod displacement_map;
#[path = "modifiers/dithering.rs"]
mod dithering;
#[path = "modifiers/drop_shadow.rs"]
mod drop_shadow;
#[path = "modifiers/edge_detection.rs"]
mod edge_detection;
#[path = "modifiers/emboss.rs"]
mod emboss;
#[path = "modifiers/erode_dilate.rs"]
mod erode_dilate;
#[path = "modifiers/film_grain.rs"]
mod film_grain;
#[path = "modifiers/fisheye.rs"]
mod fisheye;
#[path = "modifiers/gaussian_blur.rs"]
mod gaussian_blur;
#[path = "modifiers/glow_bloom.rs"]
mod glow_bloom;
#[path = "modifiers/ground.rs"]
mod ground;
#[path = "modifiers/halftone.rs"]
mod halftone;
#[path = "modifiers/hsv.rs"]
mod hsv;
#[path = "modifiers/invert.rs"]
mod invert;
#[path = "modifiers/kaleidoscope.rs"]
mod kaleidoscope;
#[path = "modifiers/kuwahara.rs"]
mod kuwahara;
#[path = "modifiers/lens_distortion.rs"]
mod lens_distortion;
#[path = "modifiers/luma_key.rs"]
mod luma_key;
#[path = "modifiers/mask.rs"]
mod mask;
#[path = "modifiers/mirror.rs"]
mod mirror;
#[path = "modifiers/object_3d.rs"]
mod object_3d;
#[path = "modifiers/opacity.rs"]
mod opacity;
#[path = "modifiers/path_offset.rs"]
mod path_offset;
#[path = "modifiers/pixelate_mosaic.rs"]
mod pixelate_mosaic;
#[path = "modifiers/point_light.rs"]
mod point_light;
#[path = "modifiers/posterize.rs"]
mod posterize;
#[path = "modifiers/radial_blur.rs"]
mod radial_blur;
#[path = "modifiers/rasterize.rs"]
mod rasterize;
#[path = "modifiers/repeat.rs"]
mod repeat;
#[path = "modifiers/sam2.rs"]
mod sam2;
#[path = "modifiers/sampling.rs"]
mod sampling;
#[path = "modifiers/scanlines_crt.rs"]
mod scanlines_crt;
#[path = "modifiers/shaky_path.rs"]
mod shaky_path;
#[path = "modifiers/shape_3d.rs"]
mod shape_3d;
#[path = "modifiers/sharpen.rs"]
mod sharpen;
#[path = "modifiers/sun_light.rs"]
mod sun_light;
#[path = "modifiers/text_3d.rs"]
mod text_3d;
#[path = "modifiers/text_mask.rs"]
mod text_mask;
#[path = "modifiers/texture_bounds.rs"]
mod texture_bounds;
#[path = "modifiers/threshold.rs"]
mod threshold;
#[path = "modifiers/transform.rs"]
mod transform;
#[path = "modifiers/transparent_fill.rs"]
mod transparent_fill;
#[path = "modifiers/twirl.rs"]
mod twirl;
#[path = "modifiers/vectorize.rs"]
mod vectorize;
#[path = "modifiers/vignette.rs"]
mod vignette;
#[path = "modifiers/wave_ripple.rs"]
mod wave_ripple;
#[path = "modifiers/zoom_blur.rs"]
mod zoom_blur;

pub(super) fn items(item: &VisualItem, context: &InspectorContext) -> Vec<InspectorListItem> {
    let mut items = item
        .modifiers
        .iter()
        .enumerate()
        .map(|(index, modifier)| modifier_item(modifier, index, item.modifiers.len(), context))
        .collect::<Vec<_>>();
    items.push(flat(modifier_buttons(context)));
    items
}

fn modifier_item(
    modifier: &VisualModifier,
    index: usize,
    len: usize,
    context: &InspectorContext,
) -> InspectorListItem {
    let key = format!("modifier:{}", modifier.id);
    let display_name = modifier.effect.display_name();
    let actions = modifier_actions(modifier.id, index, len, context);
    let alpha_mask_target = matches!(
        modifier.effect,
        ModifierEffect::Raster(ref effect) if !matches!(&**effect, RasterModifierEffect::Cache(_))
    )
    .then_some(VisualAlphaMaskTarget::Modifier(modifier.id));
    macro_rules! item {
        ($value:expr, $module:ident, $wrap:expr) => {{
            let id = modifier.id;
            let item = DefaultInspectorItem::new(
                key.clone(),
                display_name,
                $value.clone(),
                move |value, context| {
                    let out = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    $module::add_rows(value, &out, id, context);
                    if let Some(target) = alpha_mask_target {
                        out.append(&crate::alpha_mask::widget(target, context));
                    }
                    vec![out.upcast()]
                },
                move |context, value| reset_modifier(context, id, $wrap(value)),
            );
            let mut item = item.preview_target(PreviewTarget::new(id, MODIFIER_PREVIEW_FACET));
            if let Some(target) = alpha_mask_target {
                item = item.button_toggle(crate::alpha_mask::button_toggle(target, context));
            }
            item.toggle(modifier_toggle(modifier, context))
                .actions(actions)
                .boxed()
        }};
    }
    match &modifier.effect {
        ModifierEffect::Scene3d(effect) => match &**effect {
            Scene3dModifierEffect::Object(value) => {
                item!(value, object_3d, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::Object(value)
                ))
            }
            Scene3dModifierEffect::Text(value) => {
                item!(value, text_3d, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::Text(value)
                ))
            }
            Scene3dModifierEffect::Shape(value) => {
                item!(value, shape_3d, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::Shape(value)
                ))
            }
            Scene3dModifierEffect::Ground(value) => {
                item!(value, ground, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::Ground(value)
                ))
            }
            Scene3dModifierEffect::PointLight(value) => {
                item!(value, point_light, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::PointLight(value)
                ))
            }
            Scene3dModifierEffect::SunLight(value) => {
                item!(value, sun_light, |value| ModifierEffect::scene_3d(
                    Scene3dModifierEffect::SunLight(value)
                ))
            }
        },
        ModifierEffect::Vector(effect) => match &**effect {
            VectorModifierEffect::Transform(value) => {
                item!(value, transform, |value| ModifierEffect::vector(
                    VectorModifierEffect::Transform(value)
                ))
            }
            VectorModifierEffect::Repeat(value) => {
                item!(value, repeat, |value| ModifierEffect::vector(
                    VectorModifierEffect::Repeat(value)
                ))
            }
            VectorModifierEffect::ShakyPath(value) => {
                item!(value, shaky_path, |value| ModifierEffect::vector(
                    VectorModifierEffect::ShakyPath(value)
                ))
            }
            VectorModifierEffect::PathOffset(value) => {
                item!(value, path_offset, |value| ModifierEffect::vector(
                    VectorModifierEffect::PathOffset(value)
                ))
            }
            VectorModifierEffect::Opacity(value) => {
                item!(value, opacity, |value| ModifierEffect::vector(
                    VectorModifierEffect::Opacity(value)
                ))
            }
            VectorModifierEffect::Hsv(value) => {
                item!(value, hsv, |value| ModifierEffect::vector(
                    VectorModifierEffect::Hsv(value)
                ))
            }
            VectorModifierEffect::TextMask(value) => {
                item!(value, text_mask, |value| ModifierEffect::vector(
                    VectorModifierEffect::TextMask(value)
                ))
            }
        },
        ModifierEffect::Vectorize(value) => item!(value, vectorize, ModifierEffect::Vectorize),
        ModifierEffect::Rasterize(value) => item!(value, rasterize, ModifierEffect::Rasterize),
        ModifierEffect::Raster(effect) => match &**effect {
            RasterModifierEffect::Cache(value) => {
                item!(value, cache, |value| ModifierEffect::raster(
                    RasterModifierEffect::Cache(value)
                ))
            }
            RasterModifierEffect::Transform(value) => {
                item!(value, transform, |value| ModifierEffect::raster(
                    RasterModifierEffect::Transform(value)
                ))
            }
            RasterModifierEffect::TextureBounds(value) => {
                item!(value, texture_bounds, |value| ModifierEffect::raster(
                    RasterModifierEffect::TextureBounds(value)
                ))
            }
            RasterModifierEffect::Sampling(value) => {
                item!(value, sampling, |value| ModifierEffect::raster(
                    RasterModifierEffect::Sampling(value)
                ))
            }
            RasterModifierEffect::Crop(value) => {
                item!(value, crop, |value| ModifierEffect::raster(
                    RasterModifierEffect::Crop(value)
                ))
            }
            RasterModifierEffect::CornerPin(value) => {
                item!(value, corner_pin, |value| ModifierEffect::raster(
                    RasterModifierEffect::CornerPin(value)
                ))
            }
            RasterModifierEffect::Opacity(value) => {
                item!(value, opacity, |value| ModifierEffect::raster(
                    RasterModifierEffect::Opacity(value)
                ))
            }
            RasterModifierEffect::ChromaKey(value) => {
                item!(value, chroma_key, |value| ModifierEffect::raster(
                    RasterModifierEffect::ChromaKey(value)
                ))
            }
            RasterModifierEffect::Kuwahara(value) => {
                item!(value, kuwahara, |value| ModifierEffect::raster(
                    RasterModifierEffect::Kuwahara(value)
                ))
            }
            RasterModifierEffect::GaussianBlur(value) => {
                item!(value, gaussian_blur, |value| ModifierEffect::raster(
                    RasterModifierEffect::GaussianBlur(value)
                ))
            }
            RasterModifierEffect::Fisheye(value) => {
                item!(value, fisheye, |value| ModifierEffect::raster(
                    RasterModifierEffect::Fisheye(value)
                ))
            }
            RasterModifierEffect::Sharpen(value) => {
                item!(value, sharpen, |value| ModifierEffect::raster(
                    RasterModifierEffect::Sharpen(value)
                ))
            }
            RasterModifierEffect::Vignette(value) => {
                item!(value, vignette, |value| ModifierEffect::raster(
                    RasterModifierEffect::Vignette(value)
                ))
            }
            RasterModifierEffect::PixelateMosaic(value) => {
                item!(value, pixelate_mosaic, |value| ModifierEffect::raster(
                    RasterModifierEffect::PixelateMosaic(value)
                ))
            }
            RasterModifierEffect::Posterize(value) => {
                item!(value, posterize, |value| ModifierEffect::raster(
                    RasterModifierEffect::Posterize(value)
                ))
            }
            RasterModifierEffect::Threshold(value) => {
                item!(value, threshold, |value| ModifierEffect::raster(
                    RasterModifierEffect::Threshold(value)
                ))
            }
            RasterModifierEffect::FilmGrain(value) => {
                item!(value, film_grain, |value| ModifierEffect::raster(
                    RasterModifierEffect::FilmGrain(value)
                ))
            }
            RasterModifierEffect::ChromaticAberration(value) => {
                item!(value, chromatic_aberration, |value| ModifierEffect::raster(
                    RasterModifierEffect::ChromaticAberration(value)
                ))
            }
            RasterModifierEffect::EdgeDetection(value) => {
                item!(value, edge_detection, |value| ModifierEffect::raster(
                    RasterModifierEffect::EdgeDetection(value)
                ))
            }
            RasterModifierEffect::Emboss(value) => {
                item!(value, emboss, |value| ModifierEffect::raster(
                    RasterModifierEffect::Emboss(value)
                ))
            }
            RasterModifierEffect::DirectionalBlur(value) => {
                item!(value, directional_blur, |value| ModifierEffect::raster(
                    RasterModifierEffect::DirectionalBlur(value)
                ))
            }
            RasterModifierEffect::Dithering(value) => {
                item!(value, dithering, |value| ModifierEffect::raster(
                    RasterModifierEffect::Dithering(value)
                ))
            }
            RasterModifierEffect::GlowBloom(value) => {
                item!(value, glow_bloom, |value| ModifierEffect::raster(
                    RasterModifierEffect::GlowBloom(value)
                ))
            }
            RasterModifierEffect::Twirl(value) => {
                item!(value, twirl, |value| ModifierEffect::raster(
                    RasterModifierEffect::Twirl(value)
                ))
            }
            RasterModifierEffect::BulgePinch(value) => {
                item!(value, bulge_pinch, |value| ModifierEffect::raster(
                    RasterModifierEffect::BulgePinch(value)
                ))
            }
            RasterModifierEffect::WaveRipple(value) => {
                item!(value, wave_ripple, |value| ModifierEffect::raster(
                    RasterModifierEffect::WaveRipple(value)
                ))
            }
            RasterModifierEffect::Mirror(value) => {
                item!(value, mirror, |value| ModifierEffect::raster(
                    RasterModifierEffect::Mirror(value)
                ))
            }
            RasterModifierEffect::Kaleidoscope(value) => {
                item!(value, kaleidoscope, |value| ModifierEffect::raster(
                    RasterModifierEffect::Kaleidoscope(value)
                ))
            }
            RasterModifierEffect::ColorizeDuotone(value) => {
                item!(value, colorize_duotone, |value| ModifierEffect::raster(
                    RasterModifierEffect::ColorizeDuotone(value)
                ))
            }
            RasterModifierEffect::Invert(value) => {
                item!(value, invert, |value| ModifierEffect::raster(
                    RasterModifierEffect::Invert(value)
                ))
            }
            RasterModifierEffect::ChannelMixer(value) => {
                item!(value, channel_mixer, |value| ModifierEffect::raster(
                    RasterModifierEffect::ChannelMixer(value)
                ))
            }
            RasterModifierEffect::AlphaOutline(value) => {
                item!(value, alpha_outline, |value| ModifierEffect::raster(
                    RasterModifierEffect::AlphaOutline(value)
                ))
            }
            RasterModifierEffect::DropShadow(value) => {
                item!(value, drop_shadow, |value| ModifierEffect::raster(
                    RasterModifierEffect::DropShadow(value)
                ))
            }
            RasterModifierEffect::Halftone(value) => {
                item!(value, halftone, |value| ModifierEffect::raster(
                    RasterModifierEffect::Halftone(value)
                ))
            }
            RasterModifierEffect::ScanlinesCrt(value) => {
                item!(value, scanlines_crt, |value| ModifierEffect::raster(
                    RasterModifierEffect::ScanlinesCrt(value)
                ))
            }
            RasterModifierEffect::LensDistortion(value) => {
                item!(value, lens_distortion, |value| ModifierEffect::raster(
                    RasterModifierEffect::LensDistortion(value)
                ))
            }
            RasterModifierEffect::DisplacementMap(value) => {
                item!(value, displacement_map, |value| ModifierEffect::raster(
                    RasterModifierEffect::DisplacementMap(value)
                ))
            }
            RasterModifierEffect::LumaKey(value) => {
                item!(value, luma_key, |value| ModifierEffect::raster(
                    RasterModifierEffect::LumaKey(value)
                ))
            }
            RasterModifierEffect::Mask(value) => {
                item!(value, mask, |value| ModifierEffect::raster(
                    RasterModifierEffect::Mask(value)
                ))
            }
            RasterModifierEffect::Sam2(value) => {
                item!(value, sam2, |value| ModifierEffect::raster(
                    RasterModifierEffect::Sam2(value)
                ))
            }
            RasterModifierEffect::TransparentFill(value) => {
                item!(value, transparent_fill, |value| ModifierEffect::raster(
                    RasterModifierEffect::TransparentFill(value)
                ))
            }
            RasterModifierEffect::RadialBlur(value) => {
                item!(value, radial_blur, |value| ModifierEffect::raster(
                    RasterModifierEffect::RadialBlur(value)
                ))
            }
            RasterModifierEffect::ZoomBlur(value) => {
                item!(value, zoom_blur, |value| ModifierEffect::raster(
                    RasterModifierEffect::ZoomBlur(value)
                ))
            }
            RasterModifierEffect::ErodeDilate(value) => {
                item!(value, erode_dilate, |value| ModifierEffect::raster(
                    RasterModifierEffect::ErodeDilate(value)
                ))
            }
            RasterModifierEffect::ColorCorrection(value) => {
                item!(value, color_correction, |value| ModifierEffect::raster(
                    RasterModifierEffect::ColorCorrection(value)
                ))
            }
        },
    }
}

fn modifier_toggle(modifier: &VisualModifier, context: &InspectorContext) -> HeaderToggle {
    let id = modifier.id;
    let context = context.detached();
    HeaderToggle {
        active: modifier.enabled,
        tooltip: if modifier.enabled {
            "Disable modifier"
        } else {
            "Enable modifier"
        },
        activate: Rc::new(move |enabled| set_enabled(id, enabled, &context)),
    }
}

fn set_enabled(id: Uuid, enabled: bool, context: &InspectorContext) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let Some(index) = item.modifiers.iter().position(|modifier| modifier.id == id) else {
        return;
    };
    if item.modifiers[index].enabled == enabled {
        return;
    }
    item.modifiers[index].enabled = enabled;
    if !modifier_chain_is_valid(item, &item.modifiers) {
        item.modifiers[index].enabled = !enabled;
        drop(project);
        (context.refresh)();
        return;
    }
    shrimply_project::project::commit_edit(&project, "toggle-visual-modifier");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

#[derive(Clone, Copy)]
enum Action {
    Copy,
    Up,
    Down,
    Remove,
}

fn modifier_actions(
    id: Uuid,
    index: usize,
    len: usize,
    context: &InspectorContext,
) -> Vec<HeaderAction> {
    [
        ("edit-copy-symbolic", "Copy", Action::Copy, true),
        ("go-up-symbolic", "Move up", Action::Up, index > 0),
        (
            "go-down-symbolic",
            "Move down",
            Action::Down,
            index + 1 < len,
        ),
        ("user-trash-symbolic", "Remove", Action::Remove, true),
    ]
    .into_iter()
    .map(|(icon, tooltip, action, sensitive)| {
        let sensitive = sensitive && action_keeps_chain_valid(id, action, context);
        let context = context.detached();
        HeaderAction {
            icon,
            tooltip,
            sensitive,
            activate: Rc::new(move || apply_action(id, action, &context)),
        }
    })
    .collect()
}

fn action_keeps_chain_valid(id: Uuid, action: Action, context: &InspectorContext) -> bool {
    let Some(key) = context.selected_item.clone() else {
        return false;
    };
    let project = context.project.borrow();
    let Some(item) = project.video_item(&key) else {
        return false;
    };
    let mut modifiers = item.modifiers.clone();
    let Some(index) = modifiers.iter().position(|modifier| modifier.id == id) else {
        return false;
    };
    match action {
        Action::Copy => return true,
        Action::Up if index > 0 => modifiers.swap(index, index - 1),
        Action::Down if index + 1 < modifiers.len() => modifiers.swap(index, index + 1),
        Action::Remove => {
            modifiers.remove(index);
        }
        _ => return false,
    }
    modifier_chain_is_valid(item, &modifiers)
}

fn apply_action(id: Uuid, action: Action, context: &InspectorContext) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let project = context.project.clone();
    let mut project = project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    if matches!(action, Action::Copy) {
        let Some(modifier) = item.modifiers.iter().find(|modifier| modifier.id == id) else {
            return;
        };
        context
            .property_clipboard
            .borrow_mut()
            .copy_visual_modifier(modifier);
        let message = shrimply_ui_foundation::i18n::text_args(
            "%{name} copied",
            &[("name", modifier.effect.display_name().to_owned())],
        );
        shrimply_ui_foundation::toast::show_confirmation_text_for_widget(
            &context.category_bar,
            &message,
        );
        drop(project);
        (context.refresh)();
        return;
    }
    let mut modifiers = item.modifiers.clone();
    let Some(index) = modifiers.iter().position(|modifier| modifier.id == id) else {
        return;
    };
    if matches!(action, Action::Remove)
        && matches!(
            modifiers[index].effect,
            ModifierEffect::Raster(ref effect)
                if matches!(&**effect, RasterModifierEffect::Cache(_))
        )
        && let Err(error) = shrimply_video::modifier_cache::invalidate(id)
    {
        shrimply_ui_foundation::toast::show_confirmation_text_for_widget(
            &context.category_bar,
            &format!("Could not remove cache: {error}"),
        );
        return;
    }
    match action {
        Action::Copy => unreachable!(),
        Action::Up if index > 0 => modifiers.swap(index, index - 1),
        Action::Down if index + 1 < modifiers.len() => modifiers.swap(index, index + 1),
        Action::Remove => {
            modifiers.remove(index);
        }
        _ => return,
    }
    if !modifier_chain_is_valid(item, &modifiers) {
        return;
    }
    item.modifiers = modifiers;
    shrimply_project::project::commit_edit(
        &project,
        match action {
            Action::Copy => unreachable!(),
            Action::Up | Action::Down => "move-visual-modifier",
            Action::Remove => "remove-visual-modifier",
        },
    );
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn modifier_buttons(context: &InspectorContext) -> gtk::Widget {
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.set_halign(gtk::Align::Center);
    buttons.append(&add_menu::button(context));
    let targets = context
        .selected_item
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    let sensitive = {
        let project = context.project.borrow();
        context
            .property_clipboard
            .borrow()
            .can_append_modifiers(&project, &targets)
    };
    if sensitive {
        let paste = gtk::Button::builder()
            .icon_name("edit-paste-symbolic")
            .tooltip_text(tr!("Paste Modifier").as_ref())
            .build();
        let context = context.detached();
        paste.connect_clicked(move |_| paste_modifiers(&context));
        buttons.append(&paste);
    }
    buttons.upcast()
}

fn paste_modifiers(context: &InspectorContext) {
    let Some(target) = context.selected_item.clone() else {
        return;
    };
    let result = {
        let mut project = context.project.borrow_mut();
        let result = context
            .property_clipboard
            .borrow()
            .append_modifiers(&mut project, &[target]);
        if result.changed {
            shrimply_project::project::commit_edit(&project, "paste-item-modifiers");
        }
        result
    };
    if !result.changed {
        return;
    }
    let message = if result.modifiers_added == 1 {
        tr!("1 effect pasted").into_owned()
    } else {
        shrimply_ui_foundation::i18n::text_args(
            "%{count} effects pasted",
            &[("count", result.modifiers_added.to_string())],
        )
    };
    shrimply_ui_foundation::toast::show_confirmation_text_for_widget(
        &context.category_bar,
        &message,
    );
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: result.video,
            inspector: true,
            ..Default::default()
        },
    );
}

fn reset_modifier(context: &InspectorContext, id: Uuid, effect: ModifierEffect) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(modifier) = project
        .video_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
    else {
        return;
    };
    modifier.effect = effect;
    modifier.alpha_mask = None;
    shrimply_project::project::commit_edit(&project, "reset-visual-modifier");
    drop(project);
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

pub(super) fn add_effect(effect: ModifierEffect, context: &InspectorContext) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.video_item_mut(&key) else {
        return;
    };
    let modifier = VisualModifier::new(effect);
    let id = modifier.id;
    let mut modifiers = item.modifiers.clone();
    modifiers.push(modifier);
    if !modifier_chain_is_valid(item, &modifiers) {
        return;
    }
    item.modifiers = modifiers;
    shrimply_project::project::commit_edit(&project, "add-visual-modifier");
    drop(project);
    if let Some(target) = &context.expansion_target {
        context
            .expanded_rows
            .borrow_mut()
            .insert((target.clone(), format!("modifier:{id}")), true);
    }
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn modifier_chain_is_valid(item: &VisualItem, modifiers: &[VisualModifier]) -> bool {
    let Ok(state) = item.modifier_output_state_for(modifiers) else {
        return false;
    };
    !item
        .compositing
        .alpha_mask
        .as_ref()
        .is_some_and(|mask| mask.enabled)
        || state.kind == VisualKind::Raster
}

#[derive(Clone, Copy)]
pub(crate) struct ScalarOptions {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub unit: Option<&'static str>,
    pub rotating: bool,
}

pub(crate) fn scalar_row(
    label: &str,
    value: &TimelineValue<f32>,
    modifier_id: Uuid,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row_for::<ContinuousScalar>(label, value, modifier_id, options, context)
}

pub(crate) fn audio_scalar_row(
    label: &str,
    value: &TimelineValue<f32>,
    modifier_id: Uuid,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    audio_scalar_row_with_access(
        label,
        value,
        ScalarAccess::AudioModifier {
            id: modifier_id,
            value_id: value.id,
        },
        options,
        false,
        context,
    )
}

pub(crate) fn audio_item_scalar_row(
    label: &str,
    value: &TimelineValue<f32>,
    get: crate::timeline_value::scalar::ScalarGet,
    get_mut: crate::timeline_value::scalar::ScalarGetMut,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    audio_scalar_row_with_access(
        label,
        value,
        ScalarAccess::Item { get, get_mut },
        options,
        false,
        context,
    )
}

pub(crate) fn audio_item_integer_scalar_row(
    label: &str,
    value: &TimelineValue<f32>,
    get: crate::timeline_value::scalar::ScalarGet,
    get_mut: crate::timeline_value::scalar::ScalarGetMut,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    audio_scalar_row_with_access(
        label,
        value,
        ScalarAccess::Item { get, get_mut },
        options,
        true,
        context,
    )
}

fn audio_scalar_row_with_access(
    label: &str,
    value: &TimelineValue<f32>,
    access: ScalarAccess,
    options: ScalarOptions,
    integer: bool,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    let percentage = options.unit == Some("%")
        && options.minimum.is_some_and(|minimum| minimum >= 0.0)
        && options.maximum.is_some_and(|maximum| maximum <= 1.0);
    scalar_control(
        label,
        value,
        &context,
        ScalarTarget {
            access,
            scope_id: Some(value.id),
            local_time: audio_local_time,
            global_time: audio_global_time,
            duration: audio_duration,
            refresh: ProjectChange {
                audio: true,
                audio_waveforms: true,
                ..Default::default()
            },
            commit_name: "audio-modifier-value",
        },
        ScalarSpec {
            drag_step: if percentage || integer { 1.0 } else { 0.01 },
            digits: if percentage || integer { 0 } else { 2 },
            integer,
            width_chars: 8,
            minimum: if percentage {
                options.minimum.map(|minimum| minimum * 100.0)
            } else {
                options.minimum
            },
            maximum: if percentage {
                options.maximum.map(|maximum| maximum * 100.0)
            } else {
                options.maximum
            },
            unit_name: options.unit,
            rotating_icon: None,
            display: if percentage {
                |value| f64::from(value) * 100.0
            } else {
                f64::from
            },
            store: if percentage {
                |value| (value / 100.0) as f32
            } else {
                |value| value as f32
            },
            clamp: |value| value,
        },
    )
}

pub(crate) fn integer_scalar_row(
    label: &str,
    value: &TimelineValue<f32>,
    modifier_id: Uuid,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row_for::<IntegerScalar>(label, value, modifier_id, options, context)
}

trait ScalarMode {
    const DRAG_STEP: f64;
    const DIGITS: usize;
    const INTEGER: bool;
    fn clamp(options: ScalarOptions) -> fn(f32) -> f32;
}

struct ContinuousScalar;

impl ScalarMode for ContinuousScalar {
    const DRAG_STEP: f64 = 0.01;
    const DIGITS: usize = 2;
    const INTEGER: bool = false;

    fn clamp(options: ScalarOptions) -> fn(f32) -> f32 {
        match (options.minimum, options.maximum) {
            (Some(0.0), Some(1.0)) => |value| value.clamp(0.0, 1.0),
            (Some(0.0), Some(2.0)) => |value| value.clamp(0.0, 2.0),
            (Some(0.0), Some(100.0)) => |value| value.clamp(0.0, 100.0),
            (Some(-1.0), Some(1.0)) => |value| value.clamp(-1.0, 1.0),
            _ => |value| value,
        }
    }
}

struct IntegerScalar;

impl ScalarMode for IntegerScalar {
    const DRAG_STEP: f64 = 1.0;
    const DIGITS: usize = 0;
    const INTEGER: bool = true;

    fn clamp(_: ScalarOptions) -> fn(f32) -> f32 {
        |value| value.round()
    }
}

fn scalar_row_for<M: ScalarMode>(
    label: &str,
    value: &TimelineValue<f32>,
    modifier_id: Uuid,
    options: ScalarOptions,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    scalar_control(
        label,
        value,
        &context,
        ScalarTarget {
            access: ScalarAccess::Modifier {
                id: modifier_id,
                value_id: value.id,
            },
            scope_id: Some(value.id),
            local_time: visual_local_time,
            global_time: visual_global_time,
            duration: visual_duration,
            refresh: ProjectChange {
                video: true,
                ..Default::default()
            },
            commit_name: "visual-modifier-value",
        },
        ScalarSpec {
            drag_step: if options.rotating { 1.0 } else { M::DRAG_STEP },
            digits: M::DIGITS,
            integer: M::INTEGER,
            width_chars: 8,
            minimum: options.minimum,
            maximum: options.maximum,
            unit_name: options.unit,
            rotating_icon: options.rotating.then_some(("arrow3-up-symbolic", 0.0)),
            display: |v| v as f64,
            store: |v| v as f32,
            clamp: M::clamp(options),
        },
    )
}

fn visual_local_time(p: &Project, k: SelectedItem, t: Time) -> Option<Time> {
    crate::video::visual_local_time(p, k, t)
}
fn visual_global_time(p: &Project, k: SelectedItem, t: Time) -> Option<Time> {
    crate::video::visual_global_time(p, k, t)
}
fn visual_duration(p: &Project, k: SelectedItem) -> Option<Time> {
    crate::video::visual_duration(p, k)
}

fn audio_local_time(p: &Project, k: SelectedItem, t: Time) -> Option<Time> {
    let t = p.timeline_time_to_sequence(&k.track(), t)?;
    let item = p.audio_item(&k)?;
    Some(t.saturating_sub(item.start))
}

fn audio_global_time(p: &Project, k: SelectedItem, t: Time) -> Option<Time> {
    let item = p.audio_item(&k)?;
    p.sequence_time_to_timeline(&k.track(), item.start.saturating_add(t))
}

fn audio_duration(p: &Project, k: SelectedItem) -> Option<Time> {
    let (start, end) = p.projected_item_times(&k)?;
    let track = k.track();
    let start = p.timeline_time_to_sequence(&track, start)?;
    let end = p.timeline_time_to_sequence(&track, end)?;
    Some(end.saturating_sub(start).max(start.saturating_sub(end)))
}

pub(crate) fn number(modifier: &VisualModifier, id: Uuid) -> Option<&TimelineValue<f32>> {
    modifier.number(id)
}
pub(crate) fn number_mut(
    modifier: &mut VisualModifier,
    id: Uuid,
) -> Option<&mut TimelineValue<f32>> {
    modifier.number_mut(id)
}
pub(crate) fn number2(modifier: &VisualModifier, id: Uuid) -> Option<&TimelineValue<glam::Vec2>> {
    modifier.number2(id)
}
pub(crate) fn number2_mut(
    modifier: &mut VisualModifier,
    id: Uuid,
) -> Option<&mut TimelineValue<glam::Vec2>> {
    modifier.number2_mut(id)
}
pub(crate) fn number3(effect: &ModifierEffect, id: Uuid) -> Option<&TimelineValue<glam::Vec3>> {
    effect.number3(id)
}
pub(crate) fn number3_mut(
    effect: &mut ModifierEffect,
    id: Uuid,
) -> Option<&mut TimelineValue<glam::Vec3>> {
    effect.number3_mut(id)
}
pub(crate) fn color_mut(
    effect: &mut ModifierEffect,
    id: Uuid,
) -> Option<&mut TimelineValue<shrimply_core::Color<u8>>> {
    effect.color_mut(id)
}

pub(crate) fn vec_row(
    label: &str,
    value: &TimelineValue<glam::Vec2>,
    modifier_id: Uuid,
    scale: bool,
    bounds: Option<(f64, f64)>,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    vec_control(
        label,
        value,
        &context,
        VecTarget {
            access: VecAccess::Modifier {
                id: modifier_id,
                value_id: value.id,
            },
            scope_id: Some(value.id),
            local_time: visual_local_time,
            global_time: visual_global_time,
            duration: visual_duration,
            refresh: ProjectChange {
                video: true,
                ..Default::default()
            },
            commit_name: "visual-modifier-vector",
        },
        VecSpec {
            first_prefix: "X",
            second_prefix: "Y",
            drag_step: if scale { 0.01 } else { 1.0 },
            digits: if scale { 2 } else { 0 },
            width_chars: 7,
            minimum: bounds.map(|bounds| bounds.0),
            maximum: bounds.map(|bounds| bounds.1),
            unit_name: if scale { "x" } else { "px" },
        },
    )
}
pub(crate) fn color_row(
    label: &str,
    value: &TimelineValue<shrimply_core::Color<u8>>,
    modifier_id: Uuid,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    color_control(
        label,
        value,
        &context,
        ColorTarget {
            access: ColorAccess::Modifier {
                id: modifier_id,
                value_id: value.id,
            },
            scope_id: Some(value.id),
            local_time: visual_local_time,
            global_time: visual_global_time,
            duration: visual_duration,
            refresh: ProjectChange {
                video: true,
                ..Default::default()
            },
            commit_name: "visual-modifier-color",
        },
    )
}

pub(crate) fn vec3_row(
    label: &str,
    value: &TimelineValue<glam::Vec3>,
    modifier_id: Uuid,
    degrees: bool,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    let target = Vec3Target::modifier_builder(modifier_id, value.id);
    vec3_control(
        label,
        value,
        &context,
        if degrees {
            target.degrees().build()
        } else {
            target.build()
        },
    )
}

pub(crate) fn vec3_scale_row(
    label: &str,
    value: &TimelineValue<glam::Vec3>,
    modifier_id: Uuid,
    context: &InspectorContext,
) -> gtk::Widget {
    let context = deferred_context(context);
    vec3_control(
        label,
        value,
        &context,
        Vec3Target::modifier_builder(modifier_id, value.id)
            .minimum(0.0)
            .lock()
            .build(),
    )
}

fn deferred_context(context: &InspectorContext) -> InspectorContext {
    let mut context = context.clone();
    let refresh = context.refresh.clone();
    let pending = std::rc::Rc::new(std::cell::Cell::new(false));
    context.refresh = std::rc::Rc::new(move || {
        if pending.replace(true) {
            return;
        }
        let pending = pending.clone();
        let refresh = refresh.clone();
        gtk::glib::idle_add_local_once(move || {
            pending.set(false);
            refresh();
        });
    });
    context
}
