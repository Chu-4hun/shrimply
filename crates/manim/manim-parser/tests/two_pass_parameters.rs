use hashbrown::HashMap;
use shrimply_asset::Asset;
use shrimply_manim_parser::{ProgressStage, Settings, compile_uncancelled, reflected_parameters};
use shrimply_math_core::fraction_new;
use shrimply_project::project::ManimParameterValue;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn settings(source: &str, scene: &str) -> Settings {
    Settings {
        source: Asset::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(source),
        ),
        scene: scene.to_string(),
        width: 320,
        height: 180,
        fps: fraction_new(10, 1),
        parameters: HashMap::new(),
    }
}

#[test]
#[ignore = "requires the Shrimply Manim Python environment"]
fn frame_packets_report_contiguous_progress() {
    let settings = settings("stream_progress.py", "StreamProgress");
    let mut progress = Vec::new();
    let started = Instant::now();
    let animation = compile_uncancelled(&settings, |next| {
        progress.push((next.stage, next.completed, next.total, started.elapsed()));
    })
    .expect("compile streaming fixture");
    let finished = started.elapsed();
    assert_eq!(animation.frames().len(), 3);
    assert_eq!(
        progress
            .iter()
            .map(|(stage, completed, total, _)| (*stage, *completed, *total))
            .collect::<Vec<_>>(),
        [
            (ProgressStage::LoadingScene, 0, 0),
            (ProgressStage::StreamingFrames, 1, 0),
            (ProgressStage::StreamingFrames, 2, 0),
            (ProgressStage::StreamingFrames, 3, 0),
        ]
    );
    let frame_times = progress
        .iter()
        .filter_map(|(stage, _, _, received)| {
            (*stage == ProgressStage::StreamingFrames).then_some(*received)
        })
        .collect::<Vec<_>>();
    assert!(
        frame_times
            .windows(2)
            .all(|times| times[1] - times[0] >= Duration::from_millis(100)),
        "frame progress was buffered instead of streamed: {frame_times:?}",
    );
    assert!(
        finished - frame_times[0] >= Duration::from_millis(300),
        "the first frame was not received while the worker was still running",
    );
}

#[test]
#[ignore = "requires the Shrimply Manim Python environment"]
fn discovered_defaults_trigger_exactly_one_retry() {
    let mut settings = settings("two_pass_parameters.py", "TwoPassParameters");
    let first = compile_uncancelled(&settings, |_| {}).expect("compile unconfigured fixture");
    assert!(!first.scene().render_is_current);
    assert_eq!(first.scene().duration, fraction_new(1, 10));
    assert_eq!(first.frames().len(), 1);
    let parameters = reflected_parameters(&first).expect("decode discovered parameters");
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].key, "imported-scale");
    assert_eq!(parameters[0].value, ManimParameterValue::Integer(2));
    assert_eq!(parameters[1].key, "construct-hold");
    assert_eq!(
        parameters[1].value,
        ManimParameterValue::Fraction {
            numerator: 1,
            denominator: 10,
        }
    );

    settings.parameters = parameters
        .into_iter()
        .map(|parameter| (parameter.key, parameter.value))
        .collect();
    let current = compile_uncancelled(&settings, |_| {}).expect("compile configured fixture");
    assert!(current.scene().render_is_current);
    let cached = compile_uncancelled(&settings, |_| {}).expect("reuse configured fixture");
    assert!(Arc::ptr_eq(&current, &cached));
}

#[test]
#[ignore = "requires the Shrimply Manim Python environment"]
fn existing_values_render_current_on_the_first_attempt() {
    let mut settings = settings("two_pass_parameters.py", "TwoPassParameters");
    settings.parameters.insert(
        "imported-scale".to_string(),
        ManimParameterValue::Integer(4),
    );
    settings.parameters.insert(
        "construct-hold".to_string(),
        ManimParameterValue::Fraction {
            numerator: 1,
            denominator: 5,
        },
    );
    settings.parameters.insert(
        "unused-existing-value".to_string(),
        ManimParameterValue::Integer(99),
    );

    let animation = compile_uncancelled(&settings, |_| {}).expect("compile configured fixture");
    assert!(animation.scene().render_is_current);
    assert_eq!(animation.scene().duration, fraction_new(1, 5));
    assert_eq!(animation.frames().len(), 2);
    let parameters = reflected_parameters(&animation).expect("decode reflected parameters");
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].key, "imported-scale");
    assert_eq!(parameters[0].default, ManimParameterValue::Integer(2));
    assert_eq!(parameters[0].value, ManimParameterValue::Integer(4));
    assert_eq!(parameters[1].key, "construct-hold");
    assert_eq!(
        parameters[1].value,
        ManimParameterValue::Fraction {
            numerator: 1,
            denominator: 5,
        }
    );
}

#[test]
#[ignore = "requires the Shrimply Manim Python environment"]
fn legacy_fraction_values_trigger_one_migration_retry() {
    let mut settings = settings("two_pass_parameters.py", "TwoPassParameters");
    settings.parameters.insert(
        "imported-scale".to_string(),
        ManimParameterValue::Integer(4),
    );
    settings.parameters.insert(
        "construct-hold".to_string(),
        ManimParameterValue::Float(0.2),
    );

    let first = compile_uncancelled(&settings, |_| {}).expect("compile legacy parameter fixture");
    assert!(!first.scene().render_is_current);
    let parameters = reflected_parameters(&first).expect("decode migrated parameters");
    assert_eq!(
        parameters[1].value,
        ManimParameterValue::Fraction {
            numerator: 1,
            denominator: 5,
        }
    );
    settings.parameters = parameters
        .into_iter()
        .map(|parameter| (parameter.key, parameter.value))
        .collect();
    let current = compile_uncancelled(&settings, |_| {}).expect("retry migrated fixture");
    assert!(current.scene().render_is_current);
}

#[test]
#[ignore = "requires the Shrimply Manim Python environment"]
fn conditional_parameters_trigger_one_retry() {
    let mut settings = settings("schema_drift.py", "SchemaDrift");
    settings
        .parameters
        .insert("mode".to_string(), ManimParameterValue::Boolean(true));
    let first = compile_uncancelled(&settings, |_| {}).expect("compile dynamic parameter fixture");
    assert!(!first.scene().render_is_current);
    let parameters = reflected_parameters(&first).expect("decode dynamic parameters");
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].key, "mode");
    assert_eq!(parameters[0].value, ManimParameterValue::Boolean(true));
    assert_eq!(parameters[1].key, "extra");
    settings.parameters = parameters
        .into_iter()
        .map(|parameter| (parameter.key, parameter.value))
        .collect();
    let current = compile_uncancelled(&settings, |_| {}).expect("retry dynamic parameter fixture");
    assert!(current.scene().render_is_current);
}
