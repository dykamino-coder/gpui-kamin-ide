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
use super::commit::{Edited, Plan};
pub(in crate::document) use super::kept::Kept;
use super::mutation::Batch;
use super::patch::{Target, apply, attr_paths, patch};
use super::source_index::SourceIndex;
use crate::document::Document;
use crate::dom::Node;
use crate::paint_path::{self as why, refuse};
use crate::style::computed::Computed;
use paths::{painted_borders, style_paths};
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

    /// The colours of the box's borders change.
    fn borders(&self) -> bool {
        self.old.reset_border.border_colors != self.new.reset_border.border_colors
    }
}

impl Document {
    /// План пакета смен `class` и `data-*`, если перерасчёт стиля меняет у
    /// найденных элементов только значения краски; `None` — нужна сборка.
    /// `changed` — the names and node ids of the batch (`invalidation`).
    pub(super) fn repaint_plan(
        &self,
        batch: &mut Batch,
        changed: &crate::dom::invalidation::Changed,
        theme_css: &str,
    ) -> Option<Plan> {
        let values = &self.state.values;
        if !batch.texts.is_empty() || values.quirks() {
            return refuse(&why::REFUSED_PLAN_TEXT);
        }
        let input = why::why(values.traversal.take_source(), &why::REFUSED_PLAN_STATE)?;
        // The probes leave the cache in the state of the DOM after the
        // batch: it goes back only with a plan (otherwise the document is
        // rebuilt, and a fresh cache starts from its DOM).
        let mut kept = self.restyle.take().unwrap_or_default();
        let index = kept.ids(&values.traversal.elements());
        let restyles = why::why(index, &why::REFUSED_RESTYLE_PROBE).and_then(|index| {
            self.restyles(batch, changed, (&input, &index), theme_css, &mut kept.cache)
        });
        values.traversal.retain(input);
        let boxes = why::why(kept.boxes(&self.nodes), &why::REFUSED_PLAN_PATHS)?;
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
        let attrs_to_write = why::why(
            attr_paths(&self.nodes, boxes, &attrs),
            &why::REFUSED_PLAN_PATHS,
        )?;
        // Rules that change no cascaded value (an inline style or a more
        // specific rule wins): the batch writes only `class`/`data-*`, which the
        // frame build does not read; the frame stays as recorded (`Plan::Clean`).
        if restyles.is_empty() && attrs_to_write.is_empty() {
            return Some(Plan::Clean(None));
        }
        let style_paths = why::why(
            style_paths(&self.nodes, boxes, &restyles),
            &why::REFUSED_PLAN_PATHS,
        )?;
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, &attrs_to_write, &attrs) {
            return refuse(&why::REFUSED_PLAN_PATHS);
        }
        let edited = |nodes| Edited {
            nodes,
            attrs: attrs_to_write,
        };
        if restyles.is_empty() {
            return Some(Plan::Clean(Some(edited(nodes))));
        }
        for (path, id) in &style_paths {
            match (apply(&mut nodes, path), restyles.get(id)) {
                (Some(Node::Element(e)), Some(r)) => e.style.set_cascaded_paint(&r.new),
                _ => return refuse(&why::REFUSED_PLAN_PATHS),
            }
        }
        let mut restyled: Vec<u64> = restyles.into_keys().collect();
        restyled.sort_unstable();
        Some(Plan::Restyled(edited(nodes), restyled, style_paths))
    }

    /// Перерасчёты элементов, чей стиль меняет пакет, по номерам коробок;
    /// `None` — пакет доходит дальше значений краски.
    fn restyles(
        &self,
        batch: &mut Batch,
        changed: &crate::dom::invalidation::Changed,
        (input, index): (&crate::dom::input::Admitted, &SourceIndex),
        theme_css: &str,
        cache: &mut crate::dom::restyle_probe::RestyleCache,
    ) -> Option<HashMap<u64, Restyle>> {
        let values = &self.state.values;
        let media = self.state.media();
        let probe = cache.probe(input, theme_css, media, values).ok().flatten();
        let probe = why::why(probe, &why::REFUSED_RESTYLE_PROBE)?;
        // `attr()` reads attributes outside selectors (a `::before` of a
        // mutated element): such a document is rebuilt.
        let own = batch
            .elements
            .iter()
            .flat_map(|c| c.old.iter().chain(&c.new));
        let mut own = own.filter(|a| &*a.name.local == "style").map(|a| &*a.value);
        if probe.reads_attr() || own.any(|css| css.contains("attr(")) {
            return refuse(&why::REFUSED_RESTYLE_ATTR);
        }
        let mutated: Vec<_> = batch.elements.iter().map(|c| c.element.clone()).collect();
        let targets = cache.targets(&probe, changed, &mutated);
        let mut targets = why::why(targets, &why::REFUSED_RESTYLE_PROBE)?;
        // Unwrapped `html`/`body` (`document::unwrap_document`) fold their
        // values into the cascades of their children's boxes: the probe
        // folds them the same way.
        let unwrapped = fold::Unwrapped::add(&self.nodes, &input.dom.document, &mut targets);
        let unwrapped = why::why(unwrapped, &why::REFUSED_RESTYLE_ROOT)?;
        // The styles before the batch are those the last probe of each
        // target left (Blink and Stylo keep the old style on the element);
        // only targets no edit has probed yet are cascaded.
        let before = cache.kept_styles(&probe, input, values, &targets).ok();
        let before = why::why(before, &why::REFUSED_RESTYLE_PROBE)?;
        batch.write();
        let after = cache.styles(&probe, input, values, &targets);
        batch.undo();
        let after = why::why(after.ok(), &why::REFUSED_RESTYLE_PROBE)?;
        let vcx = self.state.values();
        let parent = match &unwrapped {
            Some(u) => Some(why::why(
                u.parent(|i| before[i].as_ref().map(|p| &*p.style), vcx),
                &why::REFUSED_RESTYLE_ROOT,
            )?),
            None => None,
        };
        let ids = targets.iter().map(|target| index.id(target));
        let mut out = HashMap::new();
        let entries = targets.iter().zip(ids).zip(before).zip(after).enumerate();
        for (i, (((target, id), old), new)) in entries {
            let (old, new) = why::why(old.zip(new), &why::REFUSED_RESTYLE_PROBE)?;
            if old.rest != new.rest {
                return refuse(&why::REFUSED_RESTYLE_REST);
            }
            if *old.style == *new.style {
                continue;
            }
            // A batch that restyles an unwrapped wrapper rebuilds: its values
            // reach every top-level box and the text of the page.
            if unwrapped.as_ref().is_some_and(|u| u.folds(i)) {
                return refuse(&why::REFUSED_RESTYLE_ROOT);
            }
            let id = why::why(id, &why::REFUSED_RESTYLE_PROBE)?;
            let (old, new) = match &parent {
                Some(p) => (
                    p.fold(target, old.style, vcx),
                    p.fold(target, new.style, vcx),
                ),
                None => (old.style, new.style),
            };
            // The frame may only repaint: layout reads the same values.
            let borders = painted_borders(&old) && painted_borders(&new);
            if !(old.same_layout(&new) || borders && old.same_layout_but_border_colors(&new)) {
                return refuse(&why::REFUSED_RESTYLE_LAYOUT);
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
                return refuse(&why::REFUSED_RESTYLE_ROOT);
            }
            out.insert(id, restyle);
        }
        Some(out)
    }
}
