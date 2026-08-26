Audio and Captions
==================

Captions
--------

Caption tracks store timed text independently from visual and audio tracks.
The inspector controls text, writing direction, layout, and appearance.
Shrimply can import and export WebVTT captions.

Transcription
-------------

Transcription converts selected audio into timed text. The available model
catalog comes from the connected compute server, so start the server before
using transcription. See :doc:`../server/index`.

Text-to-speech and voice conversion
-----------------------------------

Audio tracks can contain generated speech. The compute server advertises the
available text-to-speech and voice-conversion models and keeps compatible
workers loaded for reuse.

Lip sync expressions
--------------------

Expressions can call ``mouth()`` to obtain Rhubarb mouth cues for the project
audio mix or selected audio tracks. See :doc:`lip-sync` for cue values,
selection syntax, and cache behavior.

Audio cleanup
-------------

Timeline actions can remove silence or export the audio for selected content.
Use audio modifiers for nondestructive cleanup and sound design within the
project.
