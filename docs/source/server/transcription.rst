Transcription
=============

Transcription turns selected audio into timed captions on a new caption track.

Create captions
---------------

#. Select one or more audio clips, or select an audio track.
#. Open the timeline context menu and choose :guilabel:`Transcribe`.
#. Choose one of the speech-to-text models offered by the compute server.
#. Select :guilabel:`Transcribe` and wait for the operation to finish.

The current server can offer Parakeet, Qwen3 ASR, Whisper, and Distil-Whisper.
If none appear, check the selected server in
:menuselection:`Preferences --> External`.

Follow edit points
------------------

Keep :guilabel:`Follow cuts` enabled when caption boundaries should follow
nearby audio or video edits. :guilabel:`Snap source` chooses which cuts to
follow, while :guilabel:`Snap tolerance` controls how close a generated
boundary must be before it moves to a cut.

The defaults are suitable for most projects. Disable :guilabel:`Follow cuts`
when you want the transcription model's continuous timing without edit-based
chunking.

If no speech is detected, Shrimply leaves the project unchanged.
