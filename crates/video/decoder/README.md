# Video decoder

This crate owns FFmpeg demuxing and seeking, the NVDEC session, cancellation, decoder pooling, and
the retained CUDA frame. Rendering submits decode requests and consumes their results.

Random interactive seeks use FFmpeg's normal keyframe seek and return the first usable decoded
frame as a best-effort preview. Accurate requests continue decoding to the requested timestamp.
Continuous requests are also time-accurate, but keep forward decode work alive regardless of the
source-time distance between requests so accelerated playback does not become repeated seeking.
The decoder does not receive content accuracy because every request retains the full-resolution
source frame. The pool owns real-time scheduling; owner or request-mode changes supersede active
work, while local scrubbing retains its bounded-distance behavior. Owners include the consumer,
sequence path, timeline track, item, and color or alpha plane, so simultaneous uses of the same
source remain independent. Exact work uses the blocking queue.
