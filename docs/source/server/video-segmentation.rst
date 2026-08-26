Video Segmentation
==================

The :guilabel:`Segment Anything 2` modifier follows a subject through a video
and turns it into a mask. This is useful when an effect or composite should
apply to a moving subject without drawing the mask on every frame.

Create a mask
-------------

#. Select a visual clip and add the :guilabel:`Segment Anything 2` modifier.
#. Move the playhead to a frame where the subject is clear.
#. Click the subject in the preview to add a foreground point. Right-click or
   Control-click to mark background that should be excluded.
#. Add more points as needed, or drag across the preview to draw a box around
   the subject.
#. Select :guilabel:`Analyze` and keep the compute server running until the
   mask is ready.

Adjust the result
-----------------

Use :guilabel:`Threshold` to change the mask boundary and
:guilabel:`Edge softness` to feather it. :guilabel:`Invert` swaps the selected
and unselected regions.

Changing a prompt makes the existing analysis out of date. Select
:guilabel:`Reanalyze` after moving points, changing the box, or selecting a
different SAM 2 model.

Shrimply offers SAM 2.1 tiny, small, base-plus, and large variants when they
are available from the server. Larger variants generally trade more resource
use for model capacity.
