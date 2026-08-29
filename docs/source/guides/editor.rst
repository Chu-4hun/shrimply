Editor
======

The editor combines a preview, an inspector, and a multitrack timeline. The
application menu can show or hide the inspector and timeline when more preview
space is useful.

Preview
-------

The preview provides play and pause, frame stepping, fullscreen playback,
guides, playback-speed controls, and tools for copying or saving the current
frame as PNG. Paint tools operate directly in the preview when a paint item is
selected.

Inspector
---------

The inspector follows the current selection:

* Project controls cover canvas and project-wide properties.
* Caption controls cover text, writing direction, layout, and appearance.
* Visual controls cover compositing, transforms, playback, stabilization, and
  modifiers.
* Audio controls cover output, playback, generators, text-to-speech, and audio
  modifiers where applicable.

The Info tab shows the selected clip's IDs, local and projected timing, source
offset, and dimensions. For file-backed clips it loads a deeper source report
in the background, including file and container details, every audio and video
stream, codec properties, tags, chapters, embedded artwork, and image EXIF
metadata when available. A metadata diagnostic is shown when a source or one of
its optional metadata formats cannot be read.

Timeline
--------

Projects organize content into caption, video, and audio tracks. Tracks can be
enabled or disabled, and edits participate in the project undo and redo
history. The timeline supports splitting, trimming, deleting, ripple cuts,
clipboard operations, snapping, beat grids, and zooming.

The pointer and cut tools change how the timeline responds to a click. Import
collision modes can overwrite existing content, block the import, or create a
new compatible track.

Clip context actions include copy, cut, paste, modifier paste, grouping,
ungrouping, folding sequences, copying or saving a frame, exporting audio,
transcription, silence removal, and text-to-speech where the selected content
supports the operation.
