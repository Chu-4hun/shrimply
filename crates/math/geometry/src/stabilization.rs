use glam::{Mat3, USizeVec2, UVec2, Vec2, Vec3};
use rayon::prelude::*;

/// Confidence-filtered coarse-to-fine motion transfer. A robust similarity model captures
/// translation, rotation, and zoom; only its local residual is transferred to the mesh.
pub fn stabilization_mesh_motion(
    tracks: &[[f32; 6]],
    grid_width: usize,
    grid_height: usize,
    width: u32,
    height: u32,
) -> (Mat3, Vec<Vec2>, bool) {
    const MINIMUM_TRACKS: usize = 12;
    const RANSAC_ITERATIONS: usize = 96;
    const RANSAC_THRESHOLD_RATIO: f32 = 0.01;
    const MINIMUM_RANSAC_THRESHOLD: f32 = 2.0;
    const MESH_RESIDUAL_THRESHOLD_MULTIPLIER: f32 = 4.0;
    const VERTEX_SUPPORT_CELLS: f32 = 1.5;

    let valid = tracks
        .iter()
        .enumerate()
        .filter_map(|(index, track)| {
            (track[5] != 0.0 && track.iter().all(|value| value.is_finite())).then_some(index)
        })
        .collect::<Vec<_>>();
    let grid_size = grid_width.saturating_mul(grid_height);
    if valid.len() < MINIMUM_TRACKS || grid_size == 0 {
        return (Mat3::IDENTITY, vec![Vec2::ZERO; grid_size], true);
    }

    let threshold =
        (width.min(height).max(1) as f32 * RANSAC_THRESHOLD_RATIO).max(MINIMUM_RANSAC_THRESHOLD);
    let best = (0..RANSAC_ITERATIONS)
        .into_par_iter()
        .filter_map(|iteration| {
            let count = valid.len();
            let sample = [
                valid[iteration % count],
                valid[(iteration * 7 + count / 5 + 1) % count],
            ];
            if sample[0] == sample[1] {
                return None;
            }
            let model = fit_track_similarity(tracks, &sample)?;
            let mut inliers = Vec::new();
            let mut confidence = 0.0;
            let mut error = 0.0;
            for &index in &valid {
                let residual = track_similarity_error(tracks[index], model);
                if residual <= threshold {
                    inliers.push(index);
                    let weight = tracks[index][4].max(f32::EPSILON);
                    confidence += weight;
                    error += residual * weight;
                }
            }
            Some((iteration, inliers, confidence, error))
        })
        .max_by(|left, right| {
            left.2
                .total_cmp(&right.2)
                .then_with(|| right.3.total_cmp(&left.3))
                .then_with(|| right.0.cmp(&left.0))
        })
        .map(|(_, inliers, _, _)| inliers)
        .unwrap_or_default();
    if best.len() < MINIMUM_TRACKS {
        return (Mat3::IDENTITY, vec![Vec2::ZERO; grid_size], true);
    }
    let Some(similarity) = fit_track_similarity(tracks, &best) else {
        return (Mat3::IDENTITY, vec![Vec2::ZERO; grid_size], true);
    };
    let mesh_threshold = threshold * MESH_RESIDUAL_THRESHOLD_MULTIPLIER;
    let mesh_tracks = valid
        .into_iter()
        .filter(|&index| track_similarity_error(tracks[index], similarity) <= mesh_threshold)
        .collect::<Vec<_>>();
    let cell_width = width.max(1) as f32 / grid_width.saturating_sub(1).max(1) as f32;
    let cell_height = height.max(1) as f32 / grid_height.saturating_sub(1).max(1) as f32;
    let support_x = cell_width * VERTEX_SUPPORT_CELLS;
    let support_y = cell_height * VERTEX_SUPPORT_CELLS;
    let transferred_residual = (0..grid_size)
        .into_par_iter()
        .map(|index| {
            let row = index / grid_width;
            let column = index % grid_width;
            let x = column as f32 * cell_width;
            let y = row as f32 * cell_height;
            let mut horizontal = Vec::new();
            let mut vertical = Vec::new();
            for &index in &mesh_tracks {
                let track = tracks[index];
                if (track[0] - x).abs() <= support_x && (track[1] - y).abs() <= support_y {
                    let global = similarity_motion(similarity, track[0], track[1]);
                    horizontal.push(track[2] - track[0] - global.x);
                    vertical.push(track[3] - track[1] - global.y);
                }
            }
            if horizontal.is_empty() {
                Vec2::ZERO
            } else {
                Vec2::new(median(&mut horizontal), median(&mut vertical))
            }
        })
        .collect::<Vec<_>>();

    let mut mesh = Vec::with_capacity(grid_size);
    for row in 0..grid_height {
        for column in 0..grid_width {
            let mut horizontal = Vec::new();
            let mut vertical = Vec::new();
            for neighbor_y in row.saturating_sub(1)..=(row + 1).min(grid_height - 1) {
                for neighbor_x in column.saturating_sub(1)..=(column + 1).min(grid_width - 1) {
                    let motion = transferred_residual[neighbor_y * grid_width + neighbor_x];
                    horizontal.push(motion.x);
                    vertical.push(motion.y);
                }
            }
            mesh.push(Vec2::new(median(&mut horizontal), median(&mut vertical)));
        }
    }
    let center_x = grid_width.saturating_sub(1) as f32 * cell_width * 0.5;
    let center_y = grid_height.saturating_sub(1) as f32 * cell_height * 0.5;
    let translation = mesh.iter().copied().sum::<Vec2>() / grid_size as f32;
    let mut denominator = 0.0;
    let mut scale = 0.0;
    let mut rotation = 0.0;
    for (index, motion) in mesh.iter().enumerate() {
        let x = (index % grid_width) as f32 * cell_width - center_x;
        let y = (index / grid_width) as f32 * cell_height - center_y;
        let dx = motion.x - translation.x;
        let dy = motion.y - translation.y;
        denominator += x * x + y * y;
        scale += x * dx + y * dy;
        rotation += x * dy - y * dx;
    }
    let inverse = 1.0 / denominator.max(f32::EPSILON);
    scale *= inverse;
    rotation *= inverse;
    for (index, motion) in mesh.iter_mut().enumerate() {
        let x = (index % grid_width) as f32 * cell_width - center_x;
        let y = (index / grid_width) as f32 * cell_height - center_y;
        motion.x -= translation.x + scale * x - rotation * y;
        motion.y -= translation.y + rotation * x + scale * y;
    }
    (
        Mat3::from_cols(
            Vec3::new(similarity[0], similarity[1], 0.0),
            Vec3::new(-similarity[1], similarity[0], 0.0),
            Vec3::new(similarity[2], similarity[3], 1.0),
        ),
        mesh,
        false,
    )
}

pub fn stabilization_similarity_corrections(
    motion: &[Mat3],
    scene_cuts: &[bool],
    smoothing_radius: usize,
    temporal_falloff: f32,
    mut interrupted: impl FnMut() -> bool,
) -> Option<Vec<Mat3>> {
    if motion.len() != scene_cuts.len() {
        return None;
    }
    let mut relative = motion
        .iter()
        .map(|transform| {
            let (a, b) = (transform.x_axis.x, transform.x_axis.y);
            let scale = a.hypot(b);
            (scale > f32::EPSILON && transform.is_finite()).then_some([
                transform.z_axis.x,
                transform.z_axis.y,
                b.atan2(a),
                scale.ln(),
            ])
        })
        .collect::<Vec<_>>();
    robust_stabilization_motion(&mut relative, scene_cuts, &mut interrupted)?;

    let radius = smoothing_radius.max(1);
    let mut filtered = vec![[0.0; 4]; motion.len()];
    let mut corrections = vec![Mat3::IDENTITY; motion.len()];
    let mut segment_start = 0;
    while segment_start < motion.len() {
        let segment_end = ((segment_start + 1)..motion.len())
            .find(|&frame| scene_cuts[frame])
            .unwrap_or(motion.len());
        for (frame, output) in filtered
            .iter_mut()
            .enumerate()
            .take(segment_end)
            .skip(segment_start + 1)
        {
            if interrupted() {
                return None;
            }
            let start = frame.saturating_sub(radius).max(segment_start + 1);
            let end = (frame + radius + 1).min(segment_end);
            let mut weight_sum = 0.0;
            for (neighbor, sample) in relative.iter().enumerate().take(end).skip(start) {
                let sample = sample.as_ref()?;
                let weight = stabilization_temporal_weight(
                    neighbor.abs_diff(frame) as f32,
                    radius,
                    temporal_falloff,
                );
                weight_sum += weight;
                for (component, value) in output.iter_mut().enumerate() {
                    *value += sample[component] * weight;
                }
            }
            if weight_sum <= f32::EPSILON {
                return None;
            }
            for value in output {
                *value /= weight_sum;
            }
        }
        for frame in (segment_start + 1)..segment_end {
            if interrupted() {
                return None;
            }
            let measured = similarity_pose_transform(relative[frame]?);
            let desired = similarity_pose_transform(filtered[frame]);
            let determinant = measured.determinant();
            if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
                return None;
            }
            corrections[frame] = desired * corrections[frame - 1] * measured.inverse();
        }
        segment_start = segment_end;
    }
    Some(corrections)
}

fn robust_stabilization_motion(
    motion: &mut [Option<[f32; 4]>],
    scene_cuts: &[bool],
    interrupted: &mut impl FnMut() -> bool,
) -> Option<()> {
    const NEIGHBOR_RADIUS: usize = 3;
    const OUTLIER_SCALE: f32 = 4.0;
    const MINIMUM_DEVIATION: [f32; 4] = [1.0, 1.0, 0.002, 0.001];
    const DROPOUT_TRANSLATION: f32 = 0.05;
    const DROPOUT_ROTATION: f32 = 0.0001;
    const DROPOUT_ZOOM: f32 = 0.0001;
    const MOVING_TRANSLATION: f32 = 0.2;
    const MOVING_ROTATION: f32 = 0.001;
    const MOVING_ZOOM: f32 = 0.0005;

    let original = motion.to_vec();
    let mut segment_start = 0;
    while segment_start < motion.len() {
        let segment_end = ((segment_start + 1)..motion.len())
            .find(|&frame| scene_cuts.get(frame).copied().unwrap_or(false))
            .unwrap_or(motion.len());
        for frame in segment_start.saturating_add(1)..segment_end {
            if interrupted() {
                return None;
            }
            let start = frame.saturating_sub(NEIGHBOR_RADIUS).max(segment_start + 1);
            let end = (frame + NEIGHBOR_RADIUS + 1).min(segment_end);
            let neighbors = original[start..end]
                .iter()
                .enumerate()
                .filter_map(|(offset, sample)| {
                    (start + offset != frame).then_some(*sample).flatten()
                })
                .collect::<Vec<_>>();
            if neighbors.is_empty() {
                continue;
            }
            let center = std::array::from_fn(|component| {
                let mut values = neighbors
                    .iter()
                    .map(|sample| sample[component])
                    .collect::<Vec<_>>();
                median(&mut values)
            });
            let Some(mut sample) = original[frame] else {
                motion[frame] = Some(center);
                continue;
            };
            let is_stopped = |sample: [f32; 4]| {
                sample[0].hypot(sample[1]) <= DROPOUT_TRANSLATION
                    && sample[2].abs() <= DROPOUT_ROTATION
                    && sample[3].abs() <= DROPOUT_ZOOM
            };
            let is_moving = |sample: [f32; 4]| {
                sample[0].hypot(sample[1]) >= MOVING_TRANSLATION
                    || sample[2].abs() >= MOVING_ROTATION
                    || sample[3].abs() >= MOVING_ZOOM
            };
            let stopped = is_stopped(sample);
            let adjacent = frame
                .checked_sub(1)
                .and_then(|previous| original[previous])
                .zip(original.get(frame + 1).copied().flatten());
            if let Some((previous, next)) = adjacent
                && stopped
                && is_moving(previous)
                && is_moving(next)
            {
                motion[frame] = Some(std::array::from_fn(|component| {
                    (previous[component] + next[component]) * 0.5
                }));
                continue;
            }
            let neighborhood_is_moving = neighbors
                .iter()
                .filter(|sample| is_moving(**sample))
                .count()
                * 2
                >= neighbors.len();
            if stopped && neighborhood_is_moving {
                motion[frame] = Some(center);
                continue;
            }
            for component in 0..4 {
                let mut deviations = neighbors
                    .iter()
                    .map(|neighbor| (neighbor[component] - center[component]).abs())
                    .collect::<Vec<_>>();
                let threshold =
                    (median(&mut deviations) * OUTLIER_SCALE).max(MINIMUM_DEVIATION[component]);
                if (sample[component] - center[component]).abs() > threshold {
                    sample[component] = center[component];
                }
            }
            motion[frame] = Some(sample);
        }
        segment_start = segment_end;
    }
    Some(())
}

#[derive(Clone, Copy, Debug)]
pub struct StabilizationSmoothing {
    pub temporal_falloff: f32,
    pub translation: f32,
    pub rotation: f32,
    pub zoom: f32,
}

pub fn stabilization_similarity_correction_offsets(
    correction: Mat3,
    smoothing: StabilizationSmoothing,
    grid_size: USizeVec2,
    image_size: UVec2,
) -> Option<Vec<Vec2>> {
    let (a, b) = (correction.x_axis.x, correction.x_axis.y);
    let scale = a.hypot(b);
    if scale <= f32::EPSILON || !correction.is_finite() {
        return None;
    }
    let correction = similarity_pose_transform([
        correction.z_axis.x * smoothing.translation.clamp(0.0, 1.0),
        correction.z_axis.y * smoothing.translation.clamp(0.0, 1.0),
        b.atan2(a) * smoothing.rotation.clamp(0.0, 1.0),
        scale.ln() * smoothing.zoom.clamp(0.0, 1.0),
    ]);
    Some(stabilization_inverse_warp_offsets(
        correction,
        grid_size.x,
        grid_size.y,
        image_size.x,
        image_size.y,
    ))
}

pub fn stabilization_smooth_mesh_paths(
    observed: &[Vec<Vec2>],
    adaptive_weights: &[f32],
    radius: usize,
    iterations: usize,
    mut interrupted: impl FnMut() -> bool,
) -> Option<Vec<Vec<Vec2>>> {
    if observed.is_empty()
        || observed.len() != adaptive_weights.len()
        || observed
            .iter()
            .any(|frame| frame.len() != observed[0].len())
    {
        return None;
    }
    let radius = radius.max(1);
    let mut current = observed.to_vec();
    let mut next = observed.to_vec();
    for _ in 0..iterations.max(1) {
        if interrupted() {
            return None;
        }
        for frame in 0..observed.len() {
            let start = frame.saturating_sub(radius);
            let end = (frame + radius + 1).min(observed.len());
            let lambda = adaptive_weights[frame].max(0.0);
            let mut weight_sum = 0.0;
            for neighbor in start..end {
                if neighbor != frame {
                    let distance = neighbor.abs_diff(frame) as f32;
                    weight_sum += (-((3.0 * distance / radius as f32).powi(2))).exp();
                }
            }
            let denominator = 1.0 + 2.0 * lambda * weight_sum;
            for vertex in 0..observed[0].len() {
                let mut value = observed[frame][vertex];
                for (neighbor, path) in current.iter().enumerate().take(end).skip(start) {
                    if neighbor == frame {
                        continue;
                    }
                    let distance = neighbor.abs_diff(frame) as f32;
                    let weight = (-((3.0 * distance / radius as f32).powi(2))).exp();
                    value += 2.0 * lambda * weight * path[vertex];
                }
                next[frame][vertex] = value / denominator;
            }
        }
        std::mem::swap(&mut current, &mut next);
    }
    Some(current)
}

fn similarity_pose_transform([translation_x, translation_y, angle, log_scale]: [f32; 4]) -> Mat3 {
    let scale = log_scale.exp();
    Mat3::from_scale_angle_translation(
        Vec2::splat(scale),
        angle,
        Vec2::new(translation_x, translation_y),
    )
}

fn stabilization_temporal_weight(distance: f32, radius: usize, falloff: f32) -> f32 {
    const MINIMUM_SIGMA_DIVISOR: f32 = 0.5;
    const SIGMA_DIVISOR_RANGE: f32 = 4.0;

    let falloff = falloff.clamp(0.0, 1.0);
    if falloff <= f32::EPSILON {
        return 1.0;
    }
    let sigma = radius as f32 / (MINIMUM_SIGMA_DIVISOR + SIGMA_DIVISOR_RANGE * falloff);
    (-distance * distance / (2.0 * sigma * sigma)).exp()
}

fn fit_track_similarity(tracks: &[[f32; 6]], indices: &[usize]) -> Option<[f32; 4]> {
    if indices.len() < 2 {
        return None;
    }
    let weight_sum = indices
        .iter()
        .map(|&index| tracks[index][4].max(f32::EPSILON))
        .sum::<f32>();
    let mut source_mean = [0.0, 0.0];
    let mut destination_mean = [0.0, 0.0];
    for &index in indices {
        let track = tracks[index];
        let weight = track[4].max(f32::EPSILON);
        source_mean[0] += track[0] * weight;
        source_mean[1] += track[1] * weight;
        destination_mean[0] += track[2] * weight;
        destination_mean[1] += track[3] * weight;
    }
    source_mean = [source_mean[0] / weight_sum, source_mean[1] / weight_sum];
    destination_mean = [
        destination_mean[0] / weight_sum,
        destination_mean[1] / weight_sum,
    ];
    let mut denominator = 0.0;
    let mut scale_rotation = [0.0, 0.0];
    for &index in indices {
        let track = tracks[index];
        let weight = track[4].max(f32::EPSILON);
        let source = [track[0] - source_mean[0], track[1] - source_mean[1]];
        let destination = [
            track[2] - destination_mean[0],
            track[3] - destination_mean[1],
        ];
        denominator += weight * (source[0] * source[0] + source[1] * source[1]);
        scale_rotation[0] += weight * (source[0] * destination[0] + source[1] * destination[1]);
        scale_rotation[1] += weight * (source[0] * destination[1] - source[1] * destination[0]);
    }
    if denominator <= f32::EPSILON {
        return None;
    }
    let a = scale_rotation[0] / denominator;
    let b = scale_rotation[1] / denominator;
    let translation_x = destination_mean[0] - a * source_mean[0] + b * source_mean[1];
    let translation_y = destination_mean[1] - b * source_mean[0] - a * source_mean[1];
    [a, b, translation_x, translation_y]
        .iter()
        .all(|value| value.is_finite())
        .then_some([a, b, translation_x, translation_y])
}

fn track_similarity_error(track: [f32; 6], similarity: [f32; 4]) -> f32 {
    let motion = similarity_motion(similarity, track[0], track[1]);
    let dx = track[0] + motion.x - track[2];
    let dy = track[1] + motion.y - track[3];
    (dx * dx + dy * dy).sqrt()
}

fn similarity_motion([a, b, translation_x, translation_y]: [f32; 4], x: f32, y: f32) -> Vec2 {
    Vec2::new(
        a * x - b * y + translation_x - x,
        b * x + a * y + translation_y - y,
    )
}

fn median(values: &mut [f32]) -> f32 {
    values.sort_unstable_by(f32::total_cmp);
    values[values.len() / 2]
}

/// Converts a forward OpenCV image transform into the inverse sampling field used by the renderer.
pub fn stabilization_inverse_warp_offsets(
    transform: Mat3,
    grid_width: usize,
    grid_height: usize,
    width: u32,
    height: u32,
) -> Vec<Vec2> {
    let determinant = transform.determinant();
    if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
        return vec![Vec2::ZERO; grid_width.saturating_mul(grid_height)];
    }
    let inverse = transform.inverse();
    (0..grid_height)
        .flat_map(|row| {
            (0..grid_width).map(move |column| {
                let x = column as f32 * width.saturating_sub(1) as f32
                    / grid_width.saturating_sub(1).max(1) as f32;
                let y = row as f32 * height.saturating_sub(1) as f32
                    / grid_height.saturating_sub(1).max(1) as f32;
                let point = Vec2::new(x, y);
                point - project_point(inverse, point).unwrap_or(point)
            })
        })
        .collect()
}

pub fn stabilization_forward_warp_offsets(
    transform: Mat3,
    grid_width: usize,
    grid_height: usize,
    width: u32,
    height: u32,
) -> Vec<Vec2> {
    (0..grid_height)
        .flat_map(|row| {
            (0..grid_width).map(move |column| {
                let x = column as f32 * width.saturating_sub(1) as f32
                    / grid_width.saturating_sub(1).max(1) as f32;
                let y = row as f32 * height.saturating_sub(1) as f32
                    / grid_height.saturating_sub(1).max(1) as f32;
                let point = Vec2::new(x, y);
                project_point(transform, point).unwrap_or(point) - point
            })
        })
        .collect()
}

pub fn stabilization_translate_offsets(
    offsets: &[Vec2],
    translation: Vec2,
    grid_width: usize,
    grid_height: usize,
    width: u32,
    height: u32,
) -> Vec<Vec2> {
    if offsets.len() != grid_width.saturating_mul(grid_height)
        || grid_width == 0
        || grid_height == 0
    {
        return vec![Vec2::ZERO; grid_width.saturating_mul(grid_height)];
    }
    let last_x = grid_width.saturating_sub(1) as f32;
    let last_y = grid_height.saturating_sub(1) as f32;
    (0..grid_height)
        .flat_map(|row| {
            (0..grid_width).map(move |column| {
                let canvas_x = column as f32 * width.saturating_sub(1) as f32
                    / grid_width.saturating_sub(1).max(1) as f32
                    - translation.x;
                let canvas_y = row as f32 * height.saturating_sub(1) as f32
                    / grid_height.saturating_sub(1).max(1) as f32
                    - translation.y;
                let grid_x =
                    (canvas_x / width.saturating_sub(1).max(1) as f32 * last_x).clamp(0.0, last_x);
                let grid_y =
                    (canvas_y / height.saturating_sub(1).max(1) as f32 * last_y).clamp(0.0, last_y);
                let x0 = grid_x.floor() as usize;
                let y0 = grid_y.floor() as usize;
                let x1 = (x0 + 1).min(grid_width - 1);
                let y1 = (y0 + 1).min(grid_height - 1);
                let tx = grid_x - x0 as f32;
                let ty = grid_y - y0 as f32;
                let top_left = offsets[y0 * grid_width + x0];
                let top_right = offsets[y0 * grid_width + x1];
                let bottom_left = offsets[y1 * grid_width + x0];
                let bottom_right = offsets[y1 * grid_width + x1];
                top_left
                    .lerp(top_right, tx)
                    .lerp(bottom_left.lerp(bottom_right, tx), ty)
            })
        })
        .collect()
}

pub fn stabilization_translate_transform(transform: Mat3, translation: Vec2) -> Mat3 {
    Mat3::from_translation(translation) * transform * Mat3::from_translation(-translation)
}

fn project_point(matrix: Mat3, point: Vec2) -> Option<Vec2> {
    let projected = matrix * point.extend(1.0);
    if projected.z.abs() <= f32::EPSILON {
        return None;
    }
    let point = projected.truncate() / projected.z;
    point.is_finite().then_some(point)
}

pub fn stabilization_position(offsets: &[Vec2]) -> Vec<Vec2> {
    if offsets.is_empty() {
        return Vec::new();
    }
    let mean = offsets.iter().copied().sum::<Vec2>() / offsets.len() as f32;
    vec![mean; offsets.len()]
}

pub fn stabilization_similarity(
    offsets: &[Vec2],
    grid_width: usize,
    grid_height: usize,
) -> Vec<Vec2> {
    let translation = stabilization_position(offsets);
    if translation.len() != grid_width.saturating_mul(grid_height) {
        return translation;
    }
    let center_x = grid_width.saturating_sub(1) as f32 * 0.5;
    let center_y = grid_height.saturating_sub(1) as f32 * 0.5;
    let mut denominator = 0.0;
    let mut scale = 0.0;
    let mut rotation = 0.0;
    for (index, offset) in offsets.iter().enumerate() {
        let x = index % grid_width;
        let y = index / grid_width;
        let x = x as f32 - center_x;
        let y = y as f32 - center_y;
        let dx = offset.x - translation[index].x;
        let dy = offset.y - translation[index].y;
        denominator += x * x + y * y;
        scale += x * dx + y * dy;
        rotation += x * dy - y * dx;
    }
    let inverse = 1.0 / denominator.max(f32::EPSILON);
    scale *= inverse;
    rotation *= inverse;
    offsets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let x = index % grid_width;
            let y = index / grid_width;
            let x = x as f32 - center_x;
            let y = y as f32 - center_y;
            translation[index] + Vec2::new(scale * x - rotation * y, rotation * x + scale * y)
        })
        .collect()
}

pub fn stabilization_perspective(
    offsets: &[Vec2],
    grid_width: usize,
    grid_height: usize,
    width: u32,
    height: u32,
) -> Vec<Vec2> {
    if offsets.len() != grid_width.saturating_mul(grid_height) {
        return stabilization_position(offsets);
    }
    let mut normal = [[0.0_f32; 9]; 8];
    for (index, offset) in offsets.iter().enumerate() {
        let x =
            (index % grid_width) as f32 / grid_width.saturating_sub(1).max(1) as f32 * 2.0 - 1.0;
        let y =
            (index / grid_width) as f32 / grid_height.saturating_sub(1).max(1) as f32 * 2.0 - 1.0;
        let destination_x = x + offset.x * 2.0 / width.max(1) as f32;
        let destination_y = y + offset.y * 2.0 / height.max(1) as f32;
        accumulate_projective_row(
            &mut normal,
            [
                x,
                y,
                1.0,
                0.0,
                0.0,
                0.0,
                -x * destination_x,
                -y * destination_x,
            ],
            destination_x,
        );
        accumulate_projective_row(
            &mut normal,
            [
                0.0,
                0.0,
                0.0,
                x,
                y,
                1.0,
                -x * destination_y,
                -y * destination_y,
            ],
            destination_y,
        );
    }
    let Some(homography) = solve_projective(normal) else {
        return stabilization_similarity(offsets, grid_width, grid_height);
    };
    offsets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let x = (index % grid_width) as f32 / grid_width.saturating_sub(1).max(1) as f32 * 2.0
                - 1.0;
            let y = (index / grid_width) as f32 / grid_height.saturating_sub(1).max(1) as f32 * 2.0
                - 1.0;
            let denominator = homography[6] * x + homography[7] * y + 1.0;
            if denominator.abs() <= f32::EPSILON {
                return offsets[index];
            }
            let destination_x =
                (homography[0] * x + homography[1] * y + homography[2]) / denominator;
            let destination_y =
                (homography[3] * x + homography[4] * y + homography[5]) / denominator;
            Vec2::new(
                (destination_x - x) * width.max(1) as f32 * 0.5,
                (destination_y - y) * height.max(1) as f32 * 0.5,
            )
        })
        .collect()
}

fn accumulate_projective_row(normal: &mut [[f32; 9]; 8], row: [f32; 8], value: f32) {
    for y in 0..8 {
        for x in 0..8 {
            normal[y][x] += row[y] * row[x];
        }
        normal[y][8] += row[y] * value;
    }
}

fn solve_projective(mut matrix: [[f32; 9]; 8]) -> Option<[f32; 8]> {
    for column in 0..8 {
        let pivot = (column..8).max_by(|left, right| {
            matrix[*left][column]
                .abs()
                .total_cmp(&matrix[*right][column].abs())
        })?;
        if matrix[pivot][column].abs() <= f32::EPSILON {
            return None;
        }
        matrix.swap(column, pivot);
        let divisor = matrix[column][column];
        for value in &mut matrix[column][column..] {
            *value /= divisor;
        }
        let pivot_row = matrix[column];
        for (row, values) in matrix.iter_mut().enumerate() {
            if row == column {
                continue;
            }
            let factor = values[column];
            for (value, pivot) in values[column..].iter_mut().zip(&pivot_row[column..]) {
                *value -= factor * pivot;
            }
        }
    }
    Some(std::array::from_fn(|index| matrix[index][8]))
}
