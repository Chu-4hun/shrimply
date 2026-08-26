use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use shrimply_project::project::{
    Asset, BlenderItem, BlenderPreviewDownsample, BlenderRenderMethod, Time, VideoItemContent,
};

use super::update_video_item;
use crate::InspectorContext;
use crate::item::{DefaultInspectorItem, HeaderAction, InspectorListItem};
use crate::section::InspectorSection;
use crate::selector::{selector, string_selector};

pub(super) fn item(
    blender: &BlenderItem,
    source: Asset,
    _context: &InspectorContext,
) -> InspectorListItem {
    let reload_source = source.clone();
    let controls_source = source.clone();
    DefaultInspectorItem::new(
        "blender",
        "Blender",
        blender.clone(),
        move |blender, context| controls(blender, &controls_source, context),
        move |context, value| {
            let Some(key) = context.selected_item.clone() else {
                return;
            };
            update_video_item(
                &context.project,
                &context.player_state,
                key,
                "reset-blender",
                move |item| {
                    let VideoItemContent::Blender(blender) = &mut item.content else {
                        return false;
                    };
                    if **blender == value {
                        return false;
                    }
                    **blender = value;
                    true
                },
            );
        },
    )
    .actions(vec![HeaderAction {
        icon: "view-refresh-symbolic",
        tooltip: "Reload Blender file and scene metadata",
        sensitive: true,
        activate: Rc::new(move || {
            shrimply_blender::invalidate_metadata(reload_source.path());
            if let Err(error) = reload_source.mark_dirty() {
                tracing::warn!("Could not mark Blender source dirty: {error}");
            }
        }),
    }])
    .boxed()
}

fn controls(blender: &BlenderItem, source: &Asset, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let Some(key) = context.selected_item.clone() else {
        return vec![section.into_widget()];
    };
    let scene_value = if blender.scene.is_empty() {
        "Loading scenes…"
    } else {
        &blender.scene
    };
    let view_layer_value = if blender.view_layer.is_empty() {
        "Loading view layers…"
    } else {
        &blender.view_layer
    };
    let camera_value = if blender.camera.is_empty() {
        "Loading cameras…"
    } else {
        &blender.camera
    };
    let scenes = string_selector("Scene", scene_value, vec![scene_value.to_string()], {
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let key = key.clone();
        move |value| {
            update_video_item(
                &project,
                &player_state,
                key.clone(),
                "blender-scene",
                move |item| {
                    let VideoItemContent::Blender(blender) = &mut item.content else {
                        return false;
                    };
                    if blender.scene == value {
                        return false;
                    }
                    blender.scene = value;
                    blender.view_layer.clear();
                    blender.camera.clear();
                    true
                },
            );
        }
    });
    let view_layers = string_selector(
        "View Layer",
        view_layer_value,
        vec![view_layer_value.to_string()],
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            let key = key.clone();
            move |value| {
                update_video_item(
                    &project,
                    &player_state,
                    key.clone(),
                    "blender-view-layer",
                    move |item| {
                        let VideoItemContent::Blender(blender) = &mut item.content else {
                            return false;
                        };
                        if blender.view_layer == value {
                            return false;
                        }
                        blender.view_layer = value;
                        true
                    },
                )
            }
        },
    );
    let cameras = string_selector("Camera", camera_value, vec![camera_value.to_string()], {
        let project = context.project.clone();
        let player_state = context.player_state.clone();
        let key = key.clone();
        move |value| {
            update_video_item(
                &project,
                &player_state,
                key.clone(),
                "blender-camera",
                move |item| {
                    let VideoItemContent::Blender(blender) = &mut item.content else {
                        return false;
                    };
                    if blender.camera == value {
                        return false;
                    }
                    blender.camera = value;
                    true
                },
            )
        }
    });
    for control in [&scenes, &view_layers, &cameras] {
        control.set_sensitive(false);
        section.add_wide_control(control.widget());
    }
    section.add_wide_control(&selector(
        "Accurate Render Method",
        blender.render_method,
        [
            (BlenderRenderMethod::Solid, "Solid"),
            (BlenderRenderMethod::MaterialPreview, "Material Preview"),
            (BlenderRenderMethod::SceneRenderer, "Scene Renderer"),
        ],
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            let key = key.clone();
            move |value| {
                update_video_item(
                    &project,
                    &player_state,
                    key.clone(),
                    "blender-render-method",
                    move |item| {
                        let VideoItemContent::Blender(blender) = &mut item.content else {
                            return false;
                        };
                        if blender.render_method == value {
                            return false;
                        }
                        blender.render_method = value;
                        true
                    },
                )
            }
        },
    ));
    section.add_wide_control(&selector(
        "Preview Render Method",
        blender.preview_render_method,
        [
            (BlenderRenderMethod::Solid, "Solid"),
            (BlenderRenderMethod::MaterialPreview, "Material Preview"),
            (BlenderRenderMethod::SceneRenderer, "Scene Renderer"),
        ],
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            let key = key.clone();
            move |value| {
                update_video_item(
                    &project,
                    &player_state,
                    key.clone(),
                    "blender-preview-render-method",
                    move |item| {
                        let VideoItemContent::Blender(blender) = &mut item.content else {
                            return false;
                        };
                        if blender.preview_render_method == value {
                            return false;
                        }
                        blender.preview_render_method = value;
                        true
                    },
                )
            }
        },
    ));
    section.add_wide_control(&selector(
        "Preview Downsampling",
        blender.preview_downsample,
        [
            (BlenderPreviewDownsample::Full, "Off (Full Resolution)"),
            (BlenderPreviewDownsample::X2, "2×"),
            (BlenderPreviewDownsample::X4, "4×"),
            (BlenderPreviewDownsample::X8, "8×"),
            (BlenderPreviewDownsample::X16, "16×"),
            (BlenderPreviewDownsample::X32, "32×"),
        ],
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            let key = key.clone();
            move |value| {
                update_video_item(
                    &project,
                    &player_state,
                    key.clone(),
                    "blender-preview-downsampling",
                    move |item| {
                        let VideoItemContent::Blender(blender) = &mut item.content else {
                            return false;
                        };
                        if blender.preview_downsample == value {
                            return false;
                        }
                        blender.preview_downsample = value;
                        true
                    },
                )
            }
        },
    ));

    let binary = shrimply_state::preferences::snapshot(&context.preferences).blender_binary;
    let current = blender.clone();
    let source = source.path().to_path_buf();
    let listener_scope = Rc::downgrade(&context.listener_scope);
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = binary
            .as_deref()
            .ok_or_else(|| "Choose a compatible Blender binary in Preferences".to_string())
            .and_then(|binary| shrimply_blender::discover(binary, &source));
        let _ = sender.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else {
            return;
        };
        if listener_scope.upgrade().is_none() {
            return;
        }
        match result {
            Ok(metadata) if !metadata.scenes.is_empty() => {
                let scene = metadata
                    .scenes
                    .iter()
                    .find(|scene| scene.name == current.scene)
                    .unwrap_or(&metadata.scenes[0]);
                let view_layer = scene
                    .view_layers
                    .iter()
                    .find(|name| **name == current.view_layer)
                    .cloned()
                    .unwrap_or_else(|| scene.active_view_layer.clone());
                let camera = scene
                    .cameras
                    .iter()
                    .find(|name| **name == current.camera)
                    .cloned()
                    .unwrap_or_else(|| scene.active_camera.clone());
                scenes.set_options(
                    &scene.name,
                    metadata
                        .scenes
                        .iter()
                        .map(|scene| scene.name.clone())
                        .collect(),
                );
                view_layers.set_options(&view_layer, scene.view_layers.clone());
                cameras.set_options(&camera, scene.cameras.clone());
                scenes.set_sensitive(true);
                view_layers.set_sensitive(!scene.view_layers.is_empty());
                cameras.set_sensitive(!scene.cameras.is_empty());
                let scene_name = scene.name.clone();
                let duration = Time {
                    seconds: scene.duration(),
                };
                update_video_item(
                    &project,
                    &player_state,
                    key,
                    "blender-metadata",
                    move |item| {
                        let VideoItemContent::Blender(blender) = &mut item.content else {
                            return false;
                        };
                        let changed = blender.scene != scene_name
                            || blender.view_layer != view_layer
                            || blender.camera != camera
                            || item.source_duration != duration;
                        blender.scene = scene_name;
                        blender.view_layer = view_layer;
                        blender.camera = camera;
                        item.source_duration = duration;
                        changed
                    },
                );
            }
            Ok(_) => scenes
                .widget()
                .set_tooltip_text(Some("The Blender file contains no scenes")),
            Err(error) => {
                scenes.set_options(
                    "Could not load Blender",
                    vec!["Could not load Blender".into()],
                );
                scenes.widget().set_tooltip_text(Some(&error));
            }
        }
    });

    vec![section.into_widget()]
}
