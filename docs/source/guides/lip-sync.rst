Lip sync
========

Shrimply uses the `Rhubarb Lip Sync
<https://github.com/DanielSWolf/rhubarb-lip-sync>`__ C++ API directly.
The ``shrimply-lip-sync`` crate builds the native library, and the audio crate
analyzes a 16 kHz mono mix of the selected project audio tracks.

Using mouth shapes in expressions
---------------------------------

``mouth()`` returns the mouth shape for the master audio mix at the
current project time. ``mouth(1)`` returns the shape for audio track 1,
and multiple zero-based track indices can be selected with calls such as
``mouth(0, 2)``.

See :doc:`expression-basics` for the rest of the expression API, including
property values, time, math, color, and audio-level functions.

The result is one of Rhubarb's shape strings. These examples are copied from
the `mouth-shape table in Rhubarb's README
<https://github.com/DanielSWolf/rhubarb-lip-sync#mouth-shapes>`__:

.. list-table::
   :widths: 10 30 60
   :header-rows: 1

   * - Shape
     - Example
     - Use
   * - ``A``
     - .. image:: ../img/rhubarb/lisa-A.png
          :alt: Rhubarb mouth shape A, with closed lips
          :width: 140px
     - Closed mouth for M, B, and P sounds.
   * - ``B``
     - .. image:: ../img/rhubarb/lisa-B.png
          :alt: Rhubarb mouth shape B, with clenched teeth
          :width: 140px
     - Slightly open mouth with clenched teeth, used for most consonants and
       sounds such as “ee.”
   * - ``C``
     - .. image:: ../img/rhubarb/lisa-C.png
          :alt: Rhubarb mouth shape C, moderately open
          :width: 140px
     - Open mouth for vowels such as “eh” and “ae.”
   * - ``D``
     - .. image:: ../img/rhubarb/lisa-D.png
          :alt: Rhubarb mouth shape D, wide open
          :width: 140px
     - Wide-open mouth for vowels such as “aa.”
   * - ``E``
     - .. image:: ../img/rhubarb/lisa-E.png
          :alt: Rhubarb mouth shape E, slightly rounded
          :width: 140px
     - Slightly rounded mouth for sounds such as “off” and “bird.”
   * - ``F``
     - .. image:: ../img/rhubarb/lisa-F.png
          :alt: Rhubarb mouth shape F, with puckered lips
          :width: 140px
     - Puckered lips for sounds such as “you,” “show,” and “way.”
   * - ``G``
     - .. image:: ../img/rhubarb/lisa-G.png
          :alt: Rhubarb mouth shape G, with upper teeth touching the lower lip
          :width: 140px
     - Upper teeth touching the lower lip for F and V sounds.
   * - ``H``
     - .. image:: ../img/rhubarb/lisa-H.png
          :alt: Rhubarb mouth shape H, with the tongue behind the upper teeth
          :width: 140px
     - Raised tongue behind the upper teeth for long L sounds.
   * - ``X``
     - .. image:: ../img/rhubarb/lisa-X.png
          :alt: Rhubarb mouth shape X, with relaxed closed lips
          :width: 140px
     - Relaxed idle position used for pauses in speech.

``A`` through ``F`` are the basic shapes. ``G``, ``H``, and ``X`` are extended
shapes that can improve animation when the character artwork supports them.

Expressions can branch on the result:

.. code-block:: rust

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

Analysis
--------

Shrimply analyzes each unique audio selection once and reuses the result.
Preview analysis happens in the background, so ``mouth()`` can briefly report
that analysis is still loading. Export waits for analysis to finish. ``X`` is
a real rest shape, not a loading or error value.

The mouth-shape images are from the Rhubarb Lip Sync README and are used under
its MIT License.
