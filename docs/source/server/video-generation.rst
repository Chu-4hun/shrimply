Video Generation
================

Create a generated video
------------------------

Add a video-generation item from the add menu on a video track. Shrimply loads
the models and controls offered by the selected compute server. Choose a model,
provide the requested text and media inputs, then start generation.

Generation continues through the compute server, so keep the server running
until the result has returned to the editor. Cancel the operation from
Shrimply if you no longer need it.

Available models
----------------

The current server supports these model families:

``MiniMax H3 Base``
   Text-to-video, first/last-frame-to-video, and ordered reference-to-video.

``Looping Sketch Anime``
   Text-to-video and first/last-frame-to-video using the pinned animation
   adapter.

``Wan 2.1 T2V 1.3B``
   Text-to-video in landscape or portrait orientation.

``Wan 2.2 TI2V 5B``
   Text-to-video and first-frame image-to-video in landscape or portrait
   orientation.

The editor uses the server's live model catalog. If a model is not listed, it
is not currently available from the selected server.

Before downloading
------------------

Video-generation models can require substantial download space, memory, and
processing time. Requirements vary by model and by the selected compute
device. Check the model publisher's documentation before downloading weights,
especially on a shared or storage-constrained machine.

MiniMax H3 weights are governed by the separate :ref:`MiniMax H3 Community
License Agreement <minimax-h3-license>`. Review and accept it before using the
model.

Wan model details and licenses are available from the official `Wan 2.1 1.3B
<https://huggingface.co/Wan-AI/Wan2.1-T2V-1.3B-Diffusers>`__ and `Wan 2.2 5B
<https://huggingface.co/Wan-AI/Wan2.2-TI2V-5B-Diffusers>`__ pages.

Memory controls
---------------

Some models offer ``Normal`` and ``Low VRAM`` modes. ``Low VRAM`` reduces
accelerator-memory pressure by moving more work through system memory and is
usually slower. Quantization can reduce model memory further, with a possible
quality or performance tradeoff.

Start with the default settings. Use the reduced-memory options only when the
default cannot run on the selected server.
