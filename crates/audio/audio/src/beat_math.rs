use rayon::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct BeatTrack {
    pub(crate) beat_frames: Vec<u64>,
    pub(crate) period_frames: u64,
    pub(crate) bar_phase: Option<u8>,
    pub(crate) confidence: f32,
}

pub(crate) fn detect_beats(samples: &[f32], sample_rate: u32) -> Option<BeatTrack> {
    use easyfft::dyn_size::realfft::DynRealFft;

    const FFT_SIZE: usize = 2_048;
    const HOP_SIZE: usize = 512;
    const MIN_BPM: usize = 40;
    const MAX_BPM: usize = 208;

    if sample_rate == 0 || samples.len() < FFT_SIZE * 2 {
        return None;
    }

    let window = (0..FFT_SIZE)
        .map(|index| {
            0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (FFT_SIZE - 1) as f32).cos()
        })
        .collect::<Vec<_>>();
    let max_bin = (8_000_u64 * FFT_SIZE as u64 / u64::from(sample_rate))
        .clamp(1, (FFT_SIZE / 2) as u64) as usize;
    let mut previous = vec![0.0_f32; max_bin + 1];
    let mut onset = Vec::with_capacity(samples.len() / HOP_SIZE);
    let mut frame = vec![0.0_f32; FFT_SIZE];
    for start in (0..=samples.len() - FFT_SIZE).step_by(HOP_SIZE) {
        for (destination, (&sample, &weight)) in frame
            .iter_mut()
            .zip(samples[start..start + FFT_SIZE].iter().zip(&window))
        {
            *destination = sample * weight;
        }
        let spectrum = frame.real_fft();
        let mut flux = 0.0_f32;
        for (index, bin) in spectrum.iter().take(max_bin + 1).enumerate() {
            let magnitude = bin.norm_sqr().ln_1p();
            flux += (magnitude - previous[index]).max(0.0);
            previous[index] = magnitude;
        }
        onset.push(flux / (max_bin + 1) as f32);
    }
    if onset.len() < 8 {
        return None;
    }

    let mut normalized = onset
        .par_iter()
        .enumerate()
        .map(|(index, value)| {
            let start = index.saturating_sub(8);
            let end = (index + 9).min(onset.len());
            (*value - median(&onset[start..end])).max(0.0)
        })
        .collect::<Vec<_>>();
    let peak = normalized.iter().copied().fold(0.0_f32, f32::max);
    if peak <= f32::EPSILON {
        return None;
    }
    for value in &mut normalized {
        *value /= peak;
    }

    let frames_per_minute = u64::from(sample_rate) * 60;
    let min_lag = (frames_per_minute / (MAX_BPM as u64 * HOP_SIZE as u64)).max(1) as usize;
    let max_lag = (frames_per_minute / (MIN_BPM as u64 * HOP_SIZE as u64))
        .min(normalized.len().saturating_sub(1) as u64) as usize;
    if min_lag > max_lag {
        return None;
    }

    let correlations = (min_lag..=max_lag)
        .into_par_iter()
        .map(|lag| {
            let correlation = normalized[lag..]
                .iter()
                .zip(&normalized[..normalized.len() - lag])
                .map(|(right, left)| right * left)
                .sum::<f32>()
                / (normalized.len() - lag) as f32;
            let bpm = frames_per_minute as f32 / (lag * HOP_SIZE) as f32;
            let prior = (-0.5 * (bpm / 120.0).log2().powi(2) / 0.5_f32.powi(2)).exp();
            (lag, correlation * prior)
        })
        .collect::<Vec<_>>();
    let correlation_median = median(
        &correlations
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>(),
    );
    let &(period, correlation_peak) = correlations
        .iter()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))?;
    if correlation_peak <= f32::EPSILON {
        return None;
    }
    let period_curve =
        dynamic_period_curve(&normalized, sample_rate, HOP_SIZE, min_lag, max_lag, period);

    let mut cumulative = normalized.clone();
    let mut predecessor = vec![None; normalized.len()];
    for index in 0..normalized.len() {
        let expected_period = period_curve[index];
        let earliest = (expected_period / 2).max(1);
        let latest = expected_period.saturating_mul(2);
        let first = index.saturating_sub(latest);
        let last = index.saturating_sub(earliest);
        if first >= last {
            continue;
        }
        if let Some((best, score)) = (first..=last)
            .map(|candidate| {
                let ratio = (index - candidate) as f32 / expected_period as f32;
                let score = cumulative[candidate] - 8.0 * ratio.log2().powi(2);
                (candidate, score)
            })
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            && score > 0.0
        {
            cumulative[index] += score;
            predecessor[index] = Some(best);
        }
    }

    let mut cursor = cumulative
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))?
        .0;
    let mut beat_indices = Vec::new();
    loop {
        beat_indices.push(cursor);
        let Some(previous) = predecessor[cursor] else {
            break;
        };
        cursor = previous;
    }
    beat_indices.reverse();
    while beat_indices
        .first()
        .is_some_and(|index| normalized[*index] < 0.05)
    {
        beat_indices.remove(0);
    }
    while beat_indices
        .last()
        .is_some_and(|index| normalized[*index] < 0.05)
    {
        beat_indices.pop();
    }
    if beat_indices.len() < 4 {
        return None;
    }

    let prominence = ((correlation_peak - correlation_median) / correlation_peak).clamp(0.0, 1.0);
    let onset_support = beat_indices
        .iter()
        .map(|index| normalized[*index])
        .sum::<f32>()
        / beat_indices.len() as f32;
    let confidence = (prominence + onset_support) * 0.5;
    let phase_scores = std::array::from_fn::<_, 4, _>(|phase| {
        let (sum, count) = beat_indices
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 4 == phase)
            .map(|(_, beat)| normalized[*beat])
            .fold((0.0_f32, 0_u32), |(sum, count), value| {
                (sum + value, count + 1)
            });
        sum / count.max(1) as f32
    });
    let mut phases = (0..4).collect::<Vec<_>>();
    phases.sort_by(|left, right| phase_scores[*right].total_cmp(&phase_scores[*left]));
    let bar_phase =
        (phase_scores[phases[0]] >= phase_scores[phases[1]] * 1.1).then_some(phases[0] as u8);

    Some(BeatTrack {
        beat_frames: beat_indices
            .into_iter()
            .map(|index| (index * HOP_SIZE) as u64)
            .collect(),
        period_frames: (period * HOP_SIZE) as u64,
        bar_phase,
        confidence,
    })
}

fn dynamic_period_curve(
    onset: &[f32],
    sample_rate: u32,
    hop_size: usize,
    min_lag: usize,
    max_lag: usize,
    global_period: usize,
) -> Vec<usize> {
    const WINDOW_SECONDS: usize = 8;
    const STEP_SECONDS: usize = 1;
    const TEMPO_SMOOTHNESS: f32 = 2.0;
    const GLOBAL_PRIOR_WEIGHT: f32 = 0.2;

    let frames_per_second = (sample_rate as usize / hop_size).max(1);
    let anchor_step = frames_per_second.saturating_mul(STEP_SECONDS);
    let half_window = frames_per_second.saturating_mul(WINDOW_SECONDS) / 2;
    let mut anchors = (0..onset.len()).step_by(anchor_step).collect::<Vec<_>>();
    if anchors.last().copied() != Some(onset.len() - 1) {
        anchors.push(onset.len() - 1);
    }
    let lags = (min_lag..=max_lag).collect::<Vec<_>>();
    if anchors.len() < 2 || lags.is_empty() {
        return vec![global_period; onset.len()];
    }

    let scores = anchors
        .par_iter()
        .map(|&anchor| {
            let window_start = anchor.saturating_sub(half_window);
            let window_end = anchor.saturating_add(half_window).min(onset.len());
            let mut row = lags
                .iter()
                .map(|&lag| {
                    let first = window_start.saturating_add(lag);
                    if first >= window_end {
                        return 0.0;
                    }
                    onset[first..window_end]
                        .iter()
                        .zip(&onset[window_start..window_end - lag])
                        .map(|(right, left)| right * left)
                        .sum::<f32>()
                        / (window_end - first) as f32
                })
                .collect::<Vec<_>>();
            let peak = row.iter().copied().fold(0.0_f32, f32::max);
            for (score, &lag) in row.iter_mut().zip(&lags) {
                if peak > f32::EPSILON {
                    *score /= peak;
                }
                let global_prior = (-0.5 * (lag as f32 / global_period as f32).log2().powi(2)
                    / 0.75_f32.powi(2))
                .exp();
                *score += GLOBAL_PRIOR_WEIGHT * global_prior;
            }
            row
        })
        .collect::<Vec<_>>();

    let mut cumulative = scores[0].clone();
    let mut predecessors = vec![vec![0_u16; lags.len()]; anchors.len()];
    for anchor in 1..anchors.len() {
        let (next, predecessor): (Vec<_>, Vec<_>) = lags
            .par_iter()
            .enumerate()
            .map(|(lag_index, &lag)| {
                let (previous, score) = lags
                    .iter()
                    .enumerate()
                    .map(|(previous_index, &previous_lag)| {
                        let ratio = lag as f32 / previous_lag as f32;
                        let score =
                            cumulative[previous_index] - TEMPO_SMOOTHNESS * ratio.log2().powi(2);
                        (previous_index, score)
                    })
                    .max_by(|(_, left), (_, right)| left.total_cmp(right))
                    .expect("tempo lag candidates are not empty");
                (scores[anchor][lag_index] + score, previous as u16)
            })
            .unzip();
        predecessors[anchor] = predecessor;
        cumulative = next;
    }

    let mut lag_index = cumulative
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index)
        .unwrap_or_default();
    let mut anchor_periods = vec![global_period; anchors.len()];
    for anchor in (0..anchors.len()).rev() {
        anchor_periods[anchor] = lags[lag_index];
        if anchor > 0 {
            lag_index = usize::from(predecessors[anchor][lag_index]);
        }
    }

    let mut curve = vec![global_period; onset.len()];
    for anchor in 0..anchors.len() - 1 {
        let start = anchors[anchor];
        let end = anchors[anchor + 1];
        let left = anchor_periods[anchor] as f32;
        let right = anchor_periods[anchor + 1] as f32;
        let span = (end - start).max(1) as f32;
        for (offset, period) in curve[start..=end].iter_mut().enumerate() {
            let mix = offset as f32 / span;
            *period = (left + (right - left) * mix).round().max(1.0) as usize;
        }
    }
    curve
}

fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut values = values.to_vec();
    let middle = values.len() / 2;
    values.select_nth_unstable_by(middle, f32::total_cmp);
    if values.len().is_multiple_of(2) {
        let left = values[..middle]
            .iter()
            .copied()
            .max_by(f32::total_cmp)
            .unwrap_or(values[middle]);
        (left + values[middle]) * 0.5
    } else {
        values[middle]
    }
}
