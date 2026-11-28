# Smooth scrolling

`interactive::scroll_animation::Scroll` owns each scrollport trajectory. Line
wheel deltas, keys and scrollbar tracks set goals. Precise pixel deltas, dragging
and `Instant` cancel the trajectory and move the handle immediately. Absolute
clock samples on `on_next_frame` keep duration independent of display frequency.
Retargeting preserves the previous curve position and velocity.

CSSOM View leaves easing and duration to the user agent. The policy follows the
studied Blink model: ease-in-out, wheel 100–200 ms according to distance, keyboard
150 ms. Programmatic motion uses sqrt(distance)/60 seconds, bounded to 100–500 ms.
This is a chosen policy, not a measured claim of perceptual Chrome equivalence.
No upstream implementation was copied.

`Document::scroll_to/scroll_by` accept CSS pixels and `ScrollBehavior`. `Auto`
uses computed `scroll-behavior`, `Smooth` requests animation, and `Instant`
cancels it even when the target is unchanged. Future JS bindings use this same
contract; scrollIntoView computes geometry and submits goals to the relevant
scrollport controllers. The root value applies to the viewport; the body value
is not propagated. `App::reduce_motion` disables animation and finishes an active
trajectory when the preference changes.

Before a JS geometry query, `Document::synchronize_scrolling` synchronously
samples every live scrollport at one shared timestamp, without waiting for vsync
or rebuilding the document. `current_root_scroll_offset` includes this barrier;
`scroll_by` computes its relative goal from the current between-frame position.

Before distant motion, the document records the complete curve control hull in
its interest rectangle. Preparation leaves the offset unchanged and runs through
`update_rendering` before draw. The normal margin is restored after recording.
Motion changes only `ScrollHandle`; paint snaps content to whole device pixels
and replays the record. Replay refusals remain visible in `frame_miss.*`, including
`frame_miss.scroll_prepare` for path preparation.

The `--why` journal reports `scroll_animation.frames` and
`scroll_animation.builds`; text output includes animation frames / builds.
Counters record displayed motion frames and actual document builds on those
frames, including replay refusals. Stationary preparation appears in the ordinary
build counter and journal reasons. Deterministic viewport tests assert zero
motion builds and compare replay paint against a fresh tree at the same offset.
