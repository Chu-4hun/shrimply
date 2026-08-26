Importing and Creating Media
============================

Import formats
--------------

Shrimply recognizes these source types:

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Type
     - Formats
   * - Video
     - MP4, MOV, MKV, and WebM
   * - Images and documents
     - JPEG, PNG, WebP, AVIF, GIF, SVG, and PDF
   * - Audio
     - AAC, AIFF, ALAC, FLAC, M4A, MP3, Ogg, Opus, and WAV
   * - Captions
     - WebVTT
   * - Layered images
     - PSD and Krita
   * - 3D content
     - OBJ, GLB, and PLY
   * - Application sources
     - Blender ``.blend`` files and Python Manim scenes

Project opening additionally accepts native Shrimply projects, Shrimply JSON,
OpenTimelineIO, and Kdenlive projects.

Add generated content
---------------------

The add menu on a video track can create text, shapes, paint, backgrounds, 3D
scenes, and video-generation items. Available shapes include rectangles,
ellipses, triangles, stars, arrows, diamonds, polygons, hearts, and crosses.

The add menu on an audio track can create text-to-speech and audio-generator
items. Features backed by AI models require the :doc:`../server/index`.

Record content
--------------

Shrimply can record audio and the screen into the timeline. Choose the
appropriate recording action from the track controls, then stop the recording
when the desired range is complete.
