//! Publication without split sources must not inspect the host layout arena.

use super::*;
use gpui::IntoElement;

#[gpui::test]
fn ordinary_publication_does_not_prepare_layout(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for children in [0, 1, 4] {
        let host = OofQueue::initial();
        let selected = OofQueue::default();
        for _ in 0..children {
            selected.push(Default::default(), gpui::div().into_any_element());
        }
        let destination = host.retain();
        // Any attempt to obtain a host arena for this batch would panic.
        let _unavailable = host.state.preparation.borrow_mut();
        cx.draw(
            point(px(0.), px(0.)),
            gpui::size(px(100.), px(100.)),
            |window, cx| {
                selected.commit_prepared(&destination, window, cx);
                assert!(selected.is_empty());
                assert_eq!(host.len(), children);
                assert!(
                    host.state
                        .items
                        .borrow()
                        .iter()
                        .all(|(_, kid)| { matches!(kid, OofKid::Ready(_)) })
                );
                // Even an initially empty selected queue must redirect late descendants.
                selected.push(Default::default(), gpui::div().into_any_element());
                assert_eq!(host.len(), children + 1);
                gpui::div().into_any_element()
            },
        );
    }
}
