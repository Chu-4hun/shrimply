Lip sync
========

Shrimply uses the Rhubarb Lip Sync C++ API directly. The
``shrimply-lip-sync`` crate builds the native library, and the audio
crate analyzes a 16 kHz mono mix of the selected project audio tracks.

Expression API
--------------

``mouth()`` returns the mouth shape for the master audio mix at the
current project time. ``mouth(1)`` returns the shape for audio track 1,
and multiple zero-based track indices can be selected with calls such as
``mouth(0, 2)``.

The result is one of Rhubarb's shape strings:

- ``A``: closed mouth, as in M, B, and P
- ``B``: clenched teeth, most consonants, and “ee”
- ``C``: moderately open vowels
- ``D``: wide-open vowels
- ``E``: rounded mouth, as in “off”
- ``F``: puckered mouth, as in “you”, “boy”, and “way”
- ``G``: F and V
- ``H``: L
- ``X``: idle or resting mouth

Expressions can branch on the result:

.. code-block:: text

   switch mouth() {
       "A" => 0,
       "B" => 1,
       "C" => 2,
       "D" => 3,
       "E" => 4,
       "F" => 5,
       "G" => 6,
       "H" => 7,
       "X" => 8,
   }

Analysis and caching
--------------------

Rhubarb analyzes each unique audio mix and visual-item timeline range
once. The cache key contains the audio-track content, selected track
indices, and the range of the item evaluating ``mouth()``. Only that
range is rendered to the 16 kHz analysis WAV. Each rendered frame then
looks up the cue containing its exact fractional project time. Preview
analysis runs asynchronously; export analysis blocks until the result is
ready. Moving or resizing an item requests its new range; obsolete
pending ranges for that item and track selection are skipped.

``X`` is a valid Rhubarb cue, not a loading or error value. Loading and
analysis failures must remain separate from mouth shapes so expressions
cannot mistake unavailable analysis for an intentional rest.

Frame events must preserve their sampled audio analysis even when the
composited visual frame is empty. In particular, a clear-frame event
must not replace its analysis with a default silent mixer: preview
overlays and inspector feedback may evaluate expressions independently,
and doing so manufactured a periodic ``real shape, X, real shape, X``
sequence. Direct inspection of Rhubarb's raw cue timeline confirmed that
Rhubarb did not generate those alternating ``X`` values.
