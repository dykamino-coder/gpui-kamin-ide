//! Ревизия без перерасчёта стиля: новая разметка той же формы, отличие которой
//! не может изменить ни одного каскадного значения.
//!
//! Так браузер обрабатывает мутацию атрибута: Blink сверяет изменённые классы и
//! атрибуты с `RuleFeatureSet` и не ставит перерасчёт, если ни одно правило их не
//! упоминает; Gecko/Stylo делают то же через карту инвалидации. Здесь отличие
//! ревизии — только атрибуты `class` и `data-*` элементов HTML (`shape`), а такие
//! атрибуты движок читает лишь селекторами и `attr()`: каскад их не видит, если
//! ни один лист и собственный `style` элемента их не упоминают (`mentions`).
//! Тогда каскадный результат — прежнее дерево, в котором у этих элементов (и их
//! копий-коробок с тем же `node_id`) атрибуты новой ревизии; узлы без изменений
//! остаются теми же (`Children` делится, копируется только путь к правке), а
//! значения документа (арены, шрифты, at-правила, картинки) — прежними.

use super::build::Source;
use super::mentions::mentions;
use super::patch::{Attrs, Target, patch};
use super::shape::{self, Changed};
use super::text::{TextEdit, editable, patch_texts};
use crate::document::{Document, state::DocState};
use std::collections::HashMap;
use std::rc::Rc;

impl Document {
    /// Следующая ревизия из `source` без каскада; иначе вход возвращается для
    /// полной сборки. Ничего в прежней ревизии не меняется до решения.
    pub(super) fn adopt(
        &self,
        source: Source,
        theme_css: &str,
        revision: &crate::budget::RevisionBudget,
    ) -> Result<Document, Box<Source>> {
        #[cfg(feature = "engine-bench")]
        let started = std::time::Instant::now();
        let values = &self.state.values;
        let (records, suppressed) = values.diagnostics_snapshot();
        let quiet = records.is_empty()
            && suppressed == 0
            && self.last_failure.is_none()
            && self.traversal_limit().is_none()
            && self.embedded_failure().is_none()
            && revision.failure().is_none();
        if !quiet || *self.theme_css != *theme_css {
            return Err(Box::new(source));
        }
        let Some(old) = values.source_dom() else {
            return Err(Box::new(source));
        };
        let Some(shape) = shape::compare(&old, &source.input.dom.document) else {
            return Err(Box::new(source));
        };
        let mut sheets = vec![crate::style::ua::user_agent_css(), theme_css];
        if values.quirks() {
            sheets.push(crate::style::ua::QUIRKS_CSS);
        }
        sheets.extend(shape.sheets.iter().map(String::as_str));
        let ids = source_ids(&values.traversal.elements());
        let mut targets = HashMap::new();
        for changed in &shape.changed {
            let id = ids.get(&(Rc::as_ptr(&changed.old) as usize)).copied();
            match id.filter(|id| *id != AMBIGUOUS) {
                Some(id) if neutral(changed, &sheets) => {
                    targets.insert(
                        id,
                        Target {
                            tag: tag_of(&changed.old),
                            old: attrs_of(&changed.old),
                            new: attrs_of(&changed.new),
                        },
                    );
                }
                _ => return Err(Box::new(source)),
            }
        }
        let text_ids = source_ids(&values.traversal.texts());
        let read = values.traversal.read_texts();
        let mut texts: HashMap<crate::dom::TextId, TextEdit> = HashMap::new();
        for edit in &shape.texts {
            let id = text_ids
                .get(&edit.node)
                .copied()
                .filter(|id| *id != AMBIGUOUS)
                .and_then(|id| u32::try_from(id).ok())
                .and_then(crate::dom::TextId::new);
            match id {
                Some(id) if editable(edit, &read) => {
                    let edit = TextEdit {
                        old: edit.old.clone(),
                        new: edit.new.clone(),
                    };
                    texts.insert(id, edit);
                }
                _ => return Err(Box::new(source)),
            }
        }
        let mut nodes = (*self.nodes).clone();
        if !patch(&mut nodes, &targets) || !patch_texts(&mut nodes, &texts) {
            return Err(Box::new(source));
        }
        let moved: HashMap<usize, usize> = shape.nodes.iter().copied().collect();
        let remap = |old: &[usize]| -> Vec<usize> {
            old.iter()
                .map(|old| moved.get(old).copied().unwrap_or(0))
                .collect()
        };
        let elements = remap(&values.traversal.elements());
        let source_texts = remap(&values.traversal.texts());
        let read_next = read
            .iter()
            .filter_map(|old| moved.get(old).copied())
            .collect();
        drop(read);
        let state = DocState::adopting(&self.state);
        state
            .values
            .admit_revision(revision.clone(), source.base.as_ref());
        state
            .values
            .traversal
            .readopt(source.input, elements, source_texts, read_next);
        Ok(Document {
            _derived: crate::style::computed::DerivedHold::new(&state.values),
            state,
            nodes: Rc::new(nodes),
            retained: Default::default(),
            reuse: self.reuse.clone(),
            root: self.root.clone(),
            key: source.key,
            theme_css: self.theme_css.clone(),
            base_url: self.base_url.clone(),
            xml: self.xml,
            last_failure: None,
            #[cfg(feature = "engine-bench")]
            timings: crate::document::BuildTimings {
                parse: source.parse,
                style: started.elapsed(),
            },
        })
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

fn attrs_of(handle: &markup5ever_rcdom::Handle) -> Attrs {
    match &handle.data {
        markup5ever_rcdom::NodeData::Element { attrs, .. } => {
            crate::dom::element_attrs(&attrs.borrow())
        }
        _ => Vec::new(),
    }
}

/// Не меняет ли отличие атрибутов элемента ни одного каскадного значения:
/// изменены только `class` и `data-*` без пространства имён, и ни один лист
/// (и собственный `style`, где `attr()` читает атрибуты элемента) не упоминает
/// ни имя изменённого атрибута, ни добавленный или снятый класс.
fn neutral(changed: &Changed, sheets: &[&str]) -> bool {
    let (
        markup5ever_rcdom::NodeData::Element { attrs: old, .. },
        markup5ever_rcdom::NodeData::Element { attrs: new, .. },
    ) = (&changed.old.data, &changed.new.data)
    else {
        return false;
    };
    let (old, new) = (old.borrow(), new.borrow());
    let value = |list: &[html5ever::Attribute], name: &html5ever::QualName| {
        list.iter()
            .find(|a| a.name == *name)
            .map(|a| a.value.to_string())
    };
    let (mut names, mut classes) = (Vec::new(), Vec::new());
    for attr in old.iter().chain(new.iter()) {
        let (was, is) = (value(&old, &attr.name), value(&new, &attr.name));
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
        &new,
        &html5ever::QualName::new(None, html5ever::ns!(), "style".into()),
    );
    !sheets
        .iter()
        .copied()
        .chain(own.as_deref())
        .any(|css| mentions(css, &names, &classes))
}
