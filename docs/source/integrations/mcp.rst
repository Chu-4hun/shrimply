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

Connect to a project
--------------------

Open the project in Shrimply, then call ``connect_project`` with the absolute
project path before using any other project tool or resource. Calling it again
switches the MCP session to another open project.

Connection fails clearly when the project is closed, its lock is stale, or the
selected editor has a different project open.

Read live state
---------------

``get_editor_state``
   Return the live project, playhead, selection, active scope, and tracks.

``list_scopes``
   List the root scope and folded-sequence presentation scopes.

``query_clips`` and ``get_clip``
   Query clip presentations or retrieve a concrete clip.

``view_frame``
   Render a zero-based project frame to PNG without moving the playhead.

``seek_playhead``
   Move the visible playhead, clamped to the project duration.

Edit a project
--------------

MCP time values are zero-based integer frames. Use ``create_track`` and
``insert_files`` to add content; ``move_clip``, ``trim_clip``, and
``delete_clips`` to edit it; and ``set_clip_properties`` or
``set_track_enabled`` to change validated properties.

``run_edit_script`` validates an ordered group of typed operations against a
clone, then installs them atomically as one undoable history action.

Imports copy files into project media by default. Set ``link=true`` to retain
external paths. An import without a target uses an existing compatible track
with room; it creates a track only when ``collision="new_track"`` is explicit.

Resources
---------

The adapter exposes ``shrimply://editor/state``,
``shrimply://project/clips``, ``shrimply://project/clips/{item_id}``, and
``shrimply://edit-api``.
