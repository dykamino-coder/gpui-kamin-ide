//! Frame accounting and input validation after a measured sample.
use super::*;

impl Frames {
    pub(crate) fn observe(
        &mut self,
        cx: &AsyncApp,
        window: WindowHandle<View>,
        frame: Frame,
        mode: &str,
        index: usize,
    ) -> Result<()> {
        let (after, inside, contents, blocks, offset) = window.read_with(cx, |view, _| {
            let doc = &view.doc;
            (
                doc.retained_frames(),
                doc.builds_inside_draw(),
                doc.replayed_contents(),
                doc.replayed_blocks(),
                (doc.root_scroll_offset(), doc.root_scroll_max()),
            )
        })?;
        let misses = window.read_with(cx, |view, _| view.doc.take_frame_misses())?;
        let cold = mode == "cold";
        let counts = delta(if cold { (0, 0) } else { self.before }, after)?;
        let inside = inside
            .checked_sub(if cold { 0 } else { self.before_inside })
            .ok_or("document inside-draw build counter regressed")?;
        // A frame built again within the same frame counts both builds.
        if self.relayout && (counts.0 == 0 || counts.1 != 0) {
            return Err(format!(
                "relayout requires a document build and no replay; received {counts:?}"
            )
            .into());
        }
        if self.scroll {
            let back = self.back_down.is_some();
            scrolled(self.wheel_from.take(), offset.0, self.step, offset.1, back)?;
            // An end of the document turns the next step of `--scroll-back`.
            if let (Some(down), Some((_, y)), Some((_, max))) = (self.back_down, offset.0, offset.1)
            {
                self.back_down = Some(if down { y < max - 0.5 } else { y <= 0.5 });
            }
        }
        let group = usize::from(self.relayout && !index.is_multiple_of(2));
        let memory = self.scroll.then(crate::host::memory_now).flatten();
        if let (true, Some((ws, private))) = (memory_only(), memory) {
            // A long memory run killed at its time limit still leaves its curve.
            let y = offset.0.map_or(f32::NAN, |o| o.1);
            eprintln!("scroll-memory step {index} y {y} ws {ws} private {private}");
        }
        if !keeps_step(self.scroll, index) {
            self.dispatched = None;
            return Ok(());
        }
        self.samples.push(json!({"mode": mode, "sample_index": index,
            "working_set_bytes": memory.map(|m| m.0), "private_bytes": memory.map(|m| m.1),
            "css_viewport": viewport(self.base, group == 1), "builds": counts.0, "replays": counts.1,
            "wheel_step_px": if self.scroll { json!(self.step) } else { Value::Null },
            "root_scroll_offset": offset.0.map(|(x, y)| json!([x, y])),
            "dispatch_to_frame_ms": self.dispatched.take().map(|t| t.elapsed().as_secs_f64() * 1000.0),
            "builds_inside_draw": inside,
            "content_replays": contents.saturating_sub(if cold { 0 } else { self.before_contents }),
            "block_replays": blocks.saturating_sub(if cold { 0 } else { self.before_blocks }),
            "animation": crate::animation::journal(window.read_with(cx, |view, _| view.doc.animation_frame())?),
            "frame_misses": misses.into_iter().map(|(k, n)| (k, json!(n))).collect::<serde_json::Map<_, _>>()}));
        // A memory run keeps no step: its working set is the document's.
        if let (true, Some(offset), false) = (self.scroll, offset.0, memory_only()) {
            self.steps.push(&frame.pixels, offset);
        }
        self.pixels[group].observe(frame.pixels, frame.seq, mode, index)
    }
}
