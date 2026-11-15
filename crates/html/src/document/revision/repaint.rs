//! Ревизия пакета смен `class` и `data-*`, чей перерасчёт стиля меняет у
//! нескольких элементов только значения краски (`Computed::same_layout`:
//! наследуемую группу UI с цветом и собственный фон коробки): дерево коробок
//! с копией путей к ним и новыми группами каскадного стиля, без сборки
//! документа.
//!
//! Blink перечитывает стиль только помеченных инвалидацией элементов
//! (`style_invalidator.cc:286`; набор самого элемента — `:154`, набор
//! потомков с признаком субъекта, в том числе именем атрибута, —
//! `invalidation_set.cc:120`), потомкам с изменённым наследуемым — каскад
//! без сопоставления (`element.cc:5550`), а кадру по разнице стилей — только
//! перерисовку (`layout_object.cc:2879`, paint invalidation без layout).
//! Stylo так же каскадирует детей по их прежним правилам (`matching.rs:865`,
//! `RECASCADE_SELF`). Здесь элементы, которых может коснуться пакет, ищутся по
//! признакам правил (`dom::invalidation::targets`), их каскад до и после
//! записи пакета считает тот же вход в элемент, что и полная сборка
//! (`dom::restyle_probe`), а потомки получают новые значения родителя
//! повторной материализацией (`style::cascade::materialize`): их каскадные
//! стили те же. Факты раскладки прошлого дерева остаются основой
//! (`Plan::Patched`), и каждый перерассчитанный элемент кадр может только
//! перекрасить (`Plan::Restyled`).

mod fold;
mod paths;

use super::boxes::Boxes;
use super::commit::Plan;
pub(in crate::document) use super::kept::Kept;
use super::mutation::Batch;
use super::patch::{Target, apply, attr_paths, patch};
use crate::document::Document;
use crate::dom::Node;
use crate::style::computed::Computed;
use paths::style_paths;
use std::collections::HashMap;

/// Перерасчёт одного элемента: тег его коробок и каскадный стиль до и после
/// пакета; стили раскладываются одинаково.
struct Restyle {
    tag: String,
    old: Box<Computed>,
    new: Box<Computed>,
}

impl Restyle {
    /// Меняются наследуемые значения: их получает всё поддерево.
    fn inherited(&self) -> bool {
        self.old.inh.ui != self.new.inh.ui
    }

    /// Меняется собственный фон коробки.
    fn background(&self) -> bool {
        self.old.reset_background != self.new.reset_background
    }
}

impl Document {
    /// План пакета смен `class` и `data-*`, если перерасчёт стиля меняет у
    /// найденных элементов только значения краски; `None` — нужна сборка.
    /// `changed` — the names and node ids of the batch (`invalidation`).
    pub(super) fn repaint_plan(
        &self,
        batch: &Batch,
        changed: &crate::dom::invalidation::Changed,
        theme_css: &str,
    ) -> Option<Plan> {
        let values = &self.state.values;
        if !batch.texts.is_empty() || values.quirks() {
            return None;
        }
        let input = values.traversal.take_source()?;
        // The probes leave the cache in the state of the DOM after the
        // batch: it goes back only with a plan (otherwise the document is
        // rebuilt, and a fresh cache starts from its DOM).
        let mut kept = self.restyle.take().unwrap_or_default();
        let restyles = self.restyles(batch, changed, &input, theme_css, &mut kept.cache);
        values.traversal.retain(input);
        let boxes = kept.boxes(&self.nodes)?;
        let plan = self.restyled(batch, &changed.elements, &boxes, restyles?)?;
        self.restyle.set(Some(kept));
        Some(plan)
    }

    /// The box tree with the attributes of `batch` and the paint values of
    /// `restyles` written along copied paths.
    /// `ids` — the node ids of the elements of `batch`, in its order.
    fn restyled(
        &self,
        batch: &Batch,
        ids: &[u64],
        boxes: &Boxes,
        restyles: HashMap<u64, Restyle>,
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
        let attr_paths = attr_paths(&self.nodes, boxes, &attrs)?;
        let style_paths = style_paths(&self.nodes, boxes, &restyles)?;
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, attr_paths, &attrs) {
            return None;
        }
        for (path, id) in style_paths {
            match (apply(&mut nodes, &path), restyles.get(&id)) {
                (Some(Node::Element(e)), Some(r)) => e.style.set_cascaded_paint(&r.new),
                _ => return None,
            }
        }
        if restyles.is_empty() {
            return Some(Plan::Patched(nodes));
        }
        let mut restyled: Vec<u64> = restyles.into_keys().collect();
        restyled.sort_unstable();
        Some(Plan::Restyled(nodes, restyled))
    }

    /// Перерасчёты элементов, чей стиль меняет пакет, по номерам коробок;
    /// `None` — пакет доходит дальше значений краски.
    fn restyles(
        &self,
        batch: &Batch,
        changed: &crate::dom::invalidation::Changed,
        input: &crate::dom::input::Admitted,
        theme_css: &str,
        cache: &mut crate::dom::restyle_probe::RestyleCache,
    ) -> Option<HashMap<u64, Restyle>> {
        let values = &self.state.values;
        let media = self.state.media();
        let probe = cache.probe(input, theme_css, media, values).ok()??;
        // `attr()` reads attributes outside selectors (a `::before` of a
        // mutated element): such a document is rebuilt.
        let own = batch
            .elements
            .iter()
            .flat_map(|c| c.old.iter().chain(&c.new));
        let mut own = own.filter(|a| &*a.name.local == "style").map(|a| &*a.value);
        if probe.reads_attr() || own.any(|css| css.contains("attr(")) {
            return None;
        }
        let mutated: Vec<_> = batch.elements.iter().map(|c| c.element.clone()).collect();
        let mut targets = cache.targets(&probe, changed, &mutated)?;
        // Unwrapped `html`/`body` (`document::unwrap_document`) fold their
        // values into the cascades of their children's boxes: the probe
        // folds them the same way.
        let unwrapped = fold::Unwrapped::add(&self.nodes, &input.dom.document, &mut targets)?;
        let before = cache.styles(&probe, input, values, &targets).ok()?;
        batch.write();
        let after = cache.styles(&probe, input, values, &targets);
        batch.undo();
        let after = after.ok()?;
        let vcx = self.state.values();
        let parent = match &unwrapped {
            Some(u) => Some(u.parent(|i| before[i].as_ref().map(|p| &*p.style), vcx)?),
            None => None,
        };
        let ids = super::adopt::element_ids(&values.traversal.elements(), targets.iter());
        let mut out = HashMap::new();
        let entries = targets.iter().zip(ids).zip(before).zip(after).enumerate();
        for (i, (((target, id), old), new)) in entries {
            let (old, new) = (old?, new?);
            if old.rest != new.rest {
                return None;
            }
            if *old.style == *new.style {
                continue;
            }
            // A batch that restyles an unwrapped wrapper rebuilds: its values
            // reach every top-level box and the text of the page.
            if unwrapped.as_ref().is_some_and(|u| u.folds(i)) {
                return None;
            }
            let id = id?;
            let (old, new) = match &parent {
                Some(p) => (
                    p.fold(target, old.style, vcx),
                    p.fold(target, new.style, vcx),
                ),
                None => (old.style, new.style),
            };
            // The frame may only repaint: layout reads the same values.
            if !old.same_layout(&new) {
                return None;
            }
            let restyle = Restyle {
                tag: super::adopt::tag_of(target),
                old,
                new,
            };
            // The canvas takes its background from the root element or from
            // `body` (css-backgrounds-3 §2.11.2): document preparation
            // derives it, not the cascade of the element.
            let canvas = restyle.background() && restyle.tag == "body";
            if restyle.tag == "html" || canvas {
                return None;
            }
            out.insert(id, restyle);
        }
        Some(out)
    }
}
