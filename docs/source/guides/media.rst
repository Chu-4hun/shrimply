Importing and Creating Media
============================

.. |add| image:: ../../../assets/icons/plus-symbolic.svg
   :class: action-icon
   :alt: Add
.. |import| image:: ../../../assets/icons/document-open-symbolic.svg
   :class: action-icon
   :alt: Import
.. |text| image:: ../../../assets/icons/draw-text-symbolic.svg
   :class: action-icon
   :alt: Text
.. |shape| image:: ../../../assets/icons/shapes-large-symbolic.svg
   :class: action-icon
   :alt: Shape
.. |paint| image:: ../../../assets/icons/applications-graphics-symbolic.svg
   :class: action-icon
   :alt: Paint
.. |background| image:: ../../../assets/icons/preferences-desktop-wallpaper-symbolic.svg
   :class: action-icon
   :alt: Background
.. |scene-3d| image:: ../../../assets/icons/3d-object-symbolic.svg
   :class: action-icon
   :alt: 3D Scene
.. |video-generation| image:: ../../../assets/icons/video-generation-symbolic.svg
   :class: action-icon
   :alt: Video Generation
.. |text-to-speech| image:: ../../../assets/icons/font-x-generic-symbolic.svg
   :class: action-icon
   :alt: Text to Speech
.. |audio-generator| image:: ../../../assets/icons/sound-symbolic.svg
   :class: action-icon
   :alt: Audio Generator
.. |screen-recording| image:: ../../../assets/icons/screencast-recorded-symbolic.svg
   :class: action-icon
   :alt: Screen recording
.. |microphone| image:: ../../../assets/icons/mic-1-symbolic.svg
   :class: action-icon
   :alt: Microphone recording

Import media
------------

Move the playhead to the desired start time, then select the destination track.
Click |add| **Add** in that track's controls and choose |import| **Import
Media…** or **Import Captions…**. The file is inserted at the playhead on the
track whose menu you opened. If that track is part of a multi-track selection,
the import targets all selected tracks of the same type.

Video and audio files can only be imported to video or audio tracks. WebVTT
files can only be imported to caption tracks. MKV and WebM files dropped onto
the timeline can be losslessly remuxed to MP4 first; they cannot be imported
directly from a track's add menu.

Supported formats
~~~~~~~~~~~~~~~~~

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

Create media
------------

Move the playhead to an empty point on the destination track and click |add|
**Add**. The new item starts at the playhead, uses the default visual duration,
and ends early rather than overlapping the next item.
Nothing is created if the playhead is already inside an item on that track.

On a video track, the menu provides:

.. list-table::
   :widths: 8 25 67
   :header-rows: 1

   * - Icon
     - Action
     - Creates
   * - |text|
     - **Text**
     - A text layer using the default font.
   * - |shape|
     - **Shape**
     - A vector shape. Rectangle, ellipse, triangle, star, arrow, diamond,
       polygon, heart, and cross shapes are available in the inspector.
   * - |paint|
     - **Paint**
     - A layer for drawing strokes directly on the canvas.
   * - |background|
     - **Background**
     - A full-canvas background layer.
   * - |scene-3d|
     - **3D Scene**
     - A 3D scene that can contain shapes, text, models, lights, and a ground
       plane.
   * - |video-generation|
     - **Video Generation**
     - An AI video-generation item. This requires the
       :doc:`Shrimply server <../server/index>`.

On an audio track, the menu provides:

.. list-table::
   :widths: 8 25 67
   :header-rows: 1

   * - Icon
     - Action
     - Creates
   * - |text-to-speech|
     - **Text to Speech**
     - An item that generates speech from text using the Shrimply server.
   * - |audio-generator|
     - **Audio Generator**
     - A procedural audio item configured in the inspector.

Text to Speech and Video Generation are backed by AI models and require the
:doc:`Shrimply server <../server/index>`.

Record content
--------------

Recording actions are separate buttons in each track's controls:

* On a video track, click |screen-recording| **Record Screen or Application**,
  choose a screen or application in the system capture dialog, and click the
  same button again to stop. The recording starts at the playhead and stops
  before the next item on the track.
* On an audio track, click |microphone| **Record Microphone** to start recording
  from the default microphone. Click the same button again to stop.

The active recording button turns red. Recording advances playback and places
the finished item on that track at the original playhead position. Start from
an empty point so the recording does not overlap an existing item.
