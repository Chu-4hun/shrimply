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

Audio tracks can contain generated speech. The compute server determines which
text-to-speech and voice-conversion models appear in Shrimply.

Lip sync expressions
--------------------

Expressions can call ``mouth()`` to match mouth shapes to the project audio or
selected audio tracks. See :doc:`lip-sync` for mouth shapes, selection syntax,
and analysis behavior.

Audio cleanup
-------------

Timeline actions can remove silence or export the audio for selected content.
Use audio modifiers for nondestructive cleanup and sound design within the
project.
