use super::*;

#[derive(Default)]
pub(super) struct ManimParameters(Vec<ManimParameter>);

pub(super) fn item(parameters: Vec<ManimParameter>) -> InspectorListItem {
    let reset_keys = parameters
        .iter()
        .map(|parameter| parameter.key.clone())
        .collect::<Vec<_>>();
    DefaultInspectorItem::new(
        "manim-parameters",
        "Parameters",
        ManimParameters(parameters),
        controls,
        move |context, _: ManimParameters| {
            let reset_keys = reset_keys.clone();
            apply_video_reset(context, "reset-manim-parameters", move |item| {
                let VideoItemContent::Manim(manim) = &mut item.content else {
                    return;
                };
                manim.parameters.retain(|key, _| !reset_keys.contains(key));
            });
        },
    )
    .boxed()
}

fn controls(parameters: &ManimParameters, context: &InspectorContext) -> Vec<gtk::Widget> {
    let section = InspectorSection::controls();
    let Some(item_key) = context.selected_item.clone() else {
        return vec![section.into_widget()];
    };
    for parameter in &parameters.0 {
        match (&parameter.control, &parameter.value) {
            (ManimParameterControl::AntiAliasing, _) => {}
            (
                ManimParameterControl::Integer {
                    minimum,
                    maximum,
                    step,
                },
                ManimParameterValue::Integer(value),
            ) => {
                let mut builder = shrimply_ui_foundation::ui::NumberPicker::integer_builder(*value)
                    .drag_step(*step as f64);
                if let Some(minimum) = minimum {
                    builder = builder.minimum(*minimum as f64);
                }
                if let Some(maximum) = maximum {
                    builder = builder.maximum(*maximum as f64);
                }
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = builder
                    .on_commit_integer(move |value: i64| {
                        update(ManimParameterValue::Integer(value));
                    })
                    .build();
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            (
                ManimParameterControl::Float {
                    minimum,
                    maximum,
                    step,
                },
                ManimParameterValue::Float(value),
            ) => {
                let digits = format!("{step:.8}")
                    .trim_end_matches('0')
                    .split_once('.')
                    .map_or(0, |(_, fraction)| fraction.len());
                let mut builder = shrimply_ui_foundation::ui::NumberPicker::builder(*value)
                    .drag_step(*step)
                    .digits(digits);
                if let Some(minimum) = minimum {
                    builder = builder.minimum(*minimum);
                }
                if let Some(maximum) = maximum {
                    builder = builder.maximum(*maximum);
                }
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = builder
                    .on_commit(move |value| update(ManimParameterValue::Float(value)))
                    .build();
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            (
                ManimParameterControl::Fraction,
                ManimParameterValue::Fraction {
                    numerator,
                    denominator,
                },
            ) => {
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = shrimply_ui_foundation::ui::NumberPicker::fraction_builder(
                    fraction_new(*numerator, *denominator),
                )
                .drag_step(0.05)
                .digits(2)
                .on_commit_fraction(move |value| {
                    update(ManimParameterValue::Fraction {
                        numerator: fraction_numerator(value),
                        denominator: fraction_denominator(value),
                    });
                })
                .build();
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            (ManimParameterControl::Color, ManimParameterValue::Color(value)) => {
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = shrimply_ui_foundation::ui::ColorPicker::builder(*value)
                    .title(&parameter.label)
                    .with_alpha(false)
                    .hexpand(true)
                    .on_change(move |value| update(ManimParameterValue::Color(value)))
                    .build();
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            (ManimParameterControl::Option { options }, ManimParameterValue::Option(value)) => {
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = shrimply_ui_foundation::ui::string_selector(
                    &parameter.label,
                    value,
                    options.clone(),
                    move |value| update(ManimParameterValue::Option(value)),
                );
                section.add_wide_control(control.widget());
            }
            (ManimParameterControl::Boolean, ManimParameterValue::Boolean(value)) => {
                let control = gtk::Switch::builder()
                    .active(*value)
                    .halign(gtk::Align::End)
                    .build();
                let update = parameter_update(context, item_key.clone(), parameter);
                control.connect_active_notify(move |control| {
                    update(ManimParameterValue::Boolean(control.is_active()));
                });
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            (ManimParameterControl::String, ManimParameterValue::String(value)) => {
                let update = parameter_update(context, item_key.clone(), parameter);
                let control = shrimply_ui_foundation::ui::SingleLineTextInput::builder(value)
                    .on_commit(move |value| update(ManimParameterValue::String(value)))
                    .build();
                section.add_wide_control(&shrimply_ui_foundation::ui::control_row(
                    &parameter.label,
                    &control,
                ));
            }
            _ => tracing::warn!(
                key = %parameter.key,
                "Ignoring mismatched reflected Manim parameter metadata"
            ),
        }
    }
    vec![section.into_widget()]
}

pub(super) fn parameter_update(
    context: &InspectorContext,
    item_key: SelectedItem,
    parameter: &ManimParameter,
) -> impl Fn(ManimParameterValue) + 'static {
    let project = context.project.clone();
    let player_state = context.player_state.clone();
    let parameter_key = parameter.key.clone();
    move |value| {
        let parameter_key = parameter_key.clone();
        update_video_item(
            &project,
            &player_state,
            item_key.clone(),
            "manim-parameter",
            move |item| {
                let VideoItemContent::Manim(manim) = &mut item.content else {
                    return false;
                };
                if manim.parameters.get(&parameter_key) == Some(&value) {
                    return false;
                }
                manim.parameters.insert(parameter_key, value);
                true
            },
        );
    }
}
