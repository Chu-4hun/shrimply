use shrimply_ui_foundation::tr;
use shrimply_ui_foundation::ui::I18nWidgetExt;
use std::rc::Rc;
use std::thread;

use adw::prelude::PreferencesRowExt;
use gtk::glib;
use gtk::prelude::*;
use uuid::Uuid;

use crate::player_state::{self, ProjectChange};
use shrimply_audio_modifiers::{
    AudioModifier, AudioModifierEffect, BitcrusherModifier, ChorusModifier, CloseUpModifier,
    CompressorModifier, DenoiseEngine, DenoiseModifier, DistortionModifier, EchoModifier,
    EqualizerModifier, F0Method, FilterMode, FilterModifier, GainModifier, LimiterModifier,
    NoiseGateModifier, PNEUMA_MAX_PITCH_OFFSET, PNEUMA_MAX_SPEED, PNEUMA_MIN_PITCH_OFFSET,
    PNEUMA_MIN_SPEED, PanModifier, PitchModifier, PitchQuality, ReverbMode, ReverbModifier,
    StereoWidthModifier, TremoloModifier, VoiceChangeModifier, VoiceColorModifier,
};
use shrimply_core::modifier_model::ModifierModel;

use super::InspectorContext;
use super::item::{DefaultInspectorItem, HeaderAction, HeaderToggle, InspectorListItem, flat};
use super::modifiers::{ScalarOptions, audio_scalar_row};
use super::selector::{selector, string_selector};

pub(super) fn items(
    modifiers: &[AudioModifier],
    context: &InspectorContext,
) -> Vec<InspectorListItem> {
    let mut rows = modifiers
        .iter()
        .enumerate()
        .map(|(index, modifier)| modifier_item(modifier, index, modifiers.len(), context))
        .collect::<Vec<_>>();
    rows.push(flat(modifier_buttons(context)));
    rows
}

fn modifier_item(
    modifier: &AudioModifier,
    index: usize,
    len: usize,
    context: &InspectorContext,
) -> InspectorListItem {
    let key = format!("audio-modifier:{}", modifier.id);
    let display_name = modifier.effect.display_name();
    let actions = actions(modifier.id, index, len, context);
    macro_rules! item {
        ($value:expr, $rows:expr, $wrap:path) => {{
            let id = modifier.id;
            DefaultInspectorItem::new(
                key.clone(),
                display_name,
                $value.clone(),
                move |value, context| $rows(value, id, context),
                move |context, value| reset(context, id, $wrap(value)),
            )
            .toggle(modifier_toggle(modifier, context))
            .actions(actions)
            .boxed()
        }};
    }
    match &modifier.effect {
        AudioModifierEffect::Cache(value) => {
            item!(value, crate::audio_cache::rows, AudioModifierEffect::Cache)
        }
        AudioModifierEffect::Gain(value) => {
            item!(value, gain_rows, AudioModifierEffect::Gain)
        }
        AudioModifierEffect::Pan(value) => {
            item!(value, pan_rows, AudioModifierEffect::Pan)
        }
        AudioModifierEffect::Pitch(value) => {
            item!(value, pitch_rows, AudioModifierEffect::Pitch)
        }
        AudioModifierEffect::Denoise(value) => {
            item!(value, denoise_rows, AudioModifierEffect::Denoise)
        }
        AudioModifierEffect::Equalizer(value) => {
            item!(value, equalizer_rows, AudioModifierEffect::Equalizer)
        }
        AudioModifierEffect::Filter(value) => {
            item!(value, filter_rows, AudioModifierEffect::Filter)
        }
        AudioModifierEffect::NoiseGate(value) => {
            item!(value, noise_gate_rows, AudioModifierEffect::NoiseGate)
        }
        AudioModifierEffect::StereoWidth(value) => {
            item!(value, stereo_width_rows, AudioModifierEffect::StereoWidth)
        }
        AudioModifierEffect::Tremolo(value) => {
            item!(value, tremolo_rows, AudioModifierEffect::Tremolo)
        }
        AudioModifierEffect::Bitcrusher(value) => {
            item!(value, bitcrusher_rows, AudioModifierEffect::Bitcrusher)
        }
        AudioModifierEffect::Chorus(value) => {
            item!(value, chorus_rows, AudioModifierEffect::Chorus)
        }
        AudioModifierEffect::Compressor(value) => {
            item!(value, compressor_rows, AudioModifierEffect::Compressor)
        }
        AudioModifierEffect::Limiter(value) => {
            item!(value, limiter_rows, AudioModifierEffect::Limiter)
        }
        AudioModifierEffect::Reverb(value) => {
            item!(value, reverb_rows, AudioModifierEffect::Reverb)
        }
        AudioModifierEffect::CloseUp(value) => {
            item!(value, close_up_rows, AudioModifierEffect::CloseUp)
        }
        AudioModifierEffect::VoiceColor(value) => {
            item!(value, voice_color_rows, AudioModifierEffect::VoiceColor)
        }
        AudioModifierEffect::Echo(value) => {
            item!(value, echo_rows, AudioModifierEffect::Echo)
        }
        AudioModifierEffect::Distortion(value) => {
            item!(value, distortion_rows, AudioModifierEffect::Distortion)
        }
        AudioModifierEffect::VoiceChange(value) => {
            item!(value, voice_change_rows, AudioModifierEffect::VoiceChange)
        }
    }
}

fn modifier_toggle(modifier: &AudioModifier, context: &InspectorContext) -> HeaderToggle {
    let id = modifier.id;
    let context = context.detached();
    HeaderToggle {
        active: modifier.enabled,
        tooltip: if modifier.enabled {
            "Disable modifier"
        } else {
            "Enable modifier"
        },
        activate: Rc::new(move |enabled| set_enabled(&context, id, enabled)),
    }
}

fn set_enabled(context: &InspectorContext, id: Uuid, enabled: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(modifier) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
    else {
        return;
    };
    if modifier.enabled == enabled {
        return;
    }
    modifier.enabled = enabled;
    shrimply_project::project::commit_edit(&project, "toggle-audio-modifier");
    drop(project);
    refresh(context);
}

fn controls(rows: impl IntoIterator<Item = gtk::Widget>) -> Vec<gtk::Widget> {
    let out = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for row in rows {
        out.append(&row);
    }
    vec![out.upcast()]
}

fn switch_row(label: &str, active: bool) -> (gtk::Widget, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let label = gtk::Label::new(Some(label));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    let toggle = gtk::Switch::builder()
        .active(active)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    row.append(&label);
    row.append(&toggle);
    (row.upcast(), toggle)
}

fn scalar(
    label: &str,
    value: &crate::timeline_value::TimelineValue<f32>,
    id: Uuid,
    minimum: f64,
    maximum: f64,
    unit: Option<&'static str>,
    context: &InspectorContext,
) -> gtk::Widget {
    audio_scalar_row(
        label,
        value,
        id,
        ScalarOptions {
            minimum: Some(minimum),
            maximum: Some(maximum),
            unit,
            rotating: false,
        },
        context,
    )
}

fn gain_rows(value: &GainModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls([scalar(
        "Level",
        &value.decibels,
        id,
        -60.0,
        36.0,
        Some("dB"),
        context,
    )])
}

fn pan_rows(value: &PanModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls([scalar(
        "Position",
        &value.position,
        id,
        -1.0,
        1.0,
        None,
        context,
    )])
}

fn pitch_rows(value: &PitchModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let detached = context.detached();
    let quality = selector(
        "Quality",
        value.quality,
        [
            (PitchQuality::Balanced, "Balanced"),
            (PitchQuality::LowLatency, "Low latency"),
        ],
        move |quality| update_pitch_quality(&detached, id, quality),
    );
    let (formants_row, formants) = switch_row("Preserve formants", value.preserve_formants);
    let detached = context.detached();
    formants.connect_active_notify(move |toggle| {
        update_formants(&detached, id, toggle.is_active());
    });
    let (linked_row, linked) = switch_row("Link stereo channels", value.link_channels);
    let detached = context.detached();
    linked.connect_active_notify(move |toggle| {
        update_pitch_link_channels(&detached, id, toggle.is_active());
    });
    let mut rows = vec![
        quality.upcast(),
        scalar(
            "Semitones",
            &value.semitones,
            id,
            -24.0,
            24.0,
            Some("st"),
            context,
        ),
        formants_row,
    ];
    if !value.preserve_formants {
        rows.push(scalar(
            "Formant shift",
            &value.formant_semitones,
            id,
            -12.0,
            12.0,
            Some("st"),
            context,
        ));
    }
    rows.push(linked_row);
    controls(rows)
}

fn denoise_rows(value: &DenoiseModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let detached = context.detached();
    let engine = selector(
        "Engine",
        value.engine,
        [
            (DenoiseEngine::Rnnoise, "RNNoise"),
            (DenoiseEngine::DeepFilterNet, "DeepFilterNet"),
        ],
        move |engine| update_denoise_engine(&detached, id, engine),
    );
    let mut rows = vec![
        engine.upcast(),
        scalar("Amount", &value.amount, id, 0.0, 1.0, Some("%"), context),
    ];
    if value.engine == DenoiseEngine::DeepFilterNet {
        rows.push(scalar(
            "Reduction",
            &value.reduction_db,
            id,
            0.01,
            97.0,
            Some("dB"),
            context,
        ));
    }
    controls(rows)
}

fn update_denoise_engine(context: &InspectorContext, id: Uuid, engine: DenoiseEngine) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Denoise(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.engine == engine {
        return;
    }
    value.engine = engine;
    shrimply_project::project::commit_edit(&project, "audio-denoise-engine");
    drop(project);
    refresh(context);
}

fn update_filter_mode(context: &InspectorContext, id: Uuid, mode: FilterMode) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Filter(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.mode == mode {
        return;
    }
    value.mode = mode;
    shrimply_project::project::commit_edit(&project, "audio-filter-mode");
    drop(project);
    refresh(context);
}

fn update_reverb_mode(context: &InspectorContext, id: Uuid, mode: ReverbMode) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Reverb(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.mode == mode {
        return;
    }
    value.mode = mode;
    shrimply_project::project::commit_edit(&project, "audio-reverb-mode");
    drop(project);
    refresh(context);
}

fn update_voice_color_auto_level(context: &InspectorContext, id: Uuid, auto_level: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::VoiceColor(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.auto_level == auto_level {
        return;
    }
    value.auto_level = auto_level;
    shrimply_project::project::commit_edit(&project, "audio-voice-color-auto-level");
    drop(project);
    refresh(context);
}

fn equalizer_rows(
    value: &EqualizerModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([
        scalar("Low", &value.low_db, id, -24.0, 24.0, Some("dB"), context),
        scalar("Mid", &value.mid_db, id, -24.0, 24.0, Some("dB"), context),
        scalar("High", &value.high_db, id, -24.0, 24.0, Some("dB"), context),
    ])
}

fn filter_rows(value: &FilterModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let detached = context.detached();
    let mode = selector(
        "Mode",
        value.mode,
        [
            (FilterMode::LowPass, "Low-pass"),
            (FilterMode::HighPass, "High-pass"),
        ],
        move |mode| update_filter_mode(&detached, id, mode),
    );
    controls([
        mode.upcast(),
        scalar(
            "Cutoff",
            &value.cutoff_hz,
            id,
            20.0,
            20_000.0,
            Some("Hz"),
            context,
        ),
        scalar(
            "Resonance",
            &value.resonance,
            id,
            0.5,
            10.0,
            Some("Q"),
            context,
        ),
    ])
}

fn noise_gate_rows(
    value: &NoiseGateModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([
        scalar(
            "Threshold",
            &value.threshold_db,
            id,
            -80.0,
            0.0,
            Some("dB"),
            context,
        ),
        scalar(
            "Attack",
            &value.attack_ms,
            id,
            0.1,
            500.0,
            Some("ms"),
            context,
        ),
        scalar(
            "Release",
            &value.release_ms,
            id,
            1.0,
            2_000.0,
            Some("ms"),
            context,
        ),
    ])
}

fn stereo_width_rows(
    value: &StereoWidthModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([scalar(
        "Width",
        &value.width,
        id,
        0.0,
        2.0,
        Some("%"),
        context,
    )])
}

fn tremolo_rows(value: &TremoloModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls([
        scalar("Rate", &value.rate_hz, id, 0.1, 20.0, Some("Hz"), context),
        scalar("Depth", &value.depth, id, 0.0, 1.0, Some("%"), context),
    ])
}

fn bitcrusher_rows(
    value: &BitcrusherModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([
        scalar(
            "Resolution",
            &value.resolution_bits,
            id,
            2.0,
            24.0,
            Some("bit"),
            context,
        ),
        scalar(
            "Sample rate",
            &value.sample_rate_hz,
            id,
            1_000.0,
            48_000.0,
            Some("Hz"),
            context,
        ),
        scalar("Mix", &value.mix, id, 0.0, 1.0, Some("%"), context),
    ])
}

fn chorus_rows(value: &ChorusModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls([
        scalar("Rate", &value.rate_hz, id, 0.05, 5.0, Some("Hz"), context),
        scalar("Depth", &value.depth_ms, id, 0.0, 10.0, Some("ms"), context),
        scalar("Delay", &value.delay_ms, id, 5.0, 30.0, Some("ms"), context),
        scalar("Mix", &value.mix, id, 0.0, 1.0, Some("%"), context),
    ])
}

fn compressor_rows(
    value: &CompressorModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([
        scalar(
            "Threshold",
            &value.threshold_db,
            id,
            -60.0,
            0.0,
            Some("dB"),
            context,
        ),
        scalar("Ratio", &value.ratio, id, 1.0, 20.0, Some(":1"), context),
        scalar(
            "Attack",
            &value.attack_ms,
            id,
            0.01,
            2_000.0,
            Some("ms"),
            context,
        ),
        scalar(
            "Release",
            &value.release_ms,
            id,
            0.01,
            9_000.0,
            Some("ms"),
            context,
        ),
        scalar(
            "Makeup",
            &value.makeup_db,
            id,
            0.0,
            36.0,
            Some("dB"),
            context,
        ),
        scalar("Mix", &value.mix, id, 0.0, 1.0, None, context),
    ])
}

fn limiter_rows(value: &LimiterModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    controls([
        scalar(
            "Ceiling",
            &value.ceiling_db,
            id,
            -24.0,
            0.0,
            Some("dB"),
            context,
        ),
        scalar(
            "Release",
            &value.release_ms,
            id,
            1.0,
            8_000.0,
            Some("ms"),
            context,
        ),
    ])
}

fn reverb_rows(value: &ReverbModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let detached = context.detached();
    let mode = selector(
        "Mode",
        value.mode,
        [
            (ReverbMode::RoomCapture, "Room capture"),
            (ReverbMode::Classic, "Classic"),
        ],
        move |mode| update_reverb_mode(&detached, id, mode),
    );
    let mut rows = vec![mode.upcast()];
    rows.push(scalar(
        "Room scale",
        &value.room_size,
        id,
        0.0,
        1.0,
        Some("%"),
        context,
    ));
    match value.mode {
        ReverbMode::RoomCapture => {
            rows.insert(
                1,
                scalar(
                    "Distance",
                    &value.distance_m,
                    id,
                    0.2,
                    5.0,
                    Some("m"),
                    context,
                ),
            );
            rows.push(scalar(
                "Absorption",
                &value.damping,
                id,
                0.0,
                1.0,
                Some("%"),
                context,
            ));
        }
        ReverbMode::Classic => {
            rows.push(scalar(
                "Decay",
                &value.decay_seconds,
                id,
                0.1,
                20.0,
                Some("s"),
                context,
            ));
            rows.push(scalar(
                "Damping",
                &value.damping,
                id,
                0.0,
                1.0,
                Some("%"),
                context,
            ));
            rows.push(scalar(
                "Pre-delay",
                &value.pre_delay_ms,
                id,
                0.0,
                500.0,
                Some("ms"),
                context,
            ));
            rows.push(scalar("Mix", &value.mix, id, 0.0, 1.0, Some("%"), context));
        }
    }
    controls(rows)
}

fn close_up_rows(
    value: &CloseUpModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([scalar(
        "Distance",
        &value.distance_cm,
        id,
        3.0,
        100.0,
        Some("cm"),
        context,
    )])
}

fn voice_color_rows(
    value: &VoiceColorModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let (auto_level_row, auto_level) = switch_row("Auto level", value.auto_level);
    let detached = context.detached();
    auto_level.connect_active_notify(move |toggle| {
        update_voice_color_auto_level(&detached, id, toggle.is_active());
    });
    controls([
        scalar(
            "Effect strength",
            &value.amount,
            id,
            0.0,
            1.0,
            Some("%"),
            context,
        ),
        auto_level_row,
    ])
}

fn echo_rows(value: &EchoModifier, id: Uuid, context: &InspectorContext) -> Vec<gtk::Widget> {
    let (ping_pong_row, ping_pong) = switch_row("Ping-pong", value.ping_pong);
    let detached = context.detached();
    ping_pong.connect_active_notify(move |toggle| {
        update_echo_ping_pong(&detached, id, toggle.is_active());
    });
    controls([
        scalar(
            "Delay",
            &value.delay_ms,
            id,
            1.0,
            2_000.0,
            Some("ms"),
            context,
        ),
        scalar(
            "Feedback",
            &value.feedback,
            id,
            0.0,
            0.95,
            Some("%"),
            context,
        ),
        ping_pong_row,
        scalar("Mix", &value.mix, id, 0.0, 1.0, Some("%"), context),
    ])
}

fn distortion_rows(
    value: &DistortionModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    controls([
        scalar("Drive", &value.drive_db, id, 0.0, 48.0, Some("dB"), context),
        scalar("Tone", &value.tone, id, 0.0, 1.0, Some("%"), context),
        scalar("Mix", &value.mix, id, 0.0, 1.0, Some("%"), context),
    ])
}

fn voice_change_rows(
    value: &VoiceChangeModifier,
    id: Uuid,
    context: &InspectorContext,
) -> Vec<gtk::Widget> {
    let server_url = shrimply_state::preferences::snapshot(&context.preferences).compute_server_url;
    shrimply_audio::pneuma::set_server_url(&server_url);
    let detached = context.detached();
    let model = string_selector(
        "Model",
        &value.model,
        vec![value.model.clone()],
        move |model| {
            update_voice_change(&detached, id, "audio-voice-model", move |value| {
                if value.model == model {
                    false
                } else {
                    value.model = model;
                    true
                }
            });
        },
    );
    model.set_sensitive(false);
    let model_update = model.clone();
    let current_model = value.model.clone();
    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let result = shrimply_server_client::pneuma::models(&server_url).map(|models| {
            models
                .into_iter()
                .map(|model| model.name)
                .collect::<Vec<_>>()
        });
        let _ = sender.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        match receiver.recv().await {
            Ok(Ok(mut models)) => {
                if !models.iter().any(|model| model == &current_model) {
                    models.insert(0, current_model.clone());
                }
                model_update.set_options(&current_model, models);
                model_update.set_sensitive(true);
                model_update.widget().set_tooltip_text(None);
            }
            Ok(Err(error)) => model_update.widget().set_tooltip_text(Some(&error)),
            Err(_) => model_update
                .widget()
                .set_tooltip_i18n("Pneuma model request stopped unexpectedly"),
        }
    });

    let pitch_offset = adw::SpinRow::with_range(
        f64::from(PNEUMA_MIN_PITCH_OFFSET),
        f64::from(PNEUMA_MAX_PITCH_OFFSET),
        1.0,
    );
    pitch_offset.set_title(tr!("Pitch offset").as_ref());
    pitch_offset.set_value(f64::from(value.pitch_offset));
    pitch_offset.set_digits(0);
    let detached = context.detached();
    pitch_offset.connect_value_notify(move |row| {
        let pitch_offset = row.value().round() as i32;
        update_voice_change(&detached, id, "audio-voice-pitch", move |value| {
            if value.pitch_offset == pitch_offset {
                false
            } else {
                value.pitch_offset = pitch_offset;
                true
            }
        });
    });

    let detached = context.detached();
    let f0_method = selector(
        "F0 method",
        value.f0_method,
        [
            (F0Method::Crepe, "CREPE"),
            (F0Method::Rmvpe, "RMVPE"),
            (F0Method::Fcpe, "FCPE"),
            (F0Method::SwiftF0, "Swift F0"),
        ],
        move |method| {
            update_voice_change(&detached, id, "audio-voice-f0-method", move |value| {
                if value.f0_method == method {
                    false
                } else {
                    value.f0_method = method;
                    true
                }
            });
        },
    );

    let speed = adw::SpinRow::with_range(
        f64::from(PNEUMA_MIN_SPEED),
        f64::from(PNEUMA_MAX_SPEED),
        0.1,
    );
    speed.set_title(tr!("Speed").as_ref());
    speed.set_value(f64::from(value.speed));
    speed.set_digits(1);
    let detached = context.detached();
    speed.connect_value_notify(move |row| {
        let speed =
            row.value()
                .clamp(f64::from(PNEUMA_MIN_SPEED), f64::from(PNEUMA_MAX_SPEED)) as f32;
        update_voice_change(&detached, id, "audio-voice-speed", move |value| {
            if value.speed == speed {
                false
            } else {
                value.speed = speed;
                true
            }
        });
    });

    let (maintain_pitch_row, maintain_pitch) =
        switch_row("Maintain pitch while changing speed", value.maintain_pitch);
    let detached = context.detached();
    maintain_pitch.connect_active_notify(move |toggle| {
        let maintain_pitch = toggle.is_active();
        update_voice_change(&detached, id, "audio-voice-maintain-pitch", move |value| {
            if value.maintain_pitch == maintain_pitch {
                false
            } else {
                value.maintain_pitch = maintain_pitch;
                true
            }
        });
    });

    controls([
        model.widget().clone(),
        pitch_offset.upcast(),
        f0_method,
        speed.upcast(),
        maintain_pitch_row,
    ])
}

fn update_voice_change(
    context: &InspectorContext,
    id: Uuid,
    edit_name: &str,
    update: impl FnOnce(&mut VoiceChangeModifier) -> bool,
) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::VoiceChange(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if !update(value) {
        return;
    }
    shrimply_project::project::commit_edit(&project, edit_name);
    drop(project);
    refresh(context);
}

#[derive(Clone, Copy)]
enum Action {
    Copy,
    Up,
    Down,
    Remove,
}

fn actions(id: Uuid, index: usize, len: usize, context: &InspectorContext) -> Vec<HeaderAction> {
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
        let context = context.detached();
        HeaderAction {
            icon,
            tooltip,
            sensitive,
            activate: Rc::new(move || apply_action(&context, id, action)),
        }
    })
    .collect()
}

fn apply_action(context: &InspectorContext, id: Uuid, action: Action) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.audio_item_mut(&key) else {
        return;
    };
    if matches!(action, Action::Copy) {
        let Some(modifier) = item.modifiers.iter().find(|modifier| modifier.id == id) else {
            return;
        };
        context
            .property_clipboard
            .borrow_mut()
            .copy_audio_modifier(modifier);
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
    let Some(index) = item.modifiers.iter().position(|modifier| modifier.id == id) else {
        return;
    };
    if matches!(action, Action::Remove)
        && matches!(item.modifiers[index].effect, AudioModifierEffect::Cache(_))
        && let Err(error) = shrimply_audio::modifier_cache::invalidate(id)
    {
        shrimply_ui_foundation::toast::show_confirmation_text_for_widget(
            &context.category_bar,
            &format!("Could not remove cache: {error}"),
        );
        return;
    }
    match action {
        Action::Copy => unreachable!(),
        Action::Up if index > 0 => item.modifiers.swap(index, index - 1),
        Action::Down if index + 1 < item.modifiers.len() => item.modifiers.swap(index, index + 1),
        Action::Remove => {
            item.modifiers.remove(index);
        }
        _ => return,
    }
    shrimply_project::project::commit_edit(&project, "edit-audio-modifier-chain");
    drop(project);
    refresh(context);
}

fn modifier_buttons(context: &InspectorContext) -> gtk::Widget {
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.set_halign(gtk::Align::Center);
    buttons.append(&add_button(context));
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
            audio: result.audio,
            audio_waveforms: result.audio_waveforms,
            inspector: true,
            ..Default::default()
        },
    );
}

fn reset(context: &InspectorContext, id: Uuid, effect: AudioModifierEffect) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(modifier) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
    else {
        return;
    };
    modifier.effect = effect;
    shrimply_project::project::commit_edit(&project, "reset-audio-modifier");
    drop(project);
    refresh(context);
}

fn update_formants(context: &InspectorContext, id: Uuid, preserve: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Pitch(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    value.preserve_formants = preserve;
    shrimply_project::project::commit_edit(&project, "audio-pitch-formants");
    drop(project);
    refresh(context);
}

fn update_pitch_quality(context: &InspectorContext, id: Uuid, quality: PitchQuality) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Pitch(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.quality == quality {
        return;
    }
    value.quality = quality;
    shrimply_project::project::commit_edit(&project, "audio-pitch-quality");
    drop(project);
    refresh(context);
}

fn update_pitch_link_channels(context: &InspectorContext, id: Uuid, link_channels: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Pitch(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.link_channels == link_channels {
        return;
    }
    value.link_channels = link_channels;
    shrimply_project::project::commit_edit(&project, "audio-pitch-link-channels");
    drop(project);
    refresh(context);
}

fn update_echo_ping_pong(context: &InspectorContext, id: Uuid, ping_pong: bool) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(AudioModifierEffect::Echo(value)) = project
        .audio_item_mut(&key)
        .and_then(|item| item.modifiers.iter_mut().find(|modifier| modifier.id == id))
        .map(|modifier| &mut modifier.effect)
    else {
        return;
    };
    if value.ping_pong == ping_pong {
        return;
    }
    value.ping_pong = ping_pong;
    shrimply_project::project::commit_edit(&project, "audio-echo-ping-pong");
    drop(project);
    refresh(context);
}

fn add(context: &InspectorContext, effect: AudioModifierEffect) {
    let Some(key) = context.selected_item.clone() else {
        return;
    };
    let mut project = context.project.borrow_mut();
    let Some(item) = project.audio_item_mut(&key) else {
        return;
    };
    item.modifiers.push(AudioModifier::new(effect));
    shrimply_project::project::commit_edit(&project, "add-audio-modifier");
    drop(project);
    refresh(context);
}

fn refresh(context: &InspectorContext) {
    player_state::refresh_project(
        &context.player_state,
        ProjectChange {
            audio: true,
            audio_waveforms: true,
            inspector: true,
            ..Default::default()
        },
    );
}

fn add_button(context: &InspectorContext) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    content.append(&gtk::Label::new(Some(tr!("Add modifier").as_ref())));
    let button = gtk::MenuButton::builder()
        .child(&content)
        .halign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text(tr!("Search modifiers").as_ref())
        .hexpand(true)
        .build();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(280)
        .min_content_height(240)
        .max_content_height(360)
        .build();
    let popover_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    popover_content.append(&search);
    popover_content.append(&scroller);
    let popover = gtk::Popover::builder()
        .child(&popover_content)
        .has_arrow(false)
        .build();
    popover.add_css_class("menu");
    button.set_popover(Some(&popover));
    populate(&list, "", context, &popover);
    search.connect_search_changed({
        let list = list.clone();
        let context = context.detached();
        let popover = popover.clone();
        move |search| populate(&list, search.text().as_str(), &context, &popover)
    });
    popover.connect_show(move |_| {
        search.grab_focus();
    });
    button.upcast()
}

fn populate(list: &gtk::Box, query: &str, context: &InspectorContext, popover: &gtk::Popover) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let query = query.trim().to_lowercase();
    let mut effects = AudioModifierEffect::CATALOG
        .iter()
        .map(|new| new())
        .filter_map(|effect| {
            let rank = if effect.display_name().to_lowercase().contains(&query) {
                0
            } else if effect
                .keywords()
                .iter()
                .any(|keyword| keyword.to_lowercase().contains(&query))
            {
                1
            } else {
                return None;
            };
            Some((rank, effect))
        })
        .collect::<Vec<_>>();
    effects.sort_by_key(|(rank, _)| *rank);
    for (_, effect) in effects {
        let name = effect.display_name();
        let row = gtk::Button::builder()
            .label(tr!(name).as_ref())
            .halign(gtk::Align::Fill)
            .hexpand(true)
            .css_classes(["flat"])
            .build();
        let context = context.detached();
        let popover = popover.clone();
        row.connect_clicked(move |_| {
            add(&context, effect.clone());
            popover.popdown();
        });
        list.append(&row);
    }
}
