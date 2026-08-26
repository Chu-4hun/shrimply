use crate::{
    player_state::{self, ProjectChange},
    time_format,
    ui::{NumberPicker, SingleLineTextInput, dropdown},
};
use adw::prelude::*;
use shrimply_project::project::Project;

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
        let commit_project = context.project.clone();
        let name_player_state = context.player_state.clone();
        let name = SingleLineTextInput::builder(&self.name)
            .on_change(move |next| {
                let mut project = name_project.borrow_mut();
                if project.name != next {
                    project.name = next;
                }
            })
            .on_commit(move |_| {
                player_state::refresh_project(&name_player_state, ProjectChange::default());
                shrimply_project::project::commit_edit(&commit_project.borrow(), "project-name");
            })
            .build();
        config.add_control_row("Name", &name);

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
        let fps_project = context.project.clone();
        let fps_player_state = context.player_state.clone();
        let fps = dropdown(self.fps, fps_options, move |next| {
            let mut project = fps_project.borrow_mut();
            if project.fps == next {
                return;
            }
            project.fps = next;
            shrimply_project::project::commit_edit(&project, "project-fps");
            drop(project);
            player_state::refresh_project(
                &fps_player_state,
                ProjectChange {
                    video: true,
                    captions: true,
                    inspector: true,
                    ..ProjectChange::default()
                },
            );
        });
        config.add_control_row("FPS", &fps);

        let width_project = context.project.clone();
        let width_player_state = context.player_state.clone();
        config.add_control_row(
            "Canvas Width",
            &NumberPicker::integer_builder(self.canvas_size.width)
                .minimum(MIN_CANVAS_DIMENSION)
                .maximum(MAX_CANVAS_DIMENSION)
                .on_change_integer(move |next: u32| {
                    let mut project = width_project.borrow_mut();
                    if project.canvas_size.width == next {
                        return;
                    }
                    project.canvas_size.width = next;
                    shrimply_project::project::commit_coalesced_edit(
                        &project,
                        "project-canvas-size",
                    );
                    drop(project);
                    player_state::refresh_project(
                        &width_player_state,
                        ProjectChange {
                            video: true,
                            captions: true,
                            ..ProjectChange::default()
                        },
                    );
                })
                .build(),
        );

        let height_project = context.project.clone();
        let height_player_state = context.player_state.clone();
        config.add_control_row(
            "Canvas Height",
            &NumberPicker::integer_builder(self.canvas_size.height)
                .minimum(MIN_CANVAS_DIMENSION)
                .maximum(MAX_CANVAS_DIMENSION)
                .on_change_integer(move |next: u32| {
                    let mut project = height_project.borrow_mut();
                    if project.canvas_size.height == next {
                        return;
                    }
                    project.canvas_size.height = next;
                    shrimply_project::project::commit_coalesced_edit(
                        &project,
                        "project-canvas-size",
                    );
                    drop(project);
                    player_state::refresh_project(
                        &height_player_state,
                        ProjectChange {
                            video: true,
                            captions: true,
                            ..ProjectChange::default()
                        },
                    );
                })
                .build(),
        );

        let info = adw::PreferencesGroup::new();
        info.add(
            &adw::ActionRow::builder()
                .title("Tracks")
                .subtitle(format!(
                    "{} video, {} audio, {} caption",
                    self.video_tracks.len(),
                    self.audio_tracks.len(),
                    self.caption_tracks.len()
                ))
                .build(),
        );
        info.add(
            &adw::ActionRow::builder()
                .title("Duration")
                .subtitle(time_format::project_duration(self.duration()))
                .build(),
        );

        let project_path = shrimply_project::project::active_project_path();
        let file = adw::ActionRow::builder()
            .title("Project File")
            .subtitle(project_path.to_string_lossy())
            .build();
        let show_file = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Show project file in folder")
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
                dialog.add_response("close", "Close");
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
