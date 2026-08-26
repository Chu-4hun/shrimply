from fractions import Fraction

import torch
from torch import Tensor


def allocate_duration_frames(
    duration: Fraction,
    intersegment_silence: Fraction,
    segment_weights: list[int],
    sample_rate: int = 22_050,
    hop_length: int = 256,
) -> list[int]:
    if not segment_weights or any(weight < 1 for weight in segment_weights):
        raise ValueError("Duration allocation requires non-empty text segments")
    silence = intersegment_silence * (len(segment_weights) - 1)
    speech_duration = duration - silence
    if speech_duration <= 0:
        raise ValueError("Duration is too short for the intersegment silence")
    total_frames = round(speech_duration * sample_rate / hop_length)
    if total_frames < len(segment_weights):
        raise ValueError("Duration cannot allocate one frame to every text segment")
    weight_sum = sum(segment_weights)
    result: list[int] = []
    previous = 0
    cumulative = 0
    for weight in segment_weights:
        cumulative += weight
        boundary = round(Fraction(total_frames * cumulative, weight_sum))
        result.append(boundary - previous)
        previous = boundary
    if min(result) < 1:
        raise ValueError("Duration is too short for the relative segment sizes")
    return result


def frames_for_speed(natural_frames: int, speed_factor: Fraction) -> int:
    if natural_frames < 1 or speed_factor <= 0:
        raise ValueError("Natural frames and speed factor must be positive")
    return max(1, round(Fraction(natural_frames, 1) / speed_factor))


def effective_speed_factor(natural_samples: int, final_samples: int) -> Fraction:
    if natural_samples < 1 or final_samples < 1:
        raise ValueError("Audio lengths must be positive")
    return Fraction(natural_samples, final_samples)


def low_energy_token_mask(
    waveform: Tensor,
    token_count: int,
    sample_rate: int = 22_050,
    minimum_silence: Fraction = Fraction(2, 25),
    relative_threshold_db: float = 35.0,
    absolute_threshold_db: float = -45.0,
) -> Tensor:
    if token_count < 1:
        raise ValueError("Token count must be positive")
    samples = waveform.detach().float().reshape(-1)
    if samples.numel() == 0:
        return torch.zeros(token_count, dtype=torch.bool, device=waveform.device)
    boundaries = torch.linspace(
        0, samples.numel(), token_count + 1, device=samples.device
    ).round().long()
    root_mean_square = torch.empty(token_count, device=samples.device)
    for index in range(token_count):
        start = int(boundaries[index])
        end = max(start + 1, int(boundaries[index + 1]))
        root_mean_square[index] = samples[start:end].square().mean().sqrt()
    decibels = 20 * torch.log10(root_mean_square.clamp_min(1e-7))
    threshold = max(
        float(decibels.max()) - relative_threshold_db,
        absolute_threshold_db,
    )
    candidates = decibels < threshold
    minimum_run = max(
        1,
        round(
            minimum_silence
            * sample_rate
            * token_count
            / samples.numel()
        ),
    )
    result = torch.zeros_like(candidates)
    start = 0
    while start < token_count:
        if not bool(candidates[start]):
            start += 1
            continue
        end = start + 1
        while end < token_count and bool(candidates[end]):
            end += 1
        if end - start >= minimum_run:
            result[start:end] = True
        start = end
    return result


def allocate_silence_aware_durations(
    natural_frames: int,
    target_frames: int,
    silence_mask: Tensor,
    minimum_frames_per_silence_run: float = 2.0,
    silence_expansion_weight: float = 4.0,
) -> Tensor:
    token_count = silence_mask.numel()
    if token_count < 1 or target_frames < 1:
        raise ValueError("Duration allocation requires positive token and frame counts")
    durations = torch.full(
        (token_count,),
        natural_frames / token_count,
        dtype=torch.float32,
        device=silence_mask.device,
    )
    frame_delta = float(target_frames - natural_frames)
    if frame_delta == 0 or not bool(silence_mask.any()):
        return durations * (target_frames / durations.sum())
    if frame_delta > 0:
        weights = torch.ones_like(durations)
        weights[silence_mask] = silence_expansion_weight
        durations += frame_delta * weights / weights.sum()
        return durations * (target_frames / durations.sum())
    frames_to_remove = -frame_delta
    runs: list[tuple[int, int, float]] = []
    start = 0
    while start < token_count:
        if not bool(silence_mask[start]):
            start += 1
            continue
        end = start + 1
        while end < token_count and bool(silence_mask[end]):
            end += 1
        run_frames = float(durations[start:end].sum())
        runs.append(
            (start, end, max(0.0, run_frames - minimum_frames_per_silence_run))
        )
        start = end
    capacity = sum(run[2] for run in runs)
    silence_removal = min(frames_to_remove, capacity)
    if capacity > 0:
        for start, end, removable in runs:
            run_removal = silence_removal * removable / capacity
            total = durations[start:end].sum()
            durations[start:end] *= (total - run_removal) / total
    remaining = frames_to_remove - silence_removal
    if remaining > 0:
        durations *= (durations.sum() - remaining) / durations.sum()
    return durations * (target_frames / durations.sum())


def warp_embeddings(
    embeddings: Tensor,
    token_durations: Tensor,
    target_frames: int,
) -> Tensor:
    if embeddings.ndim != 3 or embeddings.size(0) != 1:
        raise ValueError("Embedding warping requires shape [1, time, channels]")
    if embeddings.size(1) != token_durations.numel():
        raise ValueError("Token durations must match the embedding sequence")
    cumulative_ends = token_durations.cumsum(0)
    positions = torch.arange(
        target_frames,
        device=embeddings.device,
        dtype=token_durations.dtype,
    ) + 0.5
    source_indices = torch.searchsorted(cumulative_ends, positions).clamp_max(
        embeddings.size(1) - 1
    )
    return embeddings.index_select(1, source_indices)
