Getting Started
===============

Run Shrimply
------------

This repository does not currently publish an end-user installation method.
From a development checkout, install the Fedora dependencies and prepare CUDA
Oxide once:

.. code-block:: console

   $ make deps-fedora
   $ make oxide-setup

Then build and launch Shrimply through the Makefile:

.. code-block:: console

   $ make dev

The current CUDA artifacts target ``sm_86``. See :doc:`development` for the
rest of the development workflow.

Create a project
----------------

Start Shrimply without a project path to open the launcher. Select
:guilabel:`Create Project`, then choose a name, canvas width, canvas height,
and frame rate. New projects begin with a caption track, a video track, and an
audio track. They use the ``.shrimp`` extension.

The launcher also shows recent projects and can filter them by name or path.

Open a project
--------------

Select :guilabel:`Open Project` to open a ``.shrimp``, ``.json``, ``.otio``,
or ``.kdenlive`` project.

Build a timeline
----------------

Open the application menu and choose :menuselection:`New Track` to add a
caption, video, or audio track. Drop or paste media onto a compatible track,
then select a clip to edit it in the inspector.

Useful timeline shortcuts include:

* :kbd:`Space` — play or pause
* :kbd:`S` — split every clip at the playhead
* :kbd:`Shift+S` — split and select the clips on the left
* :kbd:`Q` — ripple-trim the selected clip to the playhead
* :kbd:`D` — delete the selection
* :kbd:`Shift+D` — ripple cut
* :kbd:`Ctrl+X` — cut
* :kbd:`Z` — toggle timeline zoom
* :kbd:`Ctrl+Z` — undo
* :kbd:`Ctrl+Shift+Z` — redo

Save and export
---------------

Use :menuselection:`Save` or :menuselection:`Save As` to write the project.
Select :guilabel:`Export` to render a video or GIF, export WebVTT captions, or
write project data. See :doc:`guides/export` for formats and encoder details.
