3D Camera Tracking
==================

Camera tracking analyzes a visual track and creates a camera path for a 3D
scene. Use it when 3D content should follow the movement of recorded footage.

Track a camera
--------------

#. Place the source footage on a visual track.
#. Select the 3D item that should use the tracked camera.
#. In the camera controls, set :guilabel:`Camera source` to the source visual
   track instead of :guilabel:`Custom`.
#. Choose a tracking method and analysis frame rate.
#. Select :guilabel:`Analyze` and keep the compute server running until the
   camera path is ready.

Available methods
-----------------

``COLMAP``
   Offers quality and camera-model controls in addition to the analysis frame
   rate.

``VGGT-SLAM``
   Appears only when it is available on the selected compute device.

Using a lower analysis frame rate reduces the number of frames processed but
can miss fast camera movement. If the source or settings change, select
:guilabel:`Analyze Again` to replace the cached path.
