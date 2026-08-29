MCP Integration
===============

Shrimply includes an MCP stdio adapter named ``shrimply-mcp``. Each MCP client
runs its own adapter, while every open editor exposes a project-specific Unix
socket. Tools and resources operate on the editor's live in-memory project,
including unsaved changes.

Configure the adapter
---------------------

A development build places the adapter at ``target/debug/shrimply-mcp``.
Configure an MCP client with its absolute path:

.. code-block:: toml

   [mcp_servers.shrimply]
   command = "/absolute/path/to/shrimply/target/debug/shrimply-mcp"

An installed release can use ``shrimply-mcp`` as the command. After
``make dev``, Codex users can register the development adapter with:

.. code-block:: console

   $ make install-codex-mcp-dev

Create or connect to a project
------------------------------

Call ``create_project`` with an absolute path ending in ``.shrimp`` to create
a blank project, open it in a new Shrimply editor, and connect the MCP session
to it. The destination must not already exist. The optional name, canvas width,
canvas height, and exact fractional frame rate default to Untitled Project,
1920, 1080, and 30/1. Frame rates must match one of Shrimply's supported rates.

For an existing project, open it in Shrimply and call ``connect_project`` with
its absolute path. Calling either connection tool switches the MCP session to
that project after the editor bridge is ready.

Connection fails clearly when the project is closed, its lock is stale, or the
selected editor has a different project open. Creation never overwrites an
existing file, and a valid new file remains available if its editor cannot be
launched or connected.

Read live state
---------------

``get_editor_state``
   Return the live project, playhead, selection, active scope, and tracks.

``list_scopes``
   List the root scope and folded-sequence presentation scopes.

``query_clips``, ``get_clip``, and ``get_clip_info``
   Query clip presentations, retrieve a concrete clip, or request the deeper
   source report shown by the inspector. ``get_clip_info`` includes filesystem,
   container, stream, codec, tag, chapter, artwork inventory, and image EXIF
   metadata when available. Set ``include_artwork=true`` to also return embedded
   artwork as MCP image content.

``get_manim_clip``
   Return a Manim clip's discovered scenes, reflected parameter definitions and
   values, and current render error.

``view_frame``
   Render a zero-based project frame to PNG without moving the playhead.

``seek_playhead``
   Move the visible playhead, clamped to the project duration.

``get_clip_info`` accepts the same exact address or unique item ID selectors as
``get_clip`` and does not expose arbitrary filesystem probing. Source failures
are returned in ``source_error`` alongside the clip's normal live metadata, so
clients can still inspect clips whose linked files are missing or unsupported.
The existing ``get_clip`` response remains unchanged for clients that only need
project metadata.

Edit a project
--------------

MCP time values are zero-based integer frames. Use ``create_track`` and
``insert_files`` to add content; ``move_clip``, ``trim_clip``, and
``delete_clips`` to edit it; and ``set_clip_properties`` or
``set_track_enabled`` to change validated properties.

``run_edit_script`` validates an ordered group of typed operations against a
clone, then installs them atomically as one undoable history action.

Python files imported with ``insert_files`` become Manim video clips. Use
``set_manim_clip`` to select a discovered scene or set typed reflected parameter
overrides; a null parameter value resets that control to its scene default.
Scene and parameter changes are validated and committed as undoable edits. Use
``reload_manim_source`` after changing the Python file to invalidate cached
scene state and rebuild it.

``insert_tts`` creates an empty text-to-speech audio item that can be configured
and generated from the inspector. Its start defaults to the playhead and its
duration defaults to the editor's visual-item duration. The same operation is
available in ``run_edit_script``.

``list_tts_models`` returns the models and dynamic input definitions advertised
by the editor's current compute server. ``generate_tts`` applies those model
defaults, accepts typed input overrides, generates speech, and inserts the
result as one undoable edit. Number inputs use exact numerator/denominator
fractions, and audio inputs use a local file path.

``list_stt_models`` returns the speech-to-text models advertised by the current
compute server. ``transcribe_audio`` accepts either one fully addressed audio
track or one or more fully addressed audio clips, transcribes their audible
timeline ranges, and inserts the timed result on a new caption track as one
undoable edit. An optional CLDR locale identifies the caption language.

Both TTS insertion tools accept an optional audio track and scope. Without a
track they reuse one with room. Set ``collision="new_track"`` to allow a new
audio track as a fallback; ``collision="overwrite"`` requires an explicit
track.

Imports copy files into project media by default. Set ``link=true`` to retain
external paths. An import without a target uses an existing compatible track
with room; it creates a track only when ``collision="new_track"`` is explicit.

Resources
---------

The adapter exposes ``shrimply://editor/state``,
``shrimply://project/clips``, ``shrimply://project/clips/{item_id}``, and
``shrimply://edit-api``.
