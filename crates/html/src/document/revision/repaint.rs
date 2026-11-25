//! The revision of a batch of `class` and `data-*` changes by the damage of
//! each element it restyles (`levels`): the elements the batch may reach are
//! found by the features of the rules (`dom::invalidation::targets`), their
//! cascades before and after the batch come from the same entry into the
//! element as a full build (`dom::restyle_probe`), and the difference of the
//! two computed styles is the element's damage (`Computed::damage`). When
//! every damage is paint, the boxes take the new paint values along copied
//! paths (`Plan::Restyled`; descendants follow by rematerialization). Damage
//! that reaches layout or the box tree goes to the roots of its subtrees
//! (`roots`).
//!
//! Blink recalculates only the elements invalidation marked
//! (`style_invalidator.cc:286`), cascades descendants with a changed
//! inherited value without matching (`element.cc:5550`) and turns the
//! difference of the styles into the frame's work (`StyleDifference`,
//! `layout_object.cc:2879`); Stylo recascades children by their old rules
//! (`matching.rs:865`, `RECASCADE_SELF`) and derives `RestyleDamage`.

mod fold;
mod levels;
mod paths;
pub(in crate::document) mod roots;
mod why_value;

use super::boxes::Boxes;
use super::commit::{Edited, Plan};
pub(in crate::document) use super::kept::Kept;
use super::mutation::Batch;
use super::patch::{Target, apply, attr_paths, patch};
use crate::document::Document;
use crate::dom::Node;
use crate::paint_path::{self as why, refuse};
use crate::style::computed::{Computed, Damage};
use paths::style_paths;
use std::collections::HashMap;

/// The restyle of one element its boxes take in place: their tag, the
/// cascaded style before and after the batch and its damage (`Paint` or
/// `Layout`, with the group that decides it).
struct Restyle {
    tag: String,
    handle: markup5ever_rcdom::Handle,
    damage: (Damage, &'static str),
    /// The new hover layer, when it changes (paint values only).
    hover: Option<Box<Computed>>,
    old: Box<Computed>,
    new: Box<Computed>,
}

impl Restyle {
    /// Inherited values change: the whole subtree takes them.
    fn inherited(&self) -> bool {
        let (old, new) = (&self.old.inh.0, &self.new.inh.0);
        old.ui != new.ui || old.font != new.font || old.text_flow != new.text_flow
    }

    /// Меняется собственный фон коробки.
    fn background(&self) -> bool {
        self.old.reset_background != self.new.reset_background
    }

    /// The colours of the box's borders change.
    fn borders(&self) -> bool {
        self.old.reset_border.border_colors != self.new.reset_border.border_colors
    }
}

impl Document {
    /// План пакета смен `class` и `data-*`: перерасчёт стиля найденных
    /// элементов и уровень повреждения каждого (`levels`). Пакет, у которого
    /// все уровни — краска, перекрашивает коробки на месте; уровни раскладки
    /// и коробок уходят в одну точку (`roots::fall_back`), `None` — сборка.
    /// `changed` — the names and node ids of the batch (`invalidation`).
    pub(super) fn repaint_plan(
        &self,
        batch: &mut Batch,
        changed: &crate::dom::invalidation::Changed,
        theme_css: &str,
    ) -> Option<Plan> {
        let values = &self.state.values;
        if !batch.texts.is_empty() {
            return refuse(&why::REFUSED_PLAN_TEXT);
        }
        if values.quirks() {
            return roots::fall_back_rebox("quirks");
        }
        let input = why::why(values.traversal.take_source(), &why::REFUSED_PLAN_STATE)?;
        // The probes leave the cache in the state of the DOM after the
        // batch: it goes back only with a plan (otherwise the document is
        // rebuilt, and a fresh cache starts from its DOM).
        let mut kept = self.restyle.take().unwrap_or_default();
        let levels = kept.ids(&values.traversal.elements()).map(|index| {
            let levels =
                self.restyles(batch, changed, (&input, &index), theme_css, &mut kept.cache);
            (levels, index)
        });
        values.traversal.retain(input);
        let Some((levels, index)) = levels else {
            return roots::fall_back_rebox("index");
        };
        let Some(boxes) = kept.boxes(&self.nodes) else {
            return roots::fall_back_rebox("boxes");
        };
        if !levels.deeper.is_empty() {
            return roots::fall_back(&levels, &index, &boxes);
        }
        let plan = self.restyled(batch, &changed.elements, &boxes, &levels.restyles)?;
        // A layout damage is the input of the layout of dirty subtrees over
        // the retained box tree (`Plan::Restyled` with `Damage::Layout`);
        // until the commit takes it, the batch is rebuilt at the one point.
        if levels
            .restyles
            .values()
            .any(|r| r.damage.0 == Damage::Layout)
        {
            return roots::fall_back(&levels, &index, &boxes);
        }
        why::add(&why::DAMAGE_PAINT, levels.restyles.len() as u64);
        self.restyle.set(Some(kept));
        Some(plan)
    }

    /// The box tree with the attributes of `batch` and the paint or layout
    /// values of `restyles` written along copied paths; `None` when some box does not
    /// take them in place, which rebuilds its boxes (`Damage::Rebox`).
    /// `ids` — the node ids of the elements of `batch`, in its order.
    fn restyled(
        &self,
        batch: &Batch,
        ids: &[u64],
        boxes: &Boxes,
        restyles: &HashMap<u64, Restyle>,
    ) -> Option<Plan> {
        let mut attrs = HashMap::new();
        for (change, &id) in batch.elements.iter().zip(ids) {
            attrs.insert(
                id,
                Target {
                    tag: super::adopt::tag_of(&change.element),
                    old: crate::dom::element_attrs(&change.old),
                    new: crate::dom::element_attrs(&change.new),
                },
            );
        }
        let Some(attrs_to_write) = attr_paths(&self.nodes, boxes, &attrs) else {
            return roots::fall_back_rebox("attributes rewritten by the build");
        };
        // Rules that change no cascaded value (an inline style or a more
        // specific rule wins): the batch writes only `class`/`data-*`, which the
        // frame build does not read; the frame stays as recorded (`Plan::Clean`).
        if restyles.is_empty() && attrs_to_write.is_empty() {
            return Some(Plan::Clean(None));
        }
        let Some(style_paths) = style_paths(&self.nodes, boxes, &restyles) else {
            return roots::fall_back_rebox("a box keeps copies of its paint values");
        };
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, &attrs_to_write, &attrs) {
            return roots::fall_back_rebox("attributes rewritten by the build");
        }
        let edited = |nodes| Edited {
            nodes,
            attrs: attrs_to_write,
        };
        if restyles.is_empty() {
            return Some(Plan::Clean(Some(edited(nodes))));
        }
        for (path, id) in &style_paths {
            let (Some(Node::Element(e)), Some(r)) = (apply(&mut nodes, path), restyles.get(id))
            else {
                return roots::fall_back_rebox("a box path");
            };
            let written = match r.damage.0 {
                Damage::Paint => {
                    e.style.set_cascaded_paint(&r.new);
                    true
                }
                _ => e.style.set_cascaded_layout(&r.old, &r.new),
            };
            let hovered = match (&r.hover, &mut e.hover) {
                (None, _) => true,
                (Some(hover), Some(layer)) => {
                    layer.set_cascaded_paint(hover);
                    true
                }
                (Some(_), None) => false,
            };
            if !(written && hovered) {
                return roots::fall_back_rebox("the build adjusted a value of the box");
            }
        }
        let damage = restyles
            .values()
            .map(|r| r.damage.0)
            .max()
            .unwrap_or(Damage::Paint);
        let mut restyled: Vec<u64> = restyles.keys().copied().collect();
        restyled.sort_unstable();
        Some(Plan::Restyled(edited(nodes), restyled, style_paths, damage))
    }
}
