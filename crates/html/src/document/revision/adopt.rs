//! Ревизия без перерасчёта стиля: пакет записей мутаций, ни одна из которых не
//! может изменить ни одного каскадного значения.
//!
//! Так браузер обрабатывает мутацию атрибута: Blink сверяет изменённые классы и
//! атрибуты с `RuleFeatureSet` и не ставит перерасчёт, если ни одно правило их не
//! упоминает (`style_engine.cc:1867`); Gecko/Stylo делают то же через карту
//! инвалидации, а смену текста проверяют только по флагам `:empty` родителя
//! (`RestyleManager.cpp:345`). Здесь отличие — только атрибуты `class` и
//! `data-*` элементов HTML и тексты, а такие атрибуты движок читает лишь
//! селекторами и `attr()`: каскад их не видит, если ни один лист и собственный
//! `style` элемента их не упоминают (`mentions`). Тогда каскадный результат —
//! прежнее дерево, в котором у этих элементов (и их копий-коробок с тем же
//! `node_id`) атрибуты и тексты пакета; узлы без изменений остаются теми же
//! (`Children` делится, копируется только путь к правке), а значения документа
//! (арены, шрифты, at-правила, картинки) и соответствие исходных узлов узлам
//! коробок — прежними: записи пишутся в тот же исходный DOM.

use super::mentions::mentions;
use super::mutation::{AttrChange, Batch, open};
use super::patch::{Target, patch};
use super::text::{TextEdits, editable, patch_texts};
use crate::document::{Document, state::DocState};
use crate::dom::Node;
use std::collections::HashMap;
use std::rc::Rc;

impl Document {
    /// Дерево коробок после пакета `batch` без каскада, если ни одна правка не
    /// меняет каскад; `sheets` — тексты всех `<style>` документа, если уже
    /// известны. Ничего в ревизии не меняется.
    pub(super) fn plan(
        &self,
        batch: &Batch,
        theme_css: &str,
        sheets: Option<&[String]>,
        revision: &crate::budget::RevisionBudget,
    ) -> Option<Vec<Node>> {
        let values = &self.state.values;
        let (records, suppressed) = values.diagnostics_snapshot();
        let quiet = records.is_empty()
            && suppressed == 0
            && self.last_failure.is_none()
            && self.traversal_limit().is_none()
            && self.embedded_failure().is_none()
            && revision.failure().is_none();
        if !quiet || *self.theme_css != *theme_css {
            return None;
        }
        let root = values.source_dom()?;
        let mut targets = HashMap::new();
        if !batch.elements.is_empty() {
            let collected;
            let sheets = match sheets {
                Some(sheets) => sheets,
                None => {
                    collected = super::shape::sheets(&root);
                    &collected
                }
            };
            let mut all = vec![crate::style::ua::user_agent_css(), theme_css];
            if values.quirks() {
                all.push(crate::style::ua::QUIRKS_CSS);
            }
            all.extend(sheets.iter().map(String::as_str));
            let ids = source_ids(&values.traversal.elements());
            for change in &batch.elements {
                let id = ids.get(&(Rc::as_ptr(&change.element) as usize)).copied();
                match id.filter(|id| *id != AMBIGUOUS) {
                    Some(id) if open(&change.element, &root) && neutral(change, &all) => {
                        targets.insert(
                            id,
                            Target {
                                tag: tag_of(&change.element),
                                old: crate::dom::element_attrs(&change.old),
                                new: crate::dom::element_attrs(&change.new),
                            },
                        );
                    }
                    _ => return None,
                }
            }
        }
        let text_ids = source_ids(&values.traversal.texts());
        let read = values.traversal.read_texts();
        let mut texts: TextEdits<'_> = HashMap::new();
        for change in &batch.texts {
            let id = text_ids
                .get(&(Rc::as_ptr(&change.node) as usize))
                .copied()
                .filter(|id| *id != AMBIGUOUS)
                .and_then(|id| u32::try_from(id).ok())
                .and_then(crate::dom::TextId::new);
            match id {
                Some(id) if open(&change.node, &root) && editable(change, &read) => {
                    texts.insert(id, change);
                }
                _ => return None,
            }
        }
        let mut nodes = (*self.nodes).clone();
        (patch(&mut nodes, &targets) && patch_texts(&mut nodes, &texts)).then_some(nodes)
    }

    /// Следующая ревизия с деревом `nodes` и прежними значениями документа;
    /// пакет уже записан в исходный DOM, соответствие узлов не меняется.
    pub(super) fn adopted(
        &self,
        nodes: Vec<Node>,
        key: u64,
        revision: &crate::budget::RevisionBudget,
        #[cfg(feature = "engine-bench")] timings: crate::document::BuildTimings,
    ) -> Document {
        let state = DocState::adopting(&self.state);
        let base = state.values.images.borrow().base().cloned();
        state.values.admit_revision(revision.clone(), base.as_ref());
        // Документ отпускает выведенные группы стиля в Drop.
        state.values.hold_document();
        Document {
            state,
            nodes: Rc::new(nodes),
            retained: Default::default(),
            reuse: self.reuse.clone(),
            root: self.root.clone(),
            key,
            theme_css: self.theme_css.clone(),
            base_url: self.base_url.clone(),
            xml: self.xml,
            last_failure: None,
            #[cfg(feature = "engine-bench")]
            timings,
        }
    }
}

/// Один адрес у двух узлов коробок: такой узел правкой не адресуется.
const AMBIGUOUS: u64 = u64::MAX;

fn source_ids(elements: &[usize]) -> HashMap<usize, u64> {
    let mut ids = HashMap::with_capacity(elements.len());
    for (slot, &address) in elements.iter().enumerate() {
        if address != 0 {
            ids.entry(address)
                .and_modify(|id| *id = AMBIGUOUS)
                .or_insert(slot as u64 + 1);
        }
    }
    ids
}

/// Тег коробки элемента HTML: локальное имя (обход снимает только префикс).
fn tag_of(handle: &markup5ever_rcdom::Handle) -> String {
    match &handle.data {
        markup5ever_rcdom::NodeData::Element { name, .. } => name.local.to_string(),
        _ => String::new(),
    }
}

/// Не меняет ли отличие атрибутов элемента ни одного каскадного значения:
/// изменены только `class` и `data-*` без пространства имён, и ни один лист
/// (и собственный `style`, где `attr()` читает атрибуты элемента) не упоминает
/// ни имя изменённого атрибута, ни добавленный или снятый класс.
fn neutral(change: &AttrChange, sheets: &[&str]) -> bool {
    let (old, new) = (&change.old, &change.new);
    let value = |list: &[html5ever::Attribute], name: &html5ever::QualName| {
        list.iter()
            .find(|a| a.name == *name)
            .map(|a| a.value.to_string())
    };
    let (mut names, mut classes) = (Vec::new(), Vec::new());
    for attr in old.iter().chain(new.iter()) {
        let (was, is) = (value(old, &attr.name), value(new, &attr.name));
        if was == is {
            continue;
        }
        let local = &*attr.name.local;
        let plain = attr.name.ns == html5ever::ns!() && attr.name.prefix.is_none();
        let data = local.starts_with("data-") && !local.starts_with("data-kamin");
        if !plain || !(local == "class" || data) {
            return false;
        }
        if local == "class" {
            let tokens = |v: &Option<String>| -> Vec<String> {
                v.as_deref()
                    .unwrap_or("")
                    .split_ascii_whitespace()
                    .map(str::to_ascii_lowercase)
                    .collect()
            };
            let (a, b) = (tokens(&was), tokens(&is));
            classes.extend(a.iter().filter(|t| !b.contains(t)).cloned());
            classes.extend(b.iter().filter(|t| !a.contains(t)).cloned());
        }
        names.push(local.to_ascii_lowercase());
    }
    let own = value(
        new,
        &html5ever::QualName::new(None, html5ever::ns!(), "style".into()),
    );
    !sheets
        .iter()
        .copied()
        .chain(own.as_deref())
        .any(|css| mentions(css, &names, &classes))
}
