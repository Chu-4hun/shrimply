use crate::{
    player_state::{self, ProjectChange},
    time_format,
    ui::{Number2Picker, SingleLineTextInput, dropdown},
};
use adw::prelude::*;
use shrimply_project::project::Project;
use shrimply_ui_foundation::{tr, ui::I18nAlertDialogExt};
use std::{cell::Cell, rc::Rc};

use super::{Inspectable, InspectorContext, item::flat, list, section::InspectorSection};

const MIN_CANVAS_DIMENSION: f64 = 1.0;
const MAX_CANVAS_DIMENSION: f64 = 16_384.0;

impl Inspectable for Project {
    fn title(&self) -> &'static str {
        "Project"
    }

    fn add_rows(&self, _section: &InspectorSection, _context: &InspectorContext) {}

    fn inspect(&self, context: &InspectorContext) -> Vec<gtk::Widget> {
        let config = InspectorSection::controls();

        let name_project = context.project.clone();
        let name_player_state = context.player_state.clone();
        let name = SingleLineTextInput::builder(&self.name)
            .on_change(move |next| {
                let mut project = name_project.borrow_mut();
                if project.name == next {
                    return;
                }
                project.name = next;
                shrimply_project::project::commit_coalesced_edit(&project, "project-name");
                drop(project);
                player_state::refresh_project(&name_player_state, ProjectChange::default());
            })
            .build();
        config.add_control_row("Name", &name);

        let initial_fps = self.fps;
        let initial_width = self.canvas_size.width;
        let initial_height = self.canvas_size.height;
        let staged_fps = Rc::new(Cell::new(initial_fps));
        let staged_width = Rc::new(Cell::new(initial_width));
        let staged_height = Rc::new(Cell::new(initial_height));

        let discard = gtk::Button::with_label(tr!("Discard").as_ref());
        let apply = gtk::Button::with_label(tr!("Apply").as_ref());
        apply.add_css_class("suggested-action");
        let actions = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::End)
            .build();
        actions.append(&discard);
        actions.append(&apply);
        actions.set_visible(false);

        let update_actions: Rc<dyn Fn()> = {
            let actions = actions.clone();
            let staged_fps = staged_fps.clone();
            let staged_width = staged_width.clone();
            let staged_height = staged_height.clone();
            Rc::new(move || {
                actions.set_visible(
                    staged_fps.get() != initial_fps
                        || staged_width.get() != initial_width
                        || staged_height.get() != initial_height,
                );
            })
        };

        let mut fps_options = shrimply_project::project::COMMON_FRAME_RATES
            .iter()
            .map(|rate| (rate.value, rate.label.to_string()))
            .collect::<Vec<_>>();
        if fps_options.iter().all(|(fps, _)| *fps != self.fps) {
            fps_options.push((
                self.fps,
                shrimply_project::project::fraction_as_label(self.fps),
            ));
        }
        let fps = dropdown(self.fps, fps_options, {
            let staged_fps = staged_fps.clone();
            let update_actions = update_actions.clone();
            move |next| {
                staged_fps.set(next);
                update_actions();
            }
        });
        config.add_control_row("FPS", &fps);

        let resolution = Number2Picker::builder(
            f64::from(self.canvas_size.width),
            f64::from(self.canvas_size.height),
        )
        .minimum(MIN_CANVAS_DIMENSION)
        .maximum(MAX_CANVAS_DIMENSION)
        .drag_step(1.0)
        .digits(0)
        .width_chars(7)
        .first_prefix("W")
        .second_prefix("H")
        .enable_lock()
        .on_first_change({
            let staged_width = staged_width.clone();
            let update_actions = update_actions.clone();
            move |next| {
                staged_width.set(next.round() as u32);
                update_actions();
            }
        })
        .on_second_change({
            let staged_height = staged_height.clone();
            let update_actions = update_actions.clone();
            move |next| {
                staged_height.set(next.round() as u32);
                update_actions();
            }
        })
        .build_with_handles();
        config.add_control_row("Resolution", &resolution.widget);
        config.add_wide_control(&actions);

        let refresh_inspector = context.refresh.clone();
        discard.connect_clicked(move |_| refresh_inspector());

        let apply_project = context.project.clone();
        let apply_player_state = context.player_state.clone();
        apply.connect_clicked(move |button| {
            let dialog = adw::AlertDialog::new(
                Some(tr!("Change Project Settings?").as_ref()),
                Some(
                    tr!("Changing the frame rate or resolution can affect timing, visual layout, and rendered output. Existing media and effects may no longer match the project.")
                        .as_ref(),
                ),
            );
            dialog.add_responses_i18n(&[("cancel", "Cancel"), ("apply", "Apply")]);
            dialog.set_close_response("cancel");
            dialog.set_default_response(Some("cancel"));
            dialog.set_response_appearance("apply", adw::ResponseAppearance::Destructive);

            let project = apply_project.clone();
            let player_state = apply_player_state.clone();
            let next_fps = staged_fps.get();
            let next_width = staged_width.get();
            let next_height = staged_height.get();
            dialog.choose(
                Some(button.upcast_ref::<gtk::Widget>()),
                None::<&gtk::gio::Cancellable>,
                move |response| {
                    if response.as_str() != "apply" {
                        return;
                    }
                    let mut project = project.borrow_mut();
                    let fps_changed = project.fps != next_fps;
                    let resolution_changed = project.canvas_size.width != next_width
                        || project.canvas_size.height != next_height;
                    if !fps_changed && !resolution_changed {
                        return;
                    }
                    project.fps = next_fps;
                    project.canvas_size.width = next_width;
                    project.canvas_size.height = next_height;
                    shrimply_project::project::commit_edit(&project, "project-settings");
                    drop(project);
                    player_state::refresh_project(
                        &player_state,
                        ProjectChange {
                            frame_rate: fps_changed.then_some(next_fps),
                            video: true,
                            captions: true,
                            inspector: true,
                            ..ProjectChange::default()
                        },
                    );
                },
            );
        });

        let info = adw::PreferencesGroup::new();
        let track_count = |count: usize, singular: &str, plural: &str| {
            if count == 1 {
                tr!(singular).into_owned()
            } else {
                shrimply_ui_foundation::i18n::text_args(plural, &[("count", count.to_string())])
            }
        };
        let track_counts = [
            track_count(
                self.video_tracks.len(),
                "1 video track",
                "%{count} video tracks",
            ),
            track_count(
                self.audio_tracks.len(),
                "1 audio track",
                "%{count} audio tracks",
            ),
            track_count(
                self.caption_tracks.len(),
                "1 caption track",
                "%{count} caption tracks",
            ),
        ]
        .join(", ");
        info.add(
            &adw::ActionRow::builder()
                .title(tr!("Tracks").as_ref())
                .subtitle(track_counts)
                .build(),
        );
        info.add(
            &adw::ActionRow::builder()
                .title(tr!("Duration").as_ref())
                .subtitle(time_format::project_duration(self.duration()))
                .build(),
        );

        let project_path = shrimply_project::project::active_project_path();
        let file = adw::ActionRow::builder()
            .title(tr!("Project File").as_ref())
            .subtitle(project_path.to_string_lossy())
            .build();
        let show_file = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(tr!("Show project file in folder").as_ref())
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let reveal_path = project_path.clone();
        show_file.connect_clicked(move |button| {
            if let Err(error) =
                crate::desktop_open::show_path_in_folder(button.upcast_ref(), reveal_path.clone())
            {
                let dialog =
                    adw::AlertDialog::new(Some("Could not show project file"), Some(&error));
                dialog.add_response("close", tr!("Close").as_ref());
                dialog.present(Some(button));
            }
        });
        file.add_suffix(&show_file);
        file.set_activatable_widget(Some(&show_file));
        info.add(&file);

        list::render_categories(
            vec![
                list::InspectorCategory {
                    key: "config",
                    label: "Project",
                    icon: "sliders-horizontal-symbolic",
                    items: vec![flat(config.into_widget())],
                },
                list::InspectorCategory {
                    key: "info",
                    label: "Info",
                    icon: "info-outline-symbolic",
                    items: vec![flat(info)],
                },
                list::InspectorCategory {
                    key: "performance",
                    label: "Performance",
                    icon: "speedometer-symbolic",
                    items: vec![flat(super::benchmarking::widget())],
                },
            ],
            context,
        )
    }
}
