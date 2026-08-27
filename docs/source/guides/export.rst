Export
======

Export types
------------

The export window can produce rendered video, WebVTT captions, or JSON project
data.

Video and GIF
-------------

Current video choices are H.264, H.265, and GIF. H.264 and H.265 require NVENC;
no software encoder fallback is available. Available containers are MP4,
Matroska, and GIF.

Audio encoder choices include AAC, FDK AAC, and Opus. The compatible choices
depend on the selected container.

Before exporting, confirm the project canvas, frame rate, range, container,
video codec, and audio codec. The native Shrimply compositor renders exports
without recording the preview surface.
