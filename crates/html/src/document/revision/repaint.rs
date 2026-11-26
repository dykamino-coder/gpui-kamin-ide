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
mod rebox;
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

    /// The box's own background changes.
    fn background(&self) -> bool {
        self.old.reset_background != self.new.reset_background
    }

    /// The colours of the box's borders change.
    fn borders(&self) -> bool {
        self.old.reset_border.border_colors != self.new.reset_border.border_colors
    }
}

impl Document {
    /// The plan of a batch of `class` and `data-*` changes: the restyle of the
    /// elements found and the damage of each (`levels`). A batch whose every
    /// damage is paint repaints the boxes in place; layout and box damage go
    /// to one point (`roots::fall_back`); `None` is a build.
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
            return roots::fall_back_document("quirks");
        }
        let source = crate::why::stage("source", || values.traversal.take_source());
        let input = why::why(source, &why::REFUSED_PLAN_STATE)?;
        // The probes leave the cache in the state of the DOM after the
        // batch: it goes back only with a plan (otherwise the document is
        // rebuilt, and a fresh cache starts from its DOM).
        let mut kept = self.restyle.take().unwrap_or_default();
        let index = crate::why::stage("ids", || kept.ids(&values.traversal.elements()));
        let levels = index.map(|index| {
            let levels = crate::why::stage("levels", || {
                self.restyles(batch, changed, (&input, &index), theme_css, &mut kept.cache)
            });
            (levels, index)
        });
        let Some((levels, index)) = levels else {
            values.traversal.retain(input);
            return roots::fall_back_document("index");
        };
        let Some(boxes) = crate::why::stage("boxes", || kept.boxes(&self.nodes)) else {
            values.traversal.retain(input);
            return roots::fall_back_document("boxes");
        };
        let roots = roots::roots(&levels);
        if roots.is_empty() {
            values.traversal.retain(input);
            let plan = crate::why::stage("restyled", || {
                self.restyled(batch, &changed.elements, &boxes, &levels.restyles, None)
            })?;
            why::add(&why::DAMAGE_PAINT, levels.restyles.len() as u64);
            self.restyle.set(Some(kept));
            return Some(plan);
        }
        let reboxed = crate::why::stage("reboxed", || {
            let units = (&input, &*index, &*boxes);
            self.reboxed(batch, roots, &levels, units, theme_css, &mut kept)
        });
        values.traversal.retain(input);
        let reboxed = match reboxed {
            Ok(reboxed) => reboxed,
            Err(cause) => {
                let causes = roots::causes(&levels);
                return roots::fall_back_document(&format!("{cause} ({causes})"));
            }
        };
        // Paint restyles outside the rebuilt subtrees are written in place.
        let restyles: HashMap<u64, Restyle> = levels
            .restyles
            .into_iter()
            .filter(|(_, r)| r.damage.0 == Damage::Paint && !reboxed.covers(&r.handle))
            .collect();
        let Some(plan) = self.restyled(batch, &changed.elements, &boxes, &restyles, Some(&reboxed))
        else {
            values.traversal.abandon_rebuild();
            return None;
        };
        why::add(&why::DAMAGE_PAINT, restyles.len() as u64);
        why::add(&why::REBOX_UNITS, reboxed.0.len() as u64);
        reboxed.commit(values);
        kept.reshaped();
        self.restyle.set(Some(kept));
        Some(plan)
    }

    /// The box tree with the attributes of `batch`, the paint values of
    /// `restyles` and the rebuilt boxes of `units` written along copied
    /// paths; `None` when some box does not take them in place, which makes
    /// the document the root (`roots::fall_back_document`).
    /// `ids` — the node ids of the elements of `batch`, in its order.
    fn restyled(
        &self,
        batch: &Batch,
        ids: &[u64],
        boxes: &Boxes,
        restyles: &HashMap<u64, Restyle>,
        units: Option<&rebox::Reboxed>,
    ) -> Option<Plan> {
        let mut attrs = HashMap::new();
        for (change, &id) in batch.elements.iter().zip(ids) {
            // A rebuilt box carries the new attributes already.
            if units.is_some_and(|u| u.covers(&change.element)) {
                continue;
            }
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
            return roots::fall_back_document("attributes rewritten by the build");
        };
        let units = units.filter(|u| !u.0.is_empty());
        // Rules that change no cascaded value (an inline style or a more
        // specific rule wins): the batch writes only `class`/`data-*`, which the
        // frame build does not read; the frame stays as recorded (`Plan::Clean`).
        if restyles.is_empty() && attrs_to_write.is_empty() && units.is_none() {
            return Some(Plan::Clean(None));
        }
        let style_paths = match style_paths(&self.nodes, boxes, restyles, self.state.values()) {
            Ok(paths) => paths,
            Err(why) => return roots::fall_back_document(why),
        };
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, &attrs_to_write, &attrs) {
            return roots::fall_back_document("attributes rewritten by the build");
        }
        let edited = |nodes| Edited {
            nodes,
            attrs: attrs_to_write,
        };
        if restyles.is_empty() && units.is_none() {
            return Some(Plan::Clean(Some(edited(nodes))));
        }
        for (path, id) in &style_paths {
            let (Some(Node::Element(e)), Some(r)) = (apply(&mut nodes, path), restyles.get(id))
            else {
                return roots::fall_back_document("a box path");
            };
            e.style.set_cascaded_paint(&r.new);
            let hovered = match (&r.hover, &mut e.hover) {
                (None, _) => true,
                (Some(hover), Some(layer)) => {
                    layer.set_cascaded_paint(hover);
                    true
                }
                (Some(_), None) => false,
            };
            if !hovered {
                return roots::fall_back_document("the build adjusted a value of the box");
            }
        }
        let mut restyled: Vec<u64> = restyles.keys().copied().collect();
        let damage = match units {
            None => Damage::Paint,
            Some(units) => {
                let vcx = self.state.values();
                for unit in &units.0 {
                    let Some(slot) = apply(&mut nodes, &unit.path) else {
                        return roots::fall_back_document("a box path");
                    };
                    *slot = Node::Element(unit.node.clone());
                    // The images of the new styles join the document's.
                    crate::values::image_values::prepare(std::slice::from_ref(slot), vcx);
                    restyled.extend_from_slice(&unit.ids);
                }
                if units.0.iter().any(|u| u.masks) {
                    self.state.values.masks.collect(&nodes, vcx);
                }
                Damage::Rebox
            }
        };
        restyled.sort_unstable();
        restyled.dedup();
        Some(Plan::Restyled(edited(nodes), restyled, style_paths, damage))
    }
}
