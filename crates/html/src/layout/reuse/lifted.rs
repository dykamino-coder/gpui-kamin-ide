//! Paint ownership for draws lifted outside the root document layer.
//!
//! Each viewport-fixed draw records commands at its existing z-order position.
//! A replay keeps that layer slot; a paint-only edit changes the owner there,
//! instead of rebuilding the root because its layer does not own the draw.
//! Draws moving with a scroll node keep its primitive replay path: a retained
//! layer inside that node would disable scroll replay.
use super::Store;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, LayerHandle, Pixels, Window};
use std::rc::Rc;

pub(crate) fn deferred_layer(el: AnyElement, cx: &crate::layout::cx::LayoutCx) -> AnyElement {
    let Some(flow) = &cx.contracts.reuse else {
        return el;
    };
    if cx.contracts.document_root != Some(flow.doc) {
        return el;
    }
    Lifted {
        child: el,
        store: flow.store.clone(),
    }
    .hooked()
}

struct Lifted {
    child: AnyElement,
    store: Rc<Store>,
}
impl Wrap for Lifted {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let (_, recorded) = window.record_scene_layer(None, |window| self.child.paint(window, cx));
        if let gpui::RecordedLayer::Layer(layer) = recorded {
            window.paint_scene_layer(&layer);
            // Group layers are spliced when replayed. They retain the existing
            // rebuild path until their group commands can be updated in place.
            if !layer.content().has_groups()
                && layer.content().bounds().is_some_and(|b| {
                    let mask = window.content_mask().bounds.scale(window.scale_factor());
                    !b.intersect(&mask).is_empty()
                })
            {
                self.store.layer.lifted.borrow_mut().push(layer);
            }
        }
    }
}

impl Store {
    /// The layer owning this box, including draws painted after the root.
    /// Missing owners still use the root's existing offscreen/band checks.
    pub(crate) fn owner_layer(&self, owner: u64) -> Option<LayerHandle> {
        let root = self.scene_layer()?;
        let owner = u32::try_from(owner).ok()?;
        let holds = |l: &LayerHandle| l.painter(owner).is_some() || l.holds_group_root(owner);
        let layers = self.layer.lifted.borrow();
        let mut found = std::iter::once(&root)
            .chain(layers.iter())
            .filter(|l| holds(l));
        let layer = found.next().cloned();
        if found.next().is_some() {
            return None;
        }
        Some(layer.unwrap_or(root))
    }
}
