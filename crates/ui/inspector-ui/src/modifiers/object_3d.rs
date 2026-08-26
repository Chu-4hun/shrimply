use adw::prelude::AdwDialogExt;
use gtk::prelude::*;
use shrimply_project::project::Project;
use shrimply_scene_3d::{MAX_IOR, MIN_IOR, MIN_ROUGHNESS, NormalMode};
use shrimply_video_modifiers::{
    ModifierEffect,
    scene_3d::{Object3dModifier, Scene3dModifierEffect},
};
use uuid::Uuid;

use crate::{
    InspectorContext,
    player_state::{self, ProjectChange},
    selector::enum_selector,
};

use super::{ScalarOptions, color_row, scalar_row, vec3_row, vec3_scale_row};

pub fn add_rows(value: &Object3dModifier, out: &gtk::Box, id: Uuid, context: &InspectorContext) {
    out.append(&file_row(value, id, context));
    for (label, timeline, degrees) in [
        ("Position", &value.transform.position, false),
        ("Anchor", &value.transform.anchor, false),
        ("Rotation", &value.transform.rotation_degrees, true),
    ] {
        out.append(&vec3_row(label, timeline, id, degrees, context));
    }
    out.append(&vec3_scale_row(
        "Scale",
        &value.transform.scale,
        id,
        context,
    ));
    out.append(&color_row(
        "Base color",
        &value.material.base_color,
        id,
        context,
    ));
    for (label, timeline, minimum, maximum) in [
        ("Metallic", &value.material.metallic, Some(0.0), Some(1.0)),
        (
            "Roughness",
            &value.material.roughness,
            Some(MIN_ROUGHNESS as f64),
            Some(1.0),
        ),
        (
            "Subsurface",
            &value.material.subsurface,
            Some(0.0),
            Some(1.0),
        ),
        ("Clearcoat", &value.material.clearcoat, Some(0.0), Some(1.0)),
        ("Sheen", &value.material.sheen, Some(0.0), Some(1.0)),
        (
            "Transmission",
            &value.material.transmission,
            Some(0.0),
            Some(1.0),
        ),
        (
            "Index of refraction",
            &value.material.ior,
            Some(MIN_IOR as f64),
            Some(MAX_IOR as f64),
        ),
    ] {
        out.append(&number_row(
            label, timeline, id, minimum, maximum, None, context,
        ));
    }
    out.append(&normal_selector(value.material.normal_mode, id, context));
}

fn number_row(
    label: &str,
    value: &shrimply_core::timeline_value::TimelineValue<f32>,
    id: Uuid,
    minimum: Option<f64>,
    maximum: Option<f64>,
    unit: Option<&'static str>,
    context: &InspectorContext,
) -> gtk::Widget {
    scalar_row(
        label,
        value,
        id,
        ScalarOptions {
            minimum,
            maximum,
            unit,
            rotating: false,
        },
        context,
    )
}

fn file_row(value: &Object3dModifier, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let filename = value
        .file
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let filename_label = gtk::Label::builder()
        .label(filename.as_ref())
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["dim-label"])
        .build();
    filename_label.set_tooltip_text((!filename.is_empty()).then_some(filename.as_ref()));
    row.append(&filename_label);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    actions.set_halign(gtk::Align::End);
    actions.add_css_class("linked");
    let choose_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    choose_content.append(&gtk::Image::from_icon_name("folder-open-symbolic"));
    choose_content.append(&gtk::Label::new(Some(if value.file.is_some() {
        "Replace model"
    } else {
        "Select model"
    })));
    let choose = gtk::Button::builder().child(&choose_content).build();
    let clear = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Clear model")
        .sensitive(value.file.is_some())
        .build();
    actions.append(&choose);
    actions.append(&clear);
    row.append(&actions);

    let choose_context = context.detached();
    choose.connect_clicked(move |_| {
        let label = "Select 3D model";
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("3D models"));
        filter.add_pattern("*.obj");
        filter.add_pattern("*.glb");
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(label)
            .filters(&filters)
            .build();
        let context = choose_context.clone();
        shrimply_ui_foundation::file_picker::open(
            label,
            &dialog,
            None::<&gtk::Window>,
            move |result| {
                let Ok(file) = result else { return };
                let Some(path) = file.path() else { return };
                if let Err(error) = validate_model(&path) {
                    adw::AlertDialog::new(Some("Could not use 3D model"), Some(&error))
                        .present(None::<&gtk::Widget>);
                    return;
                }
                update_object(&context, id, "edit-3d-object-file", move |object| {
                    object.file = Some(path.into())
                });
            },
        );
    });
    let clear_context = context.detached();
    clear.connect_clicked(move |_| {
        update_object(&clear_context, id, "clear-3d-object-file", |object| {
            object.file = None
        })
    });
    crate::ui::control_row("Model", &row)
}

fn validate_model(path: &std::path::Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
    {
        shrimply_scene_3d::load_glb(path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        shrimply_scene_3d::load_obj(path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn normal_selector(value: NormalMode, id: Uuid, context: &InspectorContext) -> gtk::Widget {
    let context = context.detached();
    enum_selector("Normals", value, move |value| {
        update_object(&context, id, "edit-3d-object-normals", move |object| {
            object.material.normal_mode = value
        })
    })
}

fn update_object(
    context: &InspectorContext,
    id: Uuid,
    commit: &str,
    update: impl FnOnce(&mut Object3dModifier),
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(object) = object_mut(&mut project, key.clone(), id) else {
        return;
    };
    update(object);
    shrimply_project::project::commit_edit(&project, commit);
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

fn object_mut(
    project: &mut Project,
    key: crate::InspectedItem,
    id: Uuid,
) -> Option<&mut Object3dModifier> {
    project
        .video_item_mut(&key)?
        .modifiers
        .iter_mut()
        .find(|modifier| modifier.id == id)
        .and_then(|modifier| match &mut modifier.effect {
            ModifierEffect::Scene3d(effect) => match &mut **effect {
                Scene3dModifierEffect::Object(object) => Some(&mut **object),
                _ => None,
            },
            _ => None,
        })
}
