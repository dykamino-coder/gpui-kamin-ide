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
    pub(super) containing: Option<(u64, std::rc::Rc<crate::layout::layout_state::LayoutState>)>,
}

pub(super) async fn build_one_async(
    item: Item,
    bounds: Bounds<Pixels>,
    place: Place,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> (AnyElement, Option<super::DeferredPaint>) {
    let (spot, mut kid) = item;
    while let OofKid::Waiting(_, inner) = kid {
        kid = *inner;
    }
    if let OofKid::Trials(build, paint) = kid {
        return (build(bounds, spot, place, window, cx, stack).await, paint);
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
    root.layout_as_root_at_async(
        bounds.origin,
        size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        ),
        window,
        cx,
        stack,
    )
    .await;
    (root, paint)
}

impl Hooks for OofHost {
    type Layout = ();
    type Prepaint = Rc<RefCell<Vec<super::pending::Painted>>>;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
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

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        let bounds = self.containing.as_ref().map_or(bounds, |(cb, layout)| {
            crate::layout::positioned::anchor::containing_bounds(*cb, &layout.anchors)
                .unwrap_or_else(|| {
                    if *cb == 0 {
                        Bounds::new(point(px(0.), px(0.)), window.viewport_size())
                    } else {
                        bounds
                    }
                })
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
            let (mut root, paint) =
                build_one_async(item, bounds, self.place, window, cx, stack).await;
            root.prepaint_at_async(point(px(0.), px(0.)), window, cx, stack)
                .await;
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
            self.queue.pending.finish_async(window, cx, stack).await;
        }
        built
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _layout: &mut (),
        built: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, _bounds, _layout, built, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _layout: &'a mut (),
        built: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(async move {
            crate::hooks::paint_reopen_async(window, async |window| {
                let mut children = super::pending::PaintedGuard::take(built);
                for child in children.iter_mut() {
                    child.paint_async(window, cx, stack).await;
                }
            })
            .await;
        })
    }
}
