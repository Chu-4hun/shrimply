# Long-form Manim test scenes

These scenes exercise long-duration streaming, caching, seeking, and playback.
Their drawings are deliberately small and immutable so elapsed duration is the
only meaningful stress case. They do not use external assets.

| Scene | Approximate duration | Drawing cost |
| --- | ---: | --- |
| `FourierCheckpointMarathon` | 275 seconds | 7 static vector primitives |
| `ChaoticVectorFieldMarathon` | 250 seconds | 512 static points in one cloud |
| `MatrixLatticeMarathon` | 250 seconds | 84 static vector cells |

Drag `long_form_scenes.py` onto the timeline. Shrimply selects the first scene
alphabetically by default; enter another class name in the Manim inspector and
press **Reload** to switch scenes.

Every segment animates only the camera. Geometry is never rebuilt, copied, or
grown as scene time advances.
