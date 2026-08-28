use hashbrown::HashMap;

use shrimply_audio::streaming::{AudioRenderSession, AudioSourceKey, mix_project_range};
use shrimply_math_core::Time;
use shrimply_project::project::Project;

const CHANNELS: usize = 2;
pub const SAMPLE_RATE: u32 = 16_000;

pub struct PreparedTranscriptionChunk {
    pub start: Time,
    pub end: Time,
    pub samples: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct TranscribedSegment {
    pub start: Time,
    pub end: Time,
    pub text: String,
}

pub fn prepare_transcription_chunks(
    project: &Project,
    ranges: &[(Time, Time)],
) -> Result<Vec<PreparedTranscriptionChunk>, String> {
    if ranges.is_empty() {
        return Err("Selected audio has no duration".to_string());
    }
    if ranges.iter().any(|(start, end)| end <= start) {
        return Err("Selected audio has no duration".to_string());
    }

    let mut chunks = Vec::new();
    for &(start, end) in ranges {
        let samples = mix_range_mono(project, start, end);
        if samples.iter().all(|sample| sample.abs() <= f32::EPSILON) {
            continue;
        }
        chunks.push(PreparedTranscriptionChunk {
            start,
            end,
            samples,
        });
    }
    if chunks.is_empty() {
        return Err("Selected audio decoded to silence".to_string());
    }
    Ok(chunks)
}

fn mix_range_mono(project: &Project, start: Time, end: Time) -> Vec<f32> {
    let start_frame = start.as_sample_frame(SAMPLE_RATE);
    let end_frame = end.as_sample_frame(SAMPLE_RATE);
    let frame_count = end_frame.saturating_sub(start_frame) as usize;
    let mut sessions: HashMap<AudioSourceKey, AudioRenderSession> = HashMap::new();
    let stereo = mix_project_range(
        project,
        &mut sessions,
        start_frame,
        frame_count,
        SAMPLE_RATE,
    );
    stereo
        .chunks_exact(CHANNELS)
        .map(|frame| {
            let sample = frame[0] * 0.5 + frame[1] * 0.5;
            if sample.is_finite() { sample } else { 0.0 }
        })
        .collect()
}
