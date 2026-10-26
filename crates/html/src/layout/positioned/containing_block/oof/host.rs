//! Хост строит OOF после потока, затем ICB завершает отложенные зависимости.

use super::{Item, OofKid, OofQueue, Place};
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, LayoutId, ParentElement, Pixels, Styled,
    Window, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(super) struct OofHost {
    pub(super) queue: OofQueue,
    pub(super) place: Place,
    pub(super) containing: Option<(u64, crate::document::state::ValueDoc)>,
}

pub(super) fn build_one(
    item: Item,
    bounds: Bounds<Pixels>,
    place: Place,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, Option<super::DeferredPaint>) {
    let (spot, mut kid) = item;
    while let OofKid::Waiting(_, inner) = kid {
        kid = *inner;
    }
    if let OofKid::Trials(build, paint) = kid {
        return (build(bounds, spot, place, window, cx), paint);
    }
    let (child, paint) = match kid {
        OofKid::Ready(child) => (child, None),
        OofKid::Deferred(build, paint) => (build(bounds, window, cx), paint),
        _ => unreachable!(),
    };
    let mut root = gpui::div()
        .relative()
        .w(bounds.size.width)
        .h(bounds.size.height)
        .child(place(spot, child))
        .into_any_element();
    root.layout_as_root_at(
        bounds.origin,
        size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        ),
        window,
        cx,
    );
    (root, paint)
}

impl Hooks for OofHost {
    type Layout = ();
    type Prepaint = Rc<RefCell<Vec<super::pending::Painted>>>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        if self.containing.is_some() {
            return (window.request_layout(Default::default(), [], cx), ());
        }
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let bounds = self.containing.as_ref().map_or(bounds, |(cb, values)| {
            crate::layout::positioned::anchor::containing_bounds(*cb, values.vcx()).unwrap_or_else(
                || {
                    if *cb == 0 {
                        Bounds::new(point(px(0.), px(0.)), window.viewport_size())
                    } else {
                        bounds
                    }
                },
            )
        });
        let built = Rc::new(RefCell::new(Vec::new()));
        let context = self.queue.pending.context(window);
        loop {
            let item = self.queue.items.borrow_mut().pop_front();
            let Some(item) = item else { break };
            if matches!(&item.1, OofKid::Waiting(deps, _) if !deps.ready()) {
                self.queue
                    .pending
                    .push(item, bounds, self.place, context.clone(), built.clone());
                continue;
            }
            let siblings = self.queue.items.take();
            let (mut root, paint) = build_one(item, bounds, self.place, window, cx);
            root.prepaint_at(point(px(0.), px(0.)), window, cx);
            match paint {
                Some(paint) => paint.publish(root, window.content_mask()),
                None => built
                    .borrow_mut()
                    .push(super::pending::Painted(root, window.content_mask())),
            }
            self.queue.items.borrow_mut().extend(siblings);
        }
        self.queue
            .pending
            .register(&self.queue, bounds, self.place, context, &built);
        if self.queue.initial {
            self.queue.pending.finish(window, cx);
        }
        built
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        built: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_reopen(window, |window| {
            for child in built.borrow_mut().iter_mut() {
                child.paint(window, cx);
            }
        });
    }
}
