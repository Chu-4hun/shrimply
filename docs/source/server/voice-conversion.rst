Voice Conversion
================

The :guilabel:`Voice Change` audio modifier changes recorded speech with an
installed Pneuma voice model.

Install voice models
--------------------

For a local server, place ``.safetensors`` or legacy ``.pth`` models in
``server/models``. You can choose a different directory with
``SHRIMPLY_PNEUMA_MODEL_DIR``.

When using Compose, place models in ``server/.docker/pneuma/models``. Restart
the compute server after adding a model. Only installed models appear in
Shrimply; training a voice model is not part of the server.

Change a voice
--------------

#. Select an audio clip containing speech.
#. Add the :guilabel:`Voice Change` audio modifier.
#. Choose one of the models reported by the compute server.
#. Preview the clip and adjust pitch, speed, or the F0 method if needed.

Keep :guilabel:`Maintain pitch while changing speed` enabled when changing
speed should not also shift the voice's pitch.
