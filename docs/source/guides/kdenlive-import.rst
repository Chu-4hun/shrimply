Kdenlive Import Support
=======================

Shrimply reads Kdenlive's saved MLT XML and converts the active timeline to a
new Shrimply project. This is a one-way conversion, not a Kdenlive playback
engine. Only the features listed as **Supported** or **Approximate** below have
dedicated conversion logic. Other Kdenlive features are not converted, even if
Shrimply has a similar feature.

The importer prefers original media paths over proxy paths. It does not copy
media into the Shrimply project, so the source files must remain available at
their resolved paths.

Status meanings
---------------

.. list-table::
   :widths: 20 80
   :header-rows: 1

   * - Status
     - Meaning
   * - Supported
     - Converted to the corresponding Shrimply data without a known loss in
       the handled fields.
   * - Approximate
     - Converted, but some Kdenlive behavior or data is simplified or lost.
   * - Not imported
     - No dedicated conversion exists. The content is omitted or left as a
       generic media reference that Shrimply may not be able to play.
   * - Import stops
     - The whole import fails instead of producing an incomplete project.

Project and timeline
--------------------

.. list-table::
   :widths: 30 16 54
   :header-rows: 1

   * - Kdenlive feature
     - Status
     - Imported result or limitation
   * - Frame rate and canvas size
     - Supported
     - Read from the MLT profile.
   * - Pixel/display aspect, scan mode, color metadata, profile name, and
       audio settings
     - Not imported
     - Shrimply uses its defaults beyond frame rate and canvas size.
   * - Project name
     - Approximate
     - Taken from the ``.kdenlive`` filename, not Kdenlive project metadata.
   * - Video and audio track order
     - Supported
     - Black and timeline-preview tracks are discarded.
   * - Track enabled state
     - Supported
     - Kdenlive's audio/video/both hide state becomes the Shrimply track state.
   * - Track names, locks, height, collapse, targeting, and track-level effects
     - Not imported
     - Shrimply track defaults are used.
   * - Gaps, clip positions, in/out trims, and source durations
     - Supported
     - MLT ``blank`` and ``entry`` timing is converted at the project frame
       rate.
   * - Both internal lanes of one Kdenlive track
     - Approximate
     - Non-empty lanes become adjacent Shrimply tracks. Their mix/overlap
       relationship is not recreated.
   * - Nested sequences
     - Approximate
     - Reachable sequence tractors become folded sequences with video and
       audio tracks. Unused bin sequences and nested-sequence captions are not
       imported.
   * - Mixes, crossfades, wipes, tractor transitions, and track compositions
     - Not imported
     - The importer does not read MLT ``transition`` elements.
   * - Clip groups, guides, markers, zones, notes, bin folders, thumbnails, and
       unused bin clips
     - Not imported
     - These properties are not read.
   * - Preview guides and editor UI state
     - Not imported
     - Shrimply defaults are used.

Media and generators
--------------------

.. list-table::
   :widths: 30 16 54
   :header-rows: 1

   * - Kdenlive source
     - Status
     - Imported result or limitation
   * - File-backed video and audio
     - Supported
     - Imported as a media reference with the selected video/audio stream.
       Whether it can be decoded still depends on Shrimply's media support.
   * - JPEG, PNG, WebP, BMP, TIFF, GIF, SVG, and PDF
     - Supported
     - Classified as the corresponding Shrimply visual source. PDF size comes
       from its first page.
   * - Krita and PSD layered images
     - Supported
     - Layer paths are imported and nearest-neighbor sampling is selected.
   * - Forward speed changes and pitch preservation
     - Supported
     - Positive Kdenlive timewarp speed and ``warp_pitch`` are converted.
   * - Reverse or negative speed
     - Import stops
     - Negative speed cannot be represented by the importer's unsigned
       fraction parser.
   * - Source dimensions, stream selection, and media autorotation
     - Supported
     - Imported when the corresponding producer metadata is present.
   * - Proxies
     - Approximate
     - The importer prefers ``kdenlive:originalurl`` and does not retain the
       proxy relationship.
   * - Solid color clips (``color`` or ``colour``)
     - Supported
     - ``0xRRGGBBAA`` is converted to a Shrimply solid-color background,
       including alpha.
   * - Color Bars generator (``frei0r.test_pat_B``)
     - Approximate
     - A linked generator ``.mlt`` file is inspected and converted to
       Shrimply's static Test Pattern. Kdenlive's eight bar types do not have
       exact Shrimply equivalents, so the selected type is not preserved.
   * - White Noise generator (``noise``)
     - Approximate
     - A linked generator ``.mlt`` file is converted to Shrimply white-noise
       video and audio generators. The random algorithm, video range/cadence,
       stereo correlation, and trimmed-generator start are not sample-exact.
   * - Counter generator (``count``), including its optional beep
     - Not imported
     - It remains a generic MLT media reference. Shrimply has no equivalent
       counter or gated one-frame beep.
   * - Other MLT generators and playlists
     - Not imported
     - They remain generic media references without native conversion.
   * - Kdenlive title clips and title templates
     - Not imported
     - ``kdenlivetitle`` content is not parsed into Shrimply text or shapes.
   * - Image sequences and slideshows
     - Not imported
     - Their producer-specific sequence behavior is not converted.
   * - Missing-media placeholders
     - Not imported
     - ``_placeholder`` and ``_missingsource`` are not replaced with invented
       visuals; the importer still follows the saved media path.

Color Bars and White Noise created through Kdenlive's generator dialog are
normally stored in separate ``.mlt`` files. Those files must be readable while
the project is imported. After a recognized generator is converted, the
Shrimply item no longer depends on that generator file.

Video effects
-------------

Only enabled filters stored on a timeline clip entry are considered. Producer,
bin, track, and sequence effects are not imported.

.. list-table::
   :widths: 30 16 54
   :header-rows: 1

   * - Kdenlive effect
     - Status
     - Imported result or limitation
   * - Transform (``qtblend``)
     - Approximate
     - Position, scale, non-uniform distortion, rotation, anchor, opacity, and
       their handled keyframes are converted. Stacked transforms are matrix
       composed; resulting shear and intermediate clipping can be approximate.
   * - Normal and Screen clip blend modes
     - Supported
     - Other Kdenlive blend modes become Normal.
   * - Crop (``qtcrop``)
     - Approximate
     - Animated rectangular crop is converted. Rounded, circular, or colored
       padding is reduced to a rectangle; crop after rotation/shear can become
       source-aligned.
   * - Gaussian Blur (``avfilter.gblur``)
     - Approximate
     - RGB-only and alpha-only selections are preserved. Other partial plane
       selections are applied to all RGBA channels.
   * - Chroma Key (``chroma``)
     - Approximate
     - Key color and animated variance/similarity are converted. Other
       Shrimply settings use defaults, and an animated key color is not kept as
       a color animation.
   * - Saturation (``frei0r.saturat0r``)
     - Supported
     - Converted to animated Shrimply color correction.
   * - Hue Shift (``frei0r.hueshift0r``)
     - Supported
     - Converted to animated Shrimply color correction.
   * - Lift/Gamma/Gain (``lift_gamma_gain``)
     - Approximate
     - Red-channel values become Shrimply brightness, gamma, and value.
       Per-channel differences are lost.
   * - Fade from/to black
     - Supported
     - Converted to Shrimply clip intro/outro transitions.
   * - Selective Color Correction (``avfilter.colorcorrect``)
     - Not imported
     - Explicitly skipped.
   * - Every other video effect
     - Not imported
     - The importer uses the allowlist above and skips other identified entry
       filters.

Effect masks
------------

Kdenlive Alpha Shapes masks are **Approximate**. Rectangle and ellipse shapes
with the handled alpha operations can mask Crop, Gaussian Blur, Chroma Key,
Saturation, Hue Shift, and Lift/Gamma/Gain. A group containing several effects
is represented by applying the same Shrimply mask to each modifier.

Other mask types, unsupported shapes or alpha operations, nested or malformed
mask groups, and groups containing any other effect are not imported.

Audio effects
-------------

.. list-table::
   :widths: 30 16 54
   :header-rows: 1

   * - Kdenlive audio feature
     - Status
     - Imported result or limitation
   * - Clip fade in/out
     - Supported
     - Converted to Shrimply audio intro/outro transitions.
   * - Constant Gain (``gain``)
     - Supported
     - Linear gain is converted to decibels.
   * - Animated Volume (``volume``)
     - Supported
     - Kdenlive level keyframes are converted to a Shrimply Gain modifier.
   * - Every other audio effect
     - Not imported
     - Other identified entry filters are skipped.
   * - Track-level audio filters and mixing
     - Not imported
     - Only filters attached directly to clip entries are considered.

Captions
--------

.. list-table::
   :widths: 30 16 54
   :header-rows: 1

   * - Kdenlive caption feature
     - Status
     - Imported result or limitation
   * - Active-sequence ASS subtitle files
     - Approximate
     - Dialogue timing, text, explicit line breaks, and hard spaces are
       imported into caption tracks.
   * - Subtitle enabled/hidden state
     - Supported
     - A global hidden or disabled ASS/subtitles filter disables imported
       caption tracks.
   * - ASS fonts, outlines, positioning, and other styling
     - Approximate
     - Replaced with one centered-bottom Shrimply caption style.
   * - Nested-sequence captions and other subtitle representations
     - Not imported
     - Only the active sequence's subtitle-file list is read.

When import stops
-----------------

The importer deliberately fails instead of saving a partly parsed project when
required structure or handled data is invalid. This includes malformed XML;
missing or invalid profile values, active-sequence UUIDs, tracks, producers, or
timing; malformed values in a supported effect; and unreadable or malformed
PDF, layered-image, subtitle, or recognized external generator files.

An ordinary missing media file is not opened during conversion, so it may be
reported only when Shrimply later tries to use that media.
