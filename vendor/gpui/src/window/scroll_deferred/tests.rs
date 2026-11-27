use super::*;
use crate::{ScrollHandle, TestAppContext, deferred, div, prelude::*, px};

/// A scroller whose second row is lifted above the flow by a deferred draw:
/// owned by the scroller's node (`in_scroll_node`) or a plain one.
struct Lifted {
    handle: ScrollHandle,
    owned: bool,
}

impl Render for Lifted {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = || div().h(px(20.)).w(px(50.)).bg(crate::red());
        let lifted = deferred(row().id("lifted").cursor_pointer()).with_priority(1);
        let lifted = if self.owned { lifted.in_scroll_node() } else { lifted };
        div()
            .id("scroller")
            .size(px(100.))
            .overflow_y_scroll()
            .track_scroll(&self.handle)
            .child(row())
            .child(lifted)
            .children((0..20).map(|_| row()))
    }
}

fn draw(cx: &mut TestAppContext, owned: bool) -> crate::AnyWindowHandle {
    let handle = ScrollHandle::new();
    let window = cx.add_window(move |_, _| Lifted { handle, owned });
    cx.run_until_parked();
    window.into()
}

/// An owned draw keeps the scroller's node: its commands are marked under
/// it and its hitbox too, while its paint stays the draw's own range.
#[crate::test]
fn an_owned_deferred_draw_paints_under_its_scroll_node(cx: &mut TestAppContext) {
    let window = draw(cx, true);
    window
        .update(cx, |_, window, _| {
            let frame = &window.rendered_frame;
            let [draw] = &frame.deferred_draws[..] else { panic!("one deferred draw") };
            let owner = draw.scroll.owner.expect("the scroller's node") as usize;
            let node = &frame.scroll_nodes[owner];
            let painted = draw.paint_range.start.scene_index..draw.paint_range.end.scene_index;
            assert!(!painted.is_empty());
            assert!(node.more.is_empty() && !node.torn);
            let marks = frame.scene.scroll.within(&painted);
            assert!(!marks.is_empty() && marks.iter().all(|mark| mark.node == node.id));
            let hitboxes = draw.prepaint_range.start.hitboxes_index..draw.prepaint_range.end.hitboxes_index;
            assert!(!hitboxes.is_empty());
            let marked = frame.scroll_hitboxes.iter().filter(|mark| hitboxes.contains(&(mark.index as usize)));
            assert!(marked.clone().count() == hitboxes.len() && marked.into_iter().all(|mark| mark.node == node.id));
        })
        .unwrap();
}

/// DevTools names the command of an owned draw that no node marks: the
/// draw, the line that deferred it, its element and the command.
#[crate::test]
fn an_unmarked_command_of_a_deferred_draw_is_named(cx: &mut TestAppContext) {
    let window = draw(cx, true);
    window
        .update(cx, |_, window, _| {
            let end = PrepaintStateIndex { deferred_draws_index: window.rendered_frame.deferred_draws.len(), ..Default::default() };
            let whole = PrepaintStateIndex::default()..end;
            assert_eq!(window.deferred_refusal_detail(&whole), None, "every command of the draw is marked");
            let frame = &mut window.rendered_frame;
            let paint = &frame.deferred_draws[0].paint_range;
            let painted = paint.start.scene_index..paint.end.scene_index;
            frame.scene.scroll.marks.retain(|mark| !painted.contains(&(mark.op as usize)));
            let detail = window.deferred_refusal_detail(&whole).expect("the unmarked command is named");
            for part in ["deferred draw 0", "window/scroll_deferred/tests.rs:", "Name(\"scroller\")", "Unmarked: command", "quad at"] {
                assert!(detail.contains(part), "{part} in {detail}");
            }
        })
        .unwrap();
}

/// A plain draw (a popover escaping its scroller) stays outside the node,
/// and a record holding it cannot be replayed scrolled.
#[crate::test]
fn a_plain_deferred_draw_has_no_scroll_node(cx: &mut TestAppContext) {
    let window = draw(cx, false);
    window
        .update(cx, |_, window, _| {
            let frame = &window.rendered_frame;
            let [draw] = &frame.deferred_draws[..] else { panic!("one deferred draw") };
            assert!(frame.scroll_nodes.iter().all(|node| node.more.is_empty()));
            let painted = draw.paint_range.start.scene_index..draw.paint_range.end.scene_index;
            assert!(frame.scene.scroll.within(&painted).is_empty());
            assert_eq!(window.deferred_scroll_part(draw).err(), Some(ScrollRefusal::DeferredDraws));
            // DevTools reads why, and the line that deferred it (`Lifted::render`).
            assert_eq!(draw.scroll_refusal(), Some("no scroll node (a plain defer_draw)"));
            assert!(draw.site.file().replace('\\', "/").ends_with("window/scroll_deferred/tests.rs"), "{}", draw.site);
        })
        .unwrap();
}
