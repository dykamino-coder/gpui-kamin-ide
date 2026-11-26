//! Stationary viewport paint and scroll-sensitive paint dependencies.

use super::*;

impl Window {
    /// Record viewport-stationary content under `clip`. Unless `anchored`,
    /// the clip moves with the enclosing scroll node while the content stays.
    /// Pair this with `with_scroll_node_paint` using the same stable `id`.
    pub fn with_viewport_content_prepaint<R>(
        &mut self,
        id: ScrollNodeId,
        clip: Bounds<Pixels>,
        anchored: bool,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let source = scroll_nodes::NodeSource::Viewport { anchored };
        let open = self.open_scroll_node(id, clip, Point::default(), source);
        let result = f(self);
        self.end_scroll_node_prepaint(Some(open));
        result
    }

    /// The current paint reads its containing scroll position in a way a
    /// translation cannot reproduce. Moving that node requires a fresh paint.
    pub fn paint_scroll_position_dependent(&mut self) {
        if let Some(&(index, _)) = self.next_frame.scroll_paint.open.last() {
            let id = self.next_frame.scroll_nodes[index as usize].id;
            let at = self.next_frame.scene.paint_operations.len();
            let space = self.next_frame.scene.scroll.space;
            self.next_frame.scroll_dependents.push((id, at, space));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{PaintOperation, Primitive};
    use crate::{Context, IntoElement, Quad, Render, TestAppContext, div};

    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }
    fn verify(window: &mut Window, anchored: bool, dependent: bool) {
        window.scroll_interest_margin = px(100.);
        let handle = ScrollHandle::new();
        handle.set_max_offset(point(px(0.), px(10_000.)));
        let token = Rc::new(());
        let id = ScrollNodeId::of_token(&token);
        window.invalidator.set_phase(DrawPhase::Prepaint);
        let (_, prepaint) = window.record_retained_prepaint(|window| {
            window.with_scroll_node_prepaint(&handle, rect(0., 0., 100., 50.), |window| {
                window.with_viewport_content_prepaint(
                    id,
                    rect(10., 10., 80., 30.),
                    anchored,
                    |_| {},
                );
            });
        });
        window.invalidator.set_phase(DrawPhase::Paint);
        let (_, paint) = window.record_retained_paint(|window| {
            window.with_scroll_node_paint(ScrollNodeId::of(&handle), |window| {
                if dependent {
                    window.paint_scroll_position_dependent();
                }
                window.with_scroll_node_paint(id, |window| {
                    let scale = window.scale_factor();
                    window.insert_scene_primitive(Quad {
                        bounds: rect(20., 20., 10., 10.).scale(scale),
                        content_mask: window.content_mask().scale(scale),
                        ..Default::default()
                    });
                });
            });
        });
        std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
        window.next_frame.clear();
        handle.set_offset(point(px(0.), px(-10.)));
        let plan = window.plan_scrolled_replay(&prepaint, &paint, None);
        if dependent {
            assert!(
                plan.is_none(),
                "position-sensitive paint cannot be translated"
            );
            return;
        }
        let plan = plan.expect("viewport paint has enough recorded interest");
        window.invalidator.set_phase(DrawPhase::Prepaint);
        window.replay_retained_prepaint_scrolled(&prepaint, &plan);
        window.invalidator.set_phase(DrawPhase::Paint);
        window.replay_retained_paint_scrolled(&paint, &plan);
        let quads: Vec<_> = window
            .next_frame
            .scene
            .paint_operations
            .iter()
            .filter_map(|op| match op {
                PaintOperation::Primitive(p @ Primitive::Quad(_)) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(quads.len(), 1);
        let scale = window.scale_factor();
        assert_eq!(
            crate::scene::placed_bounds(quads[0]),
            rect(20., 20., 10., 10.).scale(scale)
        );
        assert_eq!(
            quads[0].content_mask().bounds,
            rect(10., if anchored { 10. } else { 0. }, 80., 30.).scale(scale)
        );
    }

    #[crate::test]
    fn stationary_content_has_a_clip_that_scrolls(cx: &mut TestAppContext) {
        cx.add_window(|_, _| Empty)
            .update(cx, |_, window, _| verify(window, false, false))
            .unwrap();
    }
    #[crate::test]
    fn canvas_content_and_clip_both_stay_in_the_viewport(cx: &mut TestAppContext) {
        cx.add_window(|_, _| Empty)
            .update(cx, |_, window, _| verify(window, true, false))
            .unwrap();
    }
    #[crate::test]
    fn position_sensitive_paint_requires_a_build(cx: &mut TestAppContext) {
        cx.add_window(|_, _| Empty)
            .update(cx, |_, window, _| verify(window, false, true))
            .unwrap();
    }
}
