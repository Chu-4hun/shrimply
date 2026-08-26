use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use shrimply_math_core::Fraction;
use shrimply_project_core::{COMMON_FRAME_RATES, CanvasSize, PROJECT_PRESETS};

pub struct ProjectSettingsSelector {
    pub preset: adw::ComboRow,
    pub width: adw::SpinRow,
    pub height: adw::SpinRow,
    pub fps: adw::ComboRow,
}

impl ProjectSettingsSelector {
    pub fn new() -> Self {
        let mut preset_labels = PROJECT_PRESETS
            .iter()
            .map(|preset| preset.label)
            .collect::<Vec<_>>();
        preset_labels.push("Custom");
        let preset = adw::ComboRow::builder()
            .title("Preset")
            .model(&gtk::StringList::new(&preset_labels))
            .selected(
                PROJECT_PRESETS
                    .iter()
                    .position(|preset| preset.label == "1080p 30 FPS")
                    .unwrap_or(0) as u32,
            )
            .build();
        let width = adw::SpinRow::with_range(1.0, 16_384.0, 1.0);
        width.set_title("Width");
        width.set_value(1920.0);
        width.set_digits(0);
        let height = adw::SpinRow::with_range(1.0, 16_384.0, 1.0);
        height.set_title("Height");
        height.set_value(1080.0);
        height.set_digits(0);
        let labels = COMMON_FRAME_RATES
            .iter()
            .map(|rate| rate.label)
            .collect::<Vec<_>>();
        let fps = adw::ComboRow::builder()
            .title("Frame Rate")
            .model(&gtk::StringList::new(&labels))
            .selected(
                COMMON_FRAME_RATES
                    .iter()
                    .position(|rate| rate.label == "30")
                    .unwrap_or(0) as u32,
            )
            .build();

        let updating = Rc::new(Cell::new(false));
        preset.connect_selected_notify({
            let width = width.clone();
            let height = height.clone();
            let fps = fps.clone();
            let updating = updating.clone();
            move |row| {
                let Some(selected) = PROJECT_PRESETS.get(row.selected() as usize) else {
                    return;
                };
                let Some(fps_index) = COMMON_FRAME_RATES
                    .iter()
                    .position(|rate| rate.value == selected.fps)
                else {
                    return;
                };
                updating.set(true);
                width.set_value(f64::from(selected.canvas_size.width));
                height.set_value(f64::from(selected.canvas_size.height));
                fps.set_selected(fps_index as u32);
                updating.set(false);
            }
        });
        let custom = PROJECT_PRESETS.len() as u32;
        width.connect_value_notify(mark_custom(&preset, &updating, custom));
        height.connect_value_notify(mark_custom(&preset, &updating, custom));
        fps.connect_selected_notify(mark_custom(&preset, &updating, custom));
        Self {
            preset,
            width,
            height,
            fps,
        }
    }

    pub fn settings(&self) -> Option<(CanvasSize, Fraction)> {
        let rate = COMMON_FRAME_RATES.get(self.fps.selected() as usize)?;
        Some((
            CanvasSize {
                width: self.width.value().round() as u32,
                height: self.height.value().round() as u32,
            },
            rate.value,
        ))
    }
}

impl Default for ProjectSettingsSelector {
    fn default() -> Self {
        Self::new()
    }
}

fn mark_custom<W: IsA<gtk::glib::Object>>(
    preset: &adw::ComboRow,
    updating: &Rc<Cell<bool>>,
    custom: u32,
) -> impl Fn(&W) + 'static {
    let preset = preset.clone();
    let updating = updating.clone();
    move |_| {
        if !updating.get() {
            preset.set_selected(custom);
        }
    }
}
