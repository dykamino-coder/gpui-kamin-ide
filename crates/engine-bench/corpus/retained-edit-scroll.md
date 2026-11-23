# Deferred inline paint acceptance

The adjacent synthetic HTML combines a justified inline flow in an ordered list, a separate
composition group, an unrelated `attr(title)` dependency and a hidden subtree.
Changing the root `alt` class paints one inline element below the viewport.

On Windows, run the functional check with the normal offscreen launcher and
software renderer. Set `ENGINE_BENCH_EDIT_SCROLL_CHECK=800` and pass:

```text
retained-edit-scroll.html --mode edit --edit class --viewport 400x300
--gpu software --runs 1 --fresh-check-dir <artifacts> --json
```

The opt-in checker uses a root scroller and the normal scene-layer policy.
The document unit tests additionally force the no-layer case. After the measured series it opens
the original document again, edits it, and scrolls to the requested offset.
The edit frame, first scroll frame and settled frame must each match an
independent fresh document exactly in RGB. A negative control requires the
original and edited documents to differ at the final offset. A mismatch,
missing scroller or invisible edit fails the process. These extra frames
are excluded from the measured samples; use the default mode for pricing.
