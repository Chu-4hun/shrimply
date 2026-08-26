Compute Server
==============

Shrimply's optional local compute server provides model-backed transcription,
text-to-speech, video segmentation, voice conversion, 3D camera tracking, and
video generation. The server advertises its available devices and exact model
capabilities to the editor.

Run locally
-----------

The server is a separate uv project requiring Python 3.14. From the repository
root, run its locked environment with:

.. code-block:: console

   $ make dev-server

From the ``server`` directory, the equivalent command is:

.. code-block:: console

   $ uv run --locked src/main.py

Models download into their configured caches on first use. Device and memory
requirements vary substantially by model; consult the model catalog before
starting a large download.

Share access
------------

Set ``SHRIMPLY_SERVER_SHARE=1`` to create a temporary public
``gradio.live`` URL while keeping the MessagePack API available:

.. code-block:: console

   $ SHRIMPLY_SERVER_SHARE=1 uv run --locked src/main.py

The public URL can invoke every compute endpoint. Share it only with trusted
users, and stop the process to remove access.

Containers
----------

The Compose configuration exposes the host network, enables GPU access, and
persists uv, Hugging Face, model, and Python caches under ``server/.docker``.

.. code-block:: console

   $ cd server
   $ docker compose up --build

Reference
---------

See :doc:`reference` for the MessagePack protocol, scheduler behavior, model
catalog, video-generation constraints, cache controls, and validation commands.

.. toctree::
   :maxdepth: 1
   :hidden:

   reference
