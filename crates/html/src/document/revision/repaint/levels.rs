//! The damage level of every element a batch restyles (`Levels`): the
//! restyle set is probed before and after the batch, and each target's
//! change is classified by the difference of its computed styles
//! (`Computed::damage`), as Blink derives a `StyleDifference` per element
//! (`ComputedStyle::VisualInvalidationDiff`) and Stylo a `RestyleDamage`
//! (`compute_style_difference`). No case refuses: what the probe cannot
//! decide widens the damage (the element's subtree, or the document when the
//! probe has no rules), never the set of reasons to rebuild.

use super::Restyle;
use super::fold;
use super::paths::painted_borders;
use crate::document::Document;
use crate::document::revision::mutation::Batch;
use crate::document::revision::source_index::SourceIndex;
use crate::dom::ProbedRest;
use crate::style::computed::Damage;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

/// An element whose boxes change (or the document), with the value that decides it.
#[derive(Clone)]
pub(super) struct Deeper {
    /// The source element; the document node for a document-wide damage.
    pub(super) handle: Handle,
    pub(super) why: &'static str,
}

/// The elements a batch restyles, by their damage: the restyles a box takes
/// in place (paint or layout values) by box number, and the elements whose
/// boxes change.
#[derive(Default)]
pub(super) struct Levels {
    pub(super) restyles: HashMap<u64, Restyle>,
    pub(super) deeper: Vec<Deeper>,
}

impl Levels {
    fn deeper(&mut self, handle: &Handle, why: &'static str) {
        let handle = handle.clone();
        self.deeper.push(Deeper { handle, why });
    }

    /// The whole document restyles: its rules or its cascades did not resolve.
    pub(super) fn document(document: &Handle, why: &'static str) -> Self {
        let mut levels = Self::default();
        levels.deeper(document, why);
        levels
    }
}

impl Document {
    /// The damage of each element whose style the batch changes.
    pub(super) fn restyles(
        &self,
        batch: &mut Batch,
        changed: &crate::dom::invalidation::Changed,
        (input, index): (&crate::dom::input::Admitted, &SourceIndex),
        theme_css: &str,
        cache: &mut crate::dom::restyle_probe::RestyleCache,
    ) -> Levels {
        let values = &self.state.values;
        let document = &input.dom.document;
        let media = self.state.media();
        let probe = crate::why::stage("probe", || cache.probe(input, theme_css, media, values));
        let Ok(Some(probe)) = probe else {
            return Levels::document(document, "probe");
        };
        let (mutated, after): (Vec<_>, Vec<&[_]>) = (batch.elements.iter())
            .map(|c| (c.element.clone(), &c.new[..]))
            .unzip();
        let found = || cache.targets(&probe, changed, (&mutated, &after));
        let mut targets = crate::why::stage("targets", found);
        // `attr()` reads an attribute of the element its declaration applies
        // to (css-values-5 §7.7): a changed name the sheets or a mutated
        // element's own `style=""` read restyles the mutated elements.
        let own = (batch.elements.iter()).flat_map(|c| c.old.iter().chain(&c.new));
        let styles = own.filter(|a| &*a.name.local == "style").map(|a| &*a.value);
        let reads = probe.reads_attr().with_styles(styles);
        let reads = reads.reads_any_of(changed.attrs.iter().map(String::as_str));
        let mut levels = Levels::default();
        if reads {
            // `attr()` in generated content (`::before`, `::after`, markers):
            // the pseudo-element boxes of a mutated element, which the probe
            // does not restyle, are built with its own box and no children
            // (`walk::shallow`) before and after the batch; boxes that differ
            // take the new value by a rebuild of its boxes (`attr`, `rebox`).
            // Its own style and every other target are probed as any.
            let traversal = &values.traversal;
            traversal.begin_rebuild();
            let old = cache.shallows(&probe, input, values, &mutated, &mutated);
            batch.write();
            let new = cache.shallows(&probe, input, values, &mutated, &mutated);
            batch.undo();
            traversal.abandon_rebuild();
            let (Ok(old), Ok(new)) = (old, new) else {
                return Levels::document(document, "budget");
            };
            for (m, (old, new)) in mutated.iter().zip(old.iter().zip(&new)) {
                let same = match (old, new) {
                    (Some(a), Some(b)) => super::rebox::same_generated(a, b),
                    (a, b) => a.is_none() && b.is_none(),
                };
                if !same {
                    levels.deeper(m, "attr");
                }
                if !targets.iter().any(|t| Rc::ptr_eq(t, m)) {
                    targets.push(m.clone());
                }
            }
        }
        crate::paint_path::add(&crate::paint_path::RESTYLED_ELEMENTS, targets.len() as u64);
        // Unwrapped `html`/`body` (`document::unwrap_document`) fold their
        // values into the cascades of their children's boxes: the probe
        // folds them the same way.
        let Some(unwrapped) = fold::Unwrapped::add(&self.nodes, document, &mut targets) else {
            return Levels::document(document, "unwrapped");
        };
        // The styles before the batch are those the last probe of each
        // target left (Blink and Stylo keep the old style on the element);
        // only targets no edit has probed yet are cascaded.
        let before = || cache.kept_styles(&probe, input, values, &targets);
        let Ok(before) = crate::why::stage("kept_styles", before) else {
            return Levels::document(document, "budget");
        };
        batch.write();
        let after = || cache.styles(&probe, input, values, &targets);
        let after = crate::why::stage("styles", after);
        batch.undo();
        crate::paint_path::add(&crate::paint_path::HAS_MARKED, cache.take_remarks());
        let Ok(after) = after else {
            return Levels::document(document, "budget");
        };
        let vcx = self.state.values();
        let parent = match &unwrapped {
            Some(u) => match u.parent(|i| before[i].as_ref().map(|p| &*p.style), vcx) {
                Some(p) => Some(p),
                None => return Levels::document(document, "unwrapped"),
            },
            None => None,
        };
        let entries = targets
            .iter()
            .zip(before.into_iter().zip(after))
            .enumerate();
        for (i, (target, (old, new))) in entries {
            // A target the cascade walk does not enter before or after (in a
            // `display: none` subtree, a dropped tag) generates no box either
            // time (css-display-3 §2.5): its new style paints nothing. One
            // that gains or loses its box changes the box tree.
            let (old, new) = match (old, new) {
                (Some(old), Some(new)) => (old, new),
                (None, None) => continue,
                _ => {
                    levels.deeper(target, "box");
                    continue;
                }
            };
            if let Some(why) = rest_change(&old.rest, &new.rest) {
                levels.deeper(target, why);
                continue;
            }
            // The hover layer of the box is a cascaded stage of its own
            // (`walk::pseudo_styles`): a paint change of it is written in
            // place with the box's, any other change rebuilds the box.
            let hover = match (&old.rest.hover, &new.rest.hover) {
                (a, b) if a == b => None,
                (Some(a), Some(b)) if parent.is_none() && a.damage(b, false).0 <= Damage::Paint => {
                    Some(b.clone())
                }
                _ => {
                    levels.deeper(target, "hover");
                    continue;
                }
            };
            if *old.style == *new.style && hover.is_none() {
                continue;
            }
            // A restyled unwrapped wrapper: its values reach every top-level
            // box and the text of the page.
            if unwrapped.as_ref().is_some_and(|u| u.folds(i)) {
                levels.deeper(target, "unwrapped");
                continue;
            }
            let Some(id) = index.id(target) else {
                levels.deeper(target, "unaddressed");
                continue;
            };
            let (old, new) = match &parent {
                Some(p) => (
                    p.fold(target, old.style, vcx),
                    p.fold(target, new.style, vcx),
                ),
                None => (old.style, new.style),
            };
            let borders = painted_borders(&old) && painted_borders(&new);
            let damage = match old.damage(&new, borders) {
                (Damage::None, _) => (Damage::Paint, "hover"),
                (Damage::Rebox, why) => {
                    super::why_value::note(why, &old, &new);
                    levels.deeper(target, why);
                    continue;
                }
                (Damage::Layout, why) => {
                    super::why_value::note(why, &old, &new);
                    (Damage::Layout, why)
                }
                paint => paint,
            };
            let restyle = Restyle {
                tag: super::super::adopt::tag_of(target),
                handle: target.clone(),
                damage,
                hover,
                old,
                new,
            };
            // The canvas takes its background from the root element or from
            // `body` (css-backgrounds-3 §2.11.2): document preparation
            // derives it, not the cascade of the element.
            let canvas = restyle.background() && restyle.tag == "body";
            if restyle.tag == "html" || canvas {
                levels.deeper(target, "canvas");
                continue;
            }
            levels.restyles.insert(id, restyle);
        }
        levels
    }
}

/// The first output of the walk's entry beside the style and the hover layer
/// that differs: each is a box of the element (pseudo-elements, a list marker, a shadow tree)
/// or reaches its descendants' cascades (custom properties).
fn rest_change(a: &ProbedRest, b: &ProbedRest) -> Option<&'static str> {
    [
        (a.vars != b.vars, "vars"),
        (a.first_letter != b.first_letter, "first_letter"),
        (a.first_line != b.first_line, "first_line"),
        // A probe numbers list items among its own entries, not the
        // document's: only whether the element is a list item compares. Its
        // number changes only with a counter, `display` or a box of another
        // element (css-lists-3 §4), which is that element's own damage.
        (a.list_item.is_some() != b.list_item.is_some(), "list_item"),
        (a.hidden != b.hidden, "hidden"),
        (a.box_level != b.box_level, "box_level"),
        (a.before != b.before, "before"),
        (a.shadow != b.shadow, "shadow"),
    ]
    .into_iter()
    .find_map(|(differs, name)| differs.then_some(name))
}
