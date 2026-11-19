//! Selected probes finish only the layout of their newly published Ready children.

use super::{Destination, OofKid, OofQueue, Place};
use gpui::{App, LayoutId, RetainedLayout, Window, point, px};

#[derive(Clone)]
pub(super) struct HostLayout {
    pub(super) id: LayoutId,
    pub(super) owner: RetainedLayout,
    pub(super) place: Place,
}

impl OofQueue {
    pub(in super::super) fn commit_prepared(
        &self,
        destination: &Destination,
        window: &mut Window,
        cx: &mut App,
    ) {
        let items = self.state.items.take();
        // Redirect before requesting layout: nested children must reach the live host.
        self.commit_to(destination);
        let host = destination
            .upgrade()
            .and_then(|queue| queue.state.preparation.borrow().clone());
        for (spot, kid) in items {
            let kid = match (kid, host.as_ref()) {
                (OofKid::Ready(child), Some(host)) => {
                    let mut owner = host.owner.clone();
                    let root = window.with_retained_layout(&mut owner, |window| {
                        // Host geometry belongs to its layout arena, not the source's scroll offset.
                        window.with_absolute_element_offset(point(px(0.), px(0.)), |window| {
                            let bounds = window.layout_bounds(host.id);
                            super::host::build_one(
                                (spot.clone(), OofKid::Ready(child)),
                                bounds,
                                host.place,
                                window,
                                cx,
                            )
                            .0
                        })
                    });
                    OofKid::Prepared(root)
                }
                (kid, _) => kid,
            };
            self.push_kid(spot, kid);
        }
    }
}
