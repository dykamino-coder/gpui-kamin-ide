//! Prepaint of a resumed paragraph's atoms: every atom is placed by
//! arithmetic, only those that meet the content mask get an element (`super`).

use super::*;
use crate::hooks::IntoHooked;
use gpui::{
    App, AvailableSpace, IntoElement, ParentElement, Point, Styled, Window, point, px, size,
};
use std::cell::Cell;

/// Tolerance of a built atom's size against its kept size.
const SIZE_EPS: f32 = 0.01;

impl Paragraph {
    /// Place the atoms of a resumed paragraph at `origins`; `false` for a
    /// paragraph built in this frame.
    pub(in crate::text::paragraph) fn place_kept(
        &mut self,
        origins: &[Option<Point<Pixels>>],
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let Some(s) = self.kept.resumed() else {
            return false;
        };
        let (Some(lazy), Some(atoms)) = (s.lazy.take(), s.atoms.clone()) else {
            return false;
        };
        let sizes = std::mem::take(&mut s.sizes);
        let mut built = std::mem::take(&mut s.built);
        built.clear();
        let (mut skipped, mut broken) = (None::<Bounds<Pixels>>, false);
        let mask = window.content_mask().bounds;
        for (k, origin) in origins.iter().enumerate() {
            let (Some(origin), Some(&(w, h))) = (*origin, sizes.get(k)) else {
                continue;
            };
            let rect = Bounds {
                origin,
                size: size(px(w), px(h)),
            };
            if !rect.intersects(&mask) {
                skipped = Some(skipped.map_or(rect, |s| s.union(&rect)));
                continue;
            }
            match self.build_kept(&lazy, &atoms, k, (w, h), origin, window, cx) {
                Some(slot) => built.push((k, slot)),
                None => broken = true,
            }
        }
        crate::work::add!(KeptAtomsBuilt, built.len() as u64);
        let s = self.kept.state();
        (s.lazy, s.sizes, s.built, s.skipped) = (Some(lazy), sizes, built, skipped);
        s.broken |= broken;
        true
    }

    /// Build, lay out and prepaint atom `k` at `origin`; `None` if it is gone
    /// or no longer has its kept size `(w, h)`.
    #[allow(clippy::too_many_arguments)]
    fn build_kept(
        &self,
        lazy: &LazyBuild,
        atoms: &KeptAtoms,
        k: usize,
        (w, h): (f32, f32),
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AtomSlot> {
        let b = *self.atom_boxes.get(k)?;
        let root = Rc::new(Cell::new(None));
        let mut el = LayoutTap {
            child: gpui::div()
                .flex()
                .items_baseline()
                .child(lazy.atom(atoms, k)?)
                .into_any_element(),
            slot: root.clone(),
        }
        .hooked();
        let space = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
        el.layout_as_root(space, window, cx);
        let id = root.get()?;
        let s = window.layout_exact(id).1;
        if (f32::from(s.width) - w).abs() > SIZE_EPS || (f32::from(s.height) - h).abs() > SIZE_EPS {
            return None;
        }
        window.set_layout_root_origin(id, origin);
        el.prepaint_at(point(px(0.), px(0.)), window, cx);
        Some(AtomSlot {
            at: b.at,
            el,
            align: b.align,
            root,
            extents: Default::default(),
            hidden: false,
            fixed_inline: true,
        })
    }
}
