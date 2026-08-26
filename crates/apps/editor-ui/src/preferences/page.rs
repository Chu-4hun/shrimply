use crate::preferences::{server, store as preferences_store};
use adw::prelude::*;
use num_traits::ToPrimitive;
use shrimply_math_core::{Fraction, Time};
use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::ColorPicker;

pub fn show_preferences_dialog(
    window: &adw::ApplicationWindow,
    preferences: preferences_store::SharedPreferences,
) {
    let snapshot = preferences_store::snapshot(&preferences);

    let default_font_row = adw::ActionRow::builder()
        .title(tr!("Default Text Font").as_ref())
        .build();
    let default_font =
        shrimply_inspector_ui::font_family_selector(&snapshot.default_text_font_family, {
            let preferences = preferences.clone();
            move |family| preferences_store::set_default_text_font_family(&preferences, family)
        });
    default_font.set_valign(gtk::Align::Center);
    default_font.set_width_request(260);
    default_font_row.add_suffix(&default_font);
    let text_group = adw::PreferencesGroup::new();
    text_group.set_title(tr!("Text").as_ref());
    text_group.add(&default_font_row);

    let caption_font_size = adw::SpinRow::with_range(8.0, 240.0, 1.0);
    caption_font_size.set_title(tr!("Font Size").as_ref());
    caption_font_size.set_value(snapshot.caption_font_size as f64);
    caption_font_size.set_digits(0);

    let caption_background_color_row = adw::ActionRow::builder()
        .title(tr!("Background Color").as_ref())
        .build();

    let caption_group = adw::PreferencesGroup::new();
    caption_group.set_title(tr!("Captions").as_ref());
    caption_group.add(&caption_font_size);
    caption_group.add(&caption_background_color_row);

    let default_visual_duration = adw::SpinRow::with_range(0.1, 3600.0, 0.1);
    default_visual_duration.set_title(tr!("Default Visual Duration").as_ref());
    default_visual_duration.set_value(snapshot.default_visual_duration.as_secs_f64());
    default_visual_duration.set_digits(1);

    let snap_radius = adw::SpinRow::with_range(
        f64::from(preferences_store::MIN_TIMELINE_SNAP_RADIUS_PX),
        f64::from(preferences_store::MAX_TIMELINE_SNAP_RADIUS_PX),
        1.0,
    );
    snap_radius.set_title(tr!("Snap Attraction Radius").as_ref());
    snap_radius
        .set_subtitle(tr!("Distance in pixels for timeline, beat, and preview snapping").as_ref());
    snap_radius.set_value(f64::from(snapshot.timeline_snap_radius_px));
    snap_radius.set_digits(0);

    let timeline_group = adw::PreferencesGroup::new();
    timeline_group.set_title(tr!("Timeline").as_ref());
    timeline_group.add(&default_visual_duration);
    timeline_group.add(&snap_radius);

    let preview_padding = adw::SpinRow::with_range(
        0.0,
        f64::from(preferences_store::MAX_PREVIEW_PADDING_PX),
        1.0,
    );
    preview_padding.set_title(tr!("Padding").as_ref());
    preview_padding.set_subtitle(tr!("Space around the video frame in pixels").as_ref());
    preview_padding.set_value(f64::from(snapshot.preview_padding_px));
    preview_padding.set_digits(0);

    let preview_shadow_size = adw::SpinRow::with_range(
        0.0,
        f64::from(preferences_store::MAX_PREVIEW_SHADOW_SIZE_PX),
        1.0,
    );
    preview_shadow_size.set_title(tr!("Shadow Size").as_ref());
    preview_shadow_size
        .set_subtitle(tr!("Drop shadow extent around the video frame in pixels").as_ref());
    preview_shadow_size.set_value(f64::from(snapshot.preview_shadow_size_px));
    preview_shadow_size.set_digits(0);

    let preview_group = adw::PreferencesGroup::new();
    preview_group.set_title(tr!("Preview").as_ref());
    preview_group.add(&preview_padding);
    preview_group.add(&preview_shadow_size);

    let temporal_decoder_pool_size = adw::SpinRow::with_range(
        f64::from(preferences_store::MIN_TEMPORAL_DECODER_POOL_SIZE),
        f64::from(preferences_store::MAX_TEMPORAL_DECODER_POOL_SIZE),
        1.0,
    );
    temporal_decoder_pool_size.set_title(tr!("Temporal Decoder Pool Size").as_ref());
    temporal_decoder_pool_size
        .set_subtitle(tr!("Maximum number of active video decoder sessions").as_ref());
    temporal_decoder_pool_size.set_value(f64::from(snapshot.temporal_decoder_pool_size));
    temporal_decoder_pool_size.set_digits(0);

    let image_pool_cpu = adw::SpinRow::with_range(
        0.0,
        f64::from(preferences_store::MAX_RESOURCE_POOL_GIB),
        0.25,
    );
    image_pool_cpu.set_title(tr!("Image Pool CPU Budget").as_ref());
    image_pool_cpu.set_subtitle(tr!("Static image cache in system RAM (GiB)").as_ref());
    image_pool_cpu.set_value(snapshot.image_pool_cpu_gib.to_f64().unwrap_or(4.0));
    image_pool_cpu.set_digits(2);

    let performance_group = adw::PreferencesGroup::new();
    performance_group.set_title(tr!("Performance").as_ref());
    performance_group.add(&temporal_decoder_pool_size);
    performance_group.add(&image_pool_cpu);

    let appearance_page = adw::PreferencesPage::builder()
        .title(tr!("Appearance").as_ref())
        .icon_name("appearance-symbolic")
        .name("appearance")
        .build();
    appearance_page.add(&caption_group);
    appearance_page.add(&text_group);
    appearance_page.add(&preview_group);
    appearance_page.add(&timeline_group);

    let performance_page = adw::PreferencesPage::builder()
        .title(tr!("Performance").as_ref())
        .icon_name("speedometer-symbolic")
        .name("performance")
        .build();
    performance_page.add(&performance_group);

    let blender_row = adw::ActionRow::builder()
        .title(tr!("Blender Binary").as_ref())
        .subtitle(shrimply_blender::binary_label(
            snapshot.blender_binary.as_deref(),
        ))
        .build();
    let clear_blender = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text(tr!("Clear Blender binary").as_ref())
        .valign(gtk::Align::Center)
        .build();
    let choose_blender = gtk::Button::builder()
        .label(tr!("Choose…").as_ref())
        .valign(gtk::Align::Center)
        .build();
    blender_row.add_suffix(&clear_blender);
    blender_row.add_suffix(&choose_blender);
    let blender_group = adw::PreferencesGroup::new();
    blender_group.set_title(tr!("Blender").as_ref());
    blender_group.add(&blender_row);

    let dialog = adw::PreferencesDialog::builder()
        .title(tr!("Preferences").as_ref())
        .search_enabled(false)
        .build();
    dialog.add(&appearance_page);
    dialog.add(&performance_page);
    dialog.add(&server::page(preferences.clone(), &blender_group));

    let font_preferences = preferences.clone();
    caption_font_size.connect_value_notify(move |row| {
        let value = row.value().clamp(8.0, 240.0) as f32;
        preferences_store::set_caption_font_size(&font_preferences, value);
    });

    let color_store = preferences.clone();
    let color_button = ColorPicker::builder(snapshot.caption_background_color)
        .title(tr!("Caption background color").as_ref())
        .on_change(move |color| {
            preferences_store::set_caption_background_color(&color_store, color)
        })
        .build();
    color_button.set_valign(gtk::Align::Center);
    color_button.set_hexpand(false);
    color_button.set_vexpand(false);
    caption_background_color_row.add_suffix(&color_button);

    let duration_store = preferences.clone();
    default_visual_duration.connect_value_notify(move |row| {
        preferences_store::set_default_visual_duration(
            &duration_store,
            Time::from_seconds_f64(row.value()),
        );
    });

    let snap_radius_store = preferences.clone();
    snap_radius.connect_value_notify(move |row| {
        preferences_store::set_timeline_snap_radius_px(
            &snap_radius_store,
            row.value().round() as u32,
        );
    });

    let preview_padding_store = preferences.clone();
    preview_padding.connect_value_notify(move |row| {
        preferences_store::set_preview_padding_px(
            &preview_padding_store,
            row.value().round() as u32,
        );
    });

    let preview_shadow_store = preferences.clone();
    preview_shadow_size.connect_value_notify(move |row| {
        preferences_store::set_preview_shadow_size_px(
            &preview_shadow_store,
            row.value().round() as u32,
        );
    });

    let temporal_decoder_pool_store = preferences.clone();
    temporal_decoder_pool_size.connect_value_notify(move |row| {
        preferences_store::set_temporal_decoder_pool_size(
            &temporal_decoder_pool_store,
            row.value().round() as u32,
        );
    });

    let image_pool_cpu_store = preferences.clone();
    image_pool_cpu.connect_value_notify(move |row| {
        preferences_store::set_image_pool_cpu_gib(&image_pool_cpu_store, gib_fraction(row.value()));
    });

    let clear_store = preferences.clone();
    let clear_row = blender_row.clone();
    clear_blender.connect_clicked(move |_| {
        preferences_store::set_blender_binary(&clear_store, None);
        shrimply_blender::set_binary(None);
        clear_row.set_subtitle(tr!("Not configured").as_ref());
    });

    let choose_store = preferences.clone();
    let choose_row = blender_row.clone();
    let choose_dialog = dialog.clone();
    choose_blender.connect_clicked(move |_| {
        let picker = gtk::FileDialog::builder()
            .title(tr!("Choose Blender Binary").as_ref())
            .build();
        let store = choose_store.clone();
        let row = choose_row.clone();
        let parent = choose_dialog.clone();
        shrimply_ui_foundation::file_picker::open(
            "Choose Blender Binary",
            &picker,
            parent.root().and_downcast::<gtk::Window>().as_ref(),
            move |result| {
                let Some(path) = result.ok().and_then(|file| file.path()) else {
                    return;
                };
                let (sender, receiver) = async_channel::bounded(1);
                let probe_path = path.clone();
                std::thread::spawn(move || {
                    let result = shrimply_blender::canonical_binary(&probe_path)
                        .and_then(|path| shrimply_blender::probe(&path).map(|()| path));
                    let _ = sender.send_blocking(result);
                });
                let store = store.clone();
                let parent = parent.clone();
                gtk::glib::spawn_future_local(async move {
                    let Ok(result) = receiver.recv().await else {
                        return;
                    };
                    match result {
                        Ok(path) => {
                            preferences_store::set_blender_binary(&store, Some(&path));
                            shrimply_blender::set_binary(Some(path.clone()));
                            row.set_subtitle(&path.display().to_string());
                        }
                        Err(error) => {
                            let alert = adw::AlertDialog::new(
                                Some("Incompatible Blender Binary"),
                                Some(&error),
                            );
                            alert.add_response("close", tr!("Close").as_ref());
                            alert.present(Some(parent.upcast_ref::<gtk::Widget>()));
                        }
                    }
                });
            },
        );
    });

    dialog.present(Some(window.upcast_ref::<gtk::Widget>()));
}

fn gib_fraction(value: f64) -> Fraction {
    Fraction::new((value.max(0.0) * 4.0).round() as u64, 4_u64)
}
