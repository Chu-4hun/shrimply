Server Reference
================

The Shrimply server is licensed under the GNU Affero General Public
License, version 3 or later. See :doc:`../licenses`.

Start the server
----------------

Run these commands from the ``server`` directory:

.. code-block:: console

   $ uv run --locked src/main.py
   $ SHRIMPLY_SERVER_SHARE=1 uv run --locked src/main.py
   $ docker compose up --build
   $ SHRIMPLY_SERVER_GIT_HASH=$(git rev-parse HEAD) docker compose up --build

The server uses Gradio Server mode with custom FastAPI routes. Set
``SHRIMPLY_SERVER_SHARE=1`` to create a temporary public ``gradio.live``
URL while keeping the MessagePack API unchanged. The public URL can
invoke every compute endpoint, so share it only with trusted users. It
remains available only while the server process is running.

Scheduling and job lifecycle
----------------------------

Reusable model workers remain loaded independently for 600 seconds after
their last use. Set ``SHRIMPLY_MODEL_IDLE_TTL_SECONDS`` to change the
shared timeout. The scheduler admits multiple jobs when CPU slots, RAM,
and VRAM permit. It reserves model weights separately from active
inference workspace before spawning, reuses idle
exact-model/configuration workers, and can start duplicate copies when
one is busy. Idle nonmatching workers are evicted largest-first when
needed. An unexpected CUDA out-of-memory failure discards only that
worker and retries its job once after evicting eligible idle workers.

``GET /`` returns protocol ``4.0`` status as MessagePack, including
queued and active job counts, reserved RAM and VRAM, grouped exact
worker keys, and the server version and git hash. Models are advertised
as service-prefixed capabilities such as
``stt:nvidia/parakeet-tdt-0.6b-v3``. The server supports Parakeet TDT
0.6B v3, Qwen3 ASR 0.6B with its forced aligner, Whisper Large v3 Turbo,
Whisper Small, and Distil-Whisper Large v3.

Every compute ``POST`` requires a UUID in the ``Shrimply-Job-ID``
header. Streams begin with a one-based ``queued`` event and resend their
latest queue or progress event every five seconds. Clients renew their
lease with ``PUT /compute/jobs/{job_id}/heartbeat`` every five seconds;
a job expires after 30 seconds without a heartbeat.
``DELETE /compute/jobs/{job_id}`` cancels any queued, loading, running,
decoding, or transferring job and is idempotent.

3D tracking
-----------

``POST /3dtracking/analyses`` accepts a Shrimply 3D-tracking frame
archive and returns a length-prefixed MessagePack event stream
containing camera poses. ``3dtracking:colmap/colmap`` supports CPU or
CUDA; the ``3dtracking:MIT-SPARK/VGGT-SLAM`` capability is advertised
when a CUDA device is selected. VGGT uses BF16 on Ampere or newer GPUs
and FP16 on older CUDA GPUs. Model and SALAD loop-closure weights
download into Torch's cache on first use. The worker is reused for
requests using the same tracking method and follows the shared model
idle timeout.

Transcription
-------------

``POST /transcriptions`` accepts raw little-endian ``f32`` mono audio at
16 kHz and returns a length-prefixed MessagePack event stream with
progress updates and the timestamped result. The ``model`` query
parameter is required and must contain one of the advertised model IDs.
Models are downloaded from Hugging Face on first use and cached between
container runs. Exact-model workers run in reusable spawned Python
processes. Multiple copies or different services may run concurrently
within the scheduler's capacity.

Compute device
--------------

``PUT /compute/device?device=<device>`` selects ``cpu`` or an advertised
CUDA device such as ``cuda:0`` for subsequent compute workers. Changing
devices stops all loaded idle workers; the server rejects the change
while any managed job exists.

Voice conversion
----------------

Pneuma voice conversion is part of the regular server. Place
``.safetensors`` or legacy ``.pth`` voice models under
``server/.docker/pneuma/models`` when using Compose, or set
``SHRIMPLY_PNEUMA_MODEL_DIR`` for a local server. The worker uses the
selected compute device and follows the shared model idle timeout.

``GET /pneuma/models`` lists installed voice models and their basic
checkpoint metadata. ``POST /pneuma/conversions`` accepts a MessagePack
conversion request and returns a length-prefixed MessagePack event
stream ending in WAV audio. Only voice-conversion inference is exposed;
training and JIT compilation are not part of the server API.

Video generation
----------------

``GET /video-generation/models`` returns the server-driven input
catalog. ``POST /video-generations`` accepts a MessagePack request and
streams progress, bounded MP4 chunks, and exact rational result
metadata. H3 capabilities are advertised only while a CUDA device is
selected. Each request runs in a fresh spawned process; cancellation or
disconnect terminates it, and CUDA OOM is retried once after idle
compute workers are preempted.

The catalog includes MiniMax H3 Base (``t2va``, ``fl2va``, and ordered
multimodal ``ref2va``) and the pinned Looping Sketch Anime adapter
(``t2va`` and ``fl2va``). The Sketch entry downloads revision
``9c88fbc800ea87d745137f1b637c08aa1a5e3bd6``, converts its Musubi fused
Q/K/V weights to PEFT, validates all 50 H3 blocks, and caches the
converted adapter.

It also includes the Apache-2.0 Wan checkpoints through Diffusers. Wan
2.1 T2V 1.3B generates silent 832×480 or 480×832 H.264 video with 81
frames at 16 fps. Wan 2.2 TI2V 5B supports text-to-video and first-frame
image-to-video at 1280×704 or 704×1280, producing 121 silent frames at
24 fps. Both paths use a float32 VAE with tiling, BF16 model weights,
and model CPU offload. Wan requires a BF16-capable CUDA device; reserve
about 10 GiB of VRAM for the 1.3B model and 24 GiB for TI2V 5B. Model
details and licenses are available from the official `Wan 2.1
1.3B <https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B-Diffusers>`__ and
`Wan 2.2 5B <https://huggingface.co/Wan-AI/Wan2.2-TI2V-5B-Diffusers>`__
pages.

Review and accept the `MiniMax H3 Community
License <https://huggingface.co/MiniMaxAI/MiniMax-H3/blob/main/LICENSE>`__
before downloading or using the weights. Plan for approximately 220 GiB
of cache storage and A100-class GPU memory. The validated hybrid path
uses an 80 GiB A100, keeps the denoiser on CUDA, streams conditioner
groups from disk, and separates resumable generation from decode/mux.
Conventional BF16 component offload can require roughly 140 GiB of
system RAM.

The catalog exposes two memory modes. ``Normal`` selects the validated
A100 path, keeping the large denoiser and decoder on the GPU while
staging the conditioner through disk. ``Low VRAM`` streams BF16 model
blocks through host memory; it uses less accelerator memory but can
require roughly 140 GiB of system RAM and is substantially slower. The
separate ``Quantization`` control can select persisted TorchAO INT8
weight-only transformer and text-encoder weights. INT8 uses its own
group-offloaded execution path, so the BF16 memory-mode control does not
apply.

Generation and decoding run in different spawned processes so their
allocators and model mappings cannot overlap. H3 models are advertised
when CUDA is selected. On an 80 GiB A100, automatic decode follows the
validated prototype and loads the 9.1 GiB decoder incrementally onto the
GPU instead of repeatedly reading it from disk. The server releases
staged upload bytes before starting a worker. A completed atomic latent
checkpoint remains resumable after worker or client interruption.

Cache and memory controls:

- ``SHRIMPLY_VIDEO_GENERATION_CACHE`` stores request media, conditioning
  state, latent checkpoints, and completed outputs.
- ``MINIMAX_H3_CACHE`` relocates downloaded H3 components.
- ``MINIMAX_H3_DISK_OFFLOAD_CACHE`` relocates the revision-specific
  hybrid cache.
- ``MINIMAX_H3_QUANTIZED_CACHE`` relocates persistent INT8 components.
- ``MINIMAX_H3_LORA_CACHE`` relocates converted PEFT adapters.
- ``MINIMAX_H3_DECODE_OFFLOAD=auto|gpu|disk`` controls the separate
  decode stage. Automatic mode keeps the decoder on GPUs with at least
  32 GiB free.

Automatic attention tries Flash Attention 3 only on supported Hopper
hardware and falls back to standard attention. Selecting Flash Attention
3 explicitly makes an unavailable kernel a request error. Five requested
seconds align to H3's decodable ``17n+5`` grid: 124 frames, or
approximately 5.17 seconds at 24 fps, with H.264 video and 32 kHz stereo
AAC audio.

Validation
----------

The fast port checks run without model inference:

.. code-block:: console

   $ PYTHONPATH=src uv run --locked python -m unittest tests.test_video_generation -v

For a real API render, start the server and run
``scripts/validate_video_generation.py`` with one of ``base-t2va``,
``sketch-fl2va``, or ``base-ref2va``. The validator consumes the
streamed MP4, fully decodes both streams, and checks the codec,
dimensions, frame count, frame rate, audio layout/rate, non-silence, and
synchronized rational duration.
