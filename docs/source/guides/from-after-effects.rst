From After Effects
==================

After Effects expressions use JavaScript. After Effects supports a modern
JavaScript engine and an `older Legacy ExtendScript engine
<https://helpx.adobe.com/after-effects/desktop/work-with-expressions/expression-basics/legacy-and-extend-script-engine.html>`__.
Shrimply expressions use `Rhai <https://rhai.rs/book/language/>`__ instead. The
concepts are similar, but an After Effects expression must be rewritten rather
than pasted directly into Shrimply.

Both systems provide ``value`` for the property's value before the expression,
``time`` for time in seconds, and arrays such as ``[x, y]`` for multi-component
properties. Shrimply also provides ``t`` and ``local_t`` as aliases for
``time``. In Shrimply, the last value in the script is the result. See
:doc:`expression-basics` for every available value and function.

Continuous motion
-----------------

A simple time calculation is the same in both systems. This rotates by 40
degrees per second:

.. tabs::

   .. code-tab:: javascript After Effects

      time * 40

   .. code-tab:: rust Shrimply

      time * 40 // or t * 40

Variable declarations
---------------------

Adobe's `expression-language guide
<https://helpx.adobe.com/after-effects/desktop/work-with-expressions/expression-basics/expression-language.html>`__
shows variables assigned without a declaration keyword. After Effects also
supports ``var`` in both expression engines and ``let`` and ``const`` in the
modern engine. Translate each form as follows:

.. list-table::
   :header-rows: 1
   :widths: 28 24 28 20

   * - After Effects
     - Expression engines
     - Shrimply
     - Meaning
   * - ``speed = 40``
     - Modern and Legacy
     - ``let speed = 40``
     - First definition
   * - ``var speed = 40``
     - Modern and Legacy
     - ``let speed = 40``
     - Mutable variable
   * - ``let speed = 40``
     - Modern only
     - ``let speed = 40``
     - Mutable variable
   * - ``const speed = 40``
     - Modern only
     - ``const speed = 40``
     - Constant

After a Rhai variable has been declared, assign a new value without repeating
``let``:

.. tabs::

   .. code-tab:: javascript After Effects

      speed = 40;
      speed = 20;
      time * speed

   .. code-tab:: rust Shrimply

      let speed = 40;
      speed = 20;
      t * speed

Wiggle and shake
----------------

After Effects' ``wiggle(frequency, amount)`` varies around the existing value.
Shrimply's ``shake(phase)`` produces smooth noise, so multiply ``time`` by the
frequency, multiply the noise by the amount, and add it to the original value:

.. tabs::

   .. code-tab:: javascript After Effects

      wiggle(5, 20)

   .. code-tab:: rust Shrimply

      value + shake(t * 5) * 20

For a 2D property, build the result from ``x`` and ``y``. Give each ``shake``
call a different seed so the axes move independently:

.. code-block:: rust

   [
     x + shake(t * 5, 0) * 20,
     y + shake(t * 5, 1) * 20
   ]

Oscillation
-----------

Shrimply provides math functions directly instead of through JavaScript's
``Math`` object:

.. tabs::

   .. code-tab:: javascript After Effects

      value + Math.sin(time * 4) * 20

   .. code-tab:: rust Shrimply

      value + sin(t * 4) * 20

Remapping values
----------------

Use ``lerp`` with a clamped progress value in place of After Effects'
``linear`` function. This moves from 0 to 100 over two seconds:

.. tabs::

   .. code-tab:: javascript After Effects

      linear(time, 0, 2, 0, 100)

   .. code-tab:: rust Shrimply

      lerp(0, 100, clamp(t / 2, 0, 1))

Conditions
----------

Rhai uses ``if`` blocks instead of JavaScript's ``? :`` conditional operator:

.. tabs::

   .. code-tab:: javascript After Effects

      time < 1 ? 0 : 100

   .. code-tab:: rust Shrimply

      if t < 1 { 0 } else { 100 }

Adobe-specific features
-----------------------

Shrimply does not provide After Effects objects such as ``thisComp``,
``thisLayer``, or property links. It also does not expose keyframes to
expressions, so ``loopIn()``, ``loopOut()``, and ``valueAtTime()`` have no
direct equivalent. To loop an item's playback, open
:menuselection:`Playback --> Repeat` in the inspector and set
:guilabel:`Strategy` to :guilabel:`Repeat` or :guilabel:`Ping Pong`. Rewrite
other expressions using only the values and functions in
:doc:`expression-basics`.
