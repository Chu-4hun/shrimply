use super::*;

pub(super) fn scalar_expression_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: ScalarField,
) -> gtk::Widget {
    expression_editor(
        current_scalar(context, key.clone(), field)
            .and_then(|value| value.expression_source().map(str::to_string)),
        crate::rhai_editor::ExpressionValue::Scalar,
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            move |source| {
                update_expression_source(
                    &project,
                    &player_state,
                    key.clone(),
                    TransformField::Scalar(field),
                    source,
                )
            }
        },
    )
}

pub(super) fn vec2_expression_editor(
    context: &InspectorContext,
    key: SelectedItem,
    field: Vec2Field,
) -> gtk::Widget {
    expression_editor(
        current_vec2(context, key.clone(), field)
            .and_then(|value| value.expression_source().map(str::to_string)),
        crate::rhai_editor::ExpressionValue::Vec2,
        {
            let project = context.project.clone();
            let player_state = context.player_state.clone();
            move |source| {
                update_expression_source(
                    &project,
                    &player_state,
                    key.clone(),
                    TransformField::Vec2(field),
                    source,
                )
            }
        },
    )
}

fn expression_editor(
    source: Option<String>,
    value: crate::rhai_editor::ExpressionValue,
    update: impl Fn(String) + 'static,
) -> gtk::Widget {
    crate::rhai_editor::editor(source, value, update)
}

pub(super) fn set_expression_enabled(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    enabled: bool,
) -> bool {
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return false;
    };
    let changed = match field {
        TransformField::Vec2(field) => {
            let value = vec2_field_mut(transform, field);
            match &mut value.expression {
                Some(expression) => {
                    if expression.enabled == enabled {
                        false
                    } else {
                        expression.enabled = enabled;
                        true
                    }
                }
                None if enabled => {
                    value.expression = Some(TimelineExpression {
                        id: uuid::Uuid::new_v4(),
                        enabled: true,
                        source: vec2_expression_default(field),
                    });
                    true
                }
                None => false,
            }
        }
        TransformField::Scalar(field) => {
            let value = scalar_field_mut(transform, field);
            match &mut value.expression {
                Some(expression) => {
                    if expression.enabled == enabled {
                        false
                    } else {
                        expression.enabled = enabled;
                        true
                    }
                }
                None if enabled => {
                    value.expression = Some(TimelineExpression {
                        id: uuid::Uuid::new_v4(),
                        enabled: true,
                        source: scalar_expression_default(field),
                    });
                    true
                }
                None => false,
            }
        }
    };
    if !changed {
        return false;
    }
    shrimply_project::project::commit_edit(&project, "video-transform-expression");
    drop(project);
    player_state::refresh_project(
        player_state,
        ProjectChange {
            video: true,
            inspector: true,
            ..ProjectChange::default()
        },
    );
    true
}

fn update_expression_source(
    project: &Rc<RefCell<Project>>,
    player_state: &SharedPlayerState,
    key: SelectedItem,
    field: TransformField,
    source: String,
) {
    let mut project = project.borrow_mut();
    let Some(transform) = selected_transform_mut(&mut project, key.clone()) else {
        return;
    };
    match field {
        TransformField::Vec2(field) => {
            let Some(expression) = &mut vec2_field_mut(transform, field).expression else {
                return;
            };
            if expression.source == source {
                return;
            }
            expression.source = source;
        }
        TransformField::Scalar(field) => {
            let Some(expression) = &mut scalar_field_mut(transform, field).expression else {
                return;
            };
            if expression.source == source {
                return;
            }
            expression.source = source;
        }
    }
    shrimply_project::project::commit_coalesced_edit(&project, "transform-expression");
    drop(project);
    refresh_video(player_state);
}

fn scalar_expression_default(_field: ScalarField) -> String {
    "value".to_string()
}

fn vec2_expression_default(_field: Vec2Field) -> String {
    "[x, y]".to_string()
}
