//! Ревизия пакета смен классов, чей перерасчёт стиля меняет только
//! наследуемую группу UI (цвет) нескольких элементов: дерево коробок с копией
//! путей к ним и новой каскадной группой, без сборки документа.
//!
//! Blink перечитывает стиль только помеченных инвалидацией элементов
//! (`style_invalidator.cc:286`), потомкам с изменённым наследуемым — каскад
//! без сопоставления (`element.cc:5550`), а кадру — только перерисовку
//! (`computed_style.cc:747`, paint invalidation без layout). Stylo так же
//! каскадирует детей по их прежним правилам (`matching.rs:865`,
//! `RECASCADE_SELF`). Здесь элементы, которых может коснуться пакет, ищутся по
//! признакам правил (`dom::invalidation::targets`), их каскад до и после
//! записи пакета считает тот же вход в элемент, что и полная сборка
//! (`dom::restyle_probe`), а потомки получают новые наследуемые значения
//! повторной материализацией (`style::cascade::materialize`): их каскадные
//! стили те же. Факты раскладки прошлого дерева остаются основой (`Plan::Patched`).

use super::commit::Plan;
use super::mutation::Batch;
use super::patch::{Target, apply, attr_paths, elements, patch};
use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::computed::{Computed, InheritedUi, StyleGroup};
use std::collections::HashMap;

/// Новая группа UI коробок одного элемента и её прежнее значение.
struct Restyle {
    tag: String,
    old: StyleGroup<InheritedUi>,
    new: StyleGroup<InheritedUi>,
}

impl Document {
    /// План пакета смен `class`, если перерасчёт стиля меняет только
    /// наследуемую группу UI найденных элементов; `None` — нужна сборка.
    pub(super) fn repaint_plan(&self, batch: &Batch, theme_css: &str) -> Option<Plan> {
        let values = &self.state.values;
        if !batch.texts.is_empty() || values.quirks() {
            return None;
        }
        // Unwrapped `html`/`body` (`document::unwrap_document`) fold their
        // values into the cascaded styles of the top-level boxes.
        if !wrapped(&self.nodes) {
            return None;
        }
        let changed = self.invalidation(batch, theme_css)?;
        if changed.attrs.iter().any(|a| a != "class") {
            return None;
        }
        let input = values.traversal.take_source()?;
        let restyles = self.restyles(batch, &changed, &input, theme_css);
        values.traversal.retain(input);
        let restyles = restyles?;
        let ids = super::adopt::source_ids(
            &values.traversal.elements(),
            batch.elements.iter().map(|c| &c.element),
        );
        let mut attrs = HashMap::new();
        for (change, id) in batch.elements.iter().zip(ids) {
            attrs.insert(
                id?,
                Target {
                    tag: super::adopt::tag_of(&change.element),
                    old: crate::dom::element_attrs(&change.old),
                    new: crate::dom::element_attrs(&change.new),
                },
            );
        }
        let attr_paths = attr_paths(&self.nodes, &attrs)?;
        let style_paths = style_paths(&self.nodes, &restyles)?;
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, attr_paths, &attrs) {
            return None;
        }
        for (path, id) in style_paths {
            match (apply(&mut nodes, &path), restyles.get(&id)) {
                (Some(Node::Element(e)), Some(r)) => e.style.set_cascaded_ui(r.new.clone()),
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

    /// Новые группы UI элементов, чей стиль меняет пакет, по номерам коробок.
    fn restyles(
        &self,
        batch: &Batch,
        changed: &crate::dom::invalidation::Changed,
        input: &crate::dom::input::Admitted,
        theme_css: &str,
    ) -> Option<HashMap<u64, Restyle>> {
        let values = &self.state.values;
        let sheets = super::shape::sheets(&input.dom.document);
        let own = batch
            .elements
            .iter()
            .flat_map(|c| c.old.iter().chain(&c.new));
        let mut own = own.filter(|a| &*a.name.local == "style").map(|a| &*a.value);
        // `attr()` reads attributes outside selectors (a `::before` of a
        // mutated element): such a document is rebuilt.
        let all = sheets.iter().map(String::as_str);
        if all
            .chain(std::iter::once(theme_css))
            .any(|css| css.contains("attr("))
            || own.any(|css| css.contains("attr("))
        {
            return None;
        }
        let media = self.state.media();
        let probe = crate::dom::restyle_probe::RestyleProbe::new(input, theme_css, media, values)
            .ok()??;
        let mutated: Vec<_> = batch.elements.iter().map(|c| c.element.clone()).collect();
        let targets = crate::dom::invalidation::targets(probe.rules(), changed, &mutated)?;
        let before = probe.styles(input, values, &targets).ok()?;
        batch.write();
        let after = probe.styles(input, values, &targets);
        batch.undo();
        let after = after.ok()?;
        let ids = super::adopt::source_ids(&values.traversal.elements(), targets.iter());
        let mut out = HashMap::new();
        for (((target, id), old), new) in targets.iter().zip(ids).zip(before).zip(after) {
            let (id, old, new) = (id?, old?, new?);
            if old.rest != new.rest {
                return None;
            }
            if *old.style == *new.style {
                continue;
            }
            let tag = super::adopt::tag_of(target);
            if tag == "html" || !ui_only(&old.style, &new.style) {
                return None;
            }
            let (old, new) = (old.style.inh.ui.clone(), new.style.inh.ui.clone());
            out.insert(id, Restyle { tag, old, new });
        }
        Some(out)
    }
}

/// The tree keeps its `body` box (under `html` and the viewport box).
fn wrapped(nodes: &[Node]) -> bool {
    let mut level: Vec<&Node> = nodes.iter().collect();
    for _ in 0..3 {
        let mut next = Vec::new();
        for node in level {
            if let Node::Element(e) = node {
                if e.tag == "body" && e.node_id != 0 {
                    return true;
                }
                next.extend(e.children.iter());
            }
        }
        level = next;
    }
    false
}

/// `new` differs from `old` only in the inherited UI group, in values that
/// layout does not read (`Computed::same_layout`).
fn ui_only(old: &Computed, new: &Computed) -> bool {
    let mut probe = old.clone();
    std::rc::Rc::make_mut(&mut probe.inh.0).ui = new.inh.ui.clone();
    probe == *new && old.same_layout(new)
}

/// Пути к коробкам перерассчитанных элементов (с их тегом и прежней группой
/// UI), если ни одна коробка их поддеревьев не держит копий наследуемых
/// значений вне материализации; у каждого элемента — хотя бы одна коробка.
fn style_paths(
    nodes: &[Node],
    restyles: &HashMap<u64, Restyle>,
) -> Option<Vec<(super::patch::Path, u64)>> {
    let mut paths = Vec::new();
    let mut fits = true;
    if !restyles.is_empty() {
        elements(nodes, |e, path| {
            match restyles.get(&e.node_id) {
                Some(r) if e.tag == r.tag => {
                    fits &= e.style.cascaded().inh.ui == r.old && follows(e);
                    paths.push((path.to_vec(), e.node_id));
                }
                _ => {}
            }
            fits
        });
    }
    let found = |id: &u64| paths.iter().any(|(_, at)| at == id);
    (fits && restyles.keys().all(found)).then_some(paths)
}

/// Every box of the subtree of `e` takes new inherited values of `e` by
/// rematerialization: no hover or first-letter/-line layer, animation frames
/// or copies of computed values outside the cascaded stage.
fn follows(e: &Element) -> bool {
    elements(std::slice::from_ref(&Node::Element(e.clone())), |b, _| {
        b.hover.is_none()
            && b.first_letter.is_none()
            && b.first_line.is_none()
            && b.anim.is_none()
            && !b.style.holds_inherited_copies()
    })
}
