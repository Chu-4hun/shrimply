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

Connect Shrimply
----------------

The local server listens at ``http://127.0.0.1:8787`` by default. In Shrimply,
open :menuselection:`Preferences --> External`. Select the local server under
:guilabel:`Inference Servers`, then choose one of the compute devices reported
by that server.

The :guilabel:`Available` row shows the features supplied by the selected
server. If a feature is missing, it will not appear in the corresponding
editor controls.

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

The Compose configuration enables GPU access and preserves downloaded models
between runs.

.. code-block:: console

   $ cd server
   $ docker compose up --build

Compute features
----------------

See :doc:`services` for an overview, or open a feature directly:

* :doc:`transcription`
* :doc:`text-to-speech`
* :doc:`video-segmentation`
* :doc:`voice-conversion`
* :doc:`camera-tracking`
* :doc:`video-generation`

Troubleshooting
---------------

Keep the server process running while Shrimply uses a compute feature. Check
the selected server in :menuselection:`Preferences --> External` if a model is
missing or a connection fails. A first request can take longer while its model
downloads; later requests reuse the downloaded files.

.. toctree::
   :maxdepth: 1
   :hidden:

   services
   transcription
   text-to-speech
   video-segmentation
   voice-conversion
   camera-tracking
   video-generation
