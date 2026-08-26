Compute Features
================

Shrimply uses the compute server for features that depend on large machine
learning models. The editor only shows models and features advertised by the
selected server, so the available choices may differ between servers.

Transcription
-------------

Transcription converts selected audio into timed captions. Choose the
transcription action from the selected audio clip's timeline menu, then select
one of the models offered by the server.

The current server can provide Parakeet, Qwen3 ASR, Whisper, and
Distil-Whisper models. Model files download the first time they are used and
remain cached for later sessions.

Text-to-speech
--------------

Text-to-speech creates audio from text or generates speech for captions. Add a
text-to-speech item to an audio track, or use the speech action for selected
caption content.

Available voices and controls depend on the selected model. The current server
supports Qwen3 TTS for built-in voices, voice cloning, and voice design, plus
IndexTTS 2 and 2.5.

Video segmentation
------------------

The SAM 2 modifier follows a selected subject through a video. Use it when an
effect or composite should apply to a moving subject without manually drawing
a mask on every frame.

Shrimply offers the SAM 2.1 tiny, small, base-plus, and large variants when
they are available from the server. Larger variants generally trade more
resource use for model capacity.

Voice conversion
----------------

Voice conversion changes recorded speech using an installed Pneuma model.
Place ``.safetensors`` or legacy ``.pth`` models in
``server/.docker/pneuma/models`` when using Compose. For a local server, place
them in ``server/models`` or set ``SHRIMPLY_PNEUMA_MODEL_DIR``.

Only installed voice models appear in Shrimply. Training a voice model is not
part of the server.

3D camera tracking
------------------

Camera tracking analyzes video motion to recover a 3D camera path. The server
can provide COLMAP and, when available, VGGT-SLAM. Choose from the methods
shown in the camera-source inspector rather than assuming a particular method
is available.

Video generation
----------------

Video generation creates new timeline media from text, images, or references.
It can involve especially large downloads and long processing times. See
:doc:`video-generation` before starting a generation.
