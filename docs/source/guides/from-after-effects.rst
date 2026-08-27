From After Effects
==================

After Effects expressions are JavaScript with Adobe-specific values and
functions. Shrimply expressions use `Rhai
<https://rhai.rs/book/language/>`__ instead. The concepts are similar, but an
After Effects expression must be rewritten rather than pasted directly into
Shrimply.

Both systems provide ``value`` for the property's value before the expression,
``time`` for time in seconds, and arrays such as ``[x, y]`` for multi-component
properties. In Shrimply, the last value in the script is the result. See
:doc:`expression-basics` for every available value and function.

Continuous motion
-----------------

A simple time calculation is the same in both systems. This rotates by 40
degrees per second:

.. tabs::

   .. code-tab:: javascript After Effects

      time * 40

   .. code-tab:: rust Shrimply

      time * 40

Wiggle and shake
----------------

After Effects' ``wiggle(frequency, amount)`` varies around the existing value.
Shrimply's ``shake(phase)`` produces smooth noise, so multiply ``time`` by the
frequency, multiply the noise by the amount, and add it to the original value:

.. tabs::

   .. code-tab:: javascript After Effects

      wiggle(5, 20)

   .. code-tab:: rust Shrimply

      value + shake(time * 5) * 20

For a 2D property, build the result from ``x`` and ``y``. Give each ``shake``
call a different seed so the axes move independently:

.. code-block:: rust

   [
     x + shake(time * 5, 0) * 20,
     y + shake(time * 5, 1) * 20
   ]

Oscillation
-----------

Shrimply provides math functions directly instead of through JavaScript's
``Math`` object:

.. tabs::

   .. code-tab:: javascript After Effects

      value + Math.sin(time * 4) * 20

   .. code-tab:: rust Shrimply

      value + sin(time * 4) * 20

Remapping values
----------------

Use ``lerp`` with a clamped progress value in place of After Effects'
``linear`` function. This moves from 0 to 100 over two seconds:

.. tabs::

   .. code-tab:: javascript After Effects

      linear(time, 0, 2, 0, 100)

   .. code-tab:: rust Shrimply

      lerp(0, 100, clamp(time / 2, 0, 1))

Conditions
----------

Rhai uses ``if`` blocks instead of JavaScript's ``? :`` conditional operator:

.. tabs::

   .. code-tab:: javascript After Effects

      time < 1 ? 0 : 100

   .. code-tab:: rust Shrimply

      if time < 1 { 0 } else { 100 }

Adobe-specific features
-----------------------

Shrimply does not provide After Effects objects such as ``thisComp``,
``thisLayer``, or property links. It also does not expose keyframes to
expressions, so functions such as ``loopIn()``, ``loopOut()``, and
``valueAtTime()`` have no direct equivalent. Use Shrimply's keyframe graphs for
keyframe loops and rewrite expressions using only the values and functions in
:doc:`expression-basics`.
