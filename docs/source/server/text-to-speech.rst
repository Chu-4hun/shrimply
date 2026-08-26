Text to Speech
==============

Text-to-speech can create an editable audio item from new text or generate
timed speech for existing captions.

Create a speech item
--------------------

#. Open the add menu on an audio track.
#. Choose :guilabel:`Text to Speech`.
#. Select the new item and enter its text in the inspector.
#. Choose a model and configure the controls it provides.
#. Generate the speech.

The available voices, reference inputs, emotion controls, and other settings
depend on the selected model.

Generate speech for captions
----------------------------

Select caption clips and choose :guilabel:`Generate Speech` from the timeline
context menu. You can also use the same action on a caption track to process
all of its non-empty captions.

Choose a model that supports caption timing. Shrimply generates each caption
at its existing duration and places the resulting audio on a track where it
does not overlap existing clips.

Models and licenses
-------------------

The current server supports Qwen3 TTS for built-in voices, voice cloning, and
voice design, plus IndexTTS 2 and 2.5.

IndexTTS is provided under the separate :ref:`bilibili Model Use License
Agreement <indextts-license>`. Review that agreement before downloading or
using the model; the Shrimply and compute-server licenses do not replace its
terms.
