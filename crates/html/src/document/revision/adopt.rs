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

use super::commit::Plan;
use super::mentions::mentions;
use super::mutation::{AttrChange, Batch, open};
use super::patch::{Target, attr_paths, patch};
use super::text::{TextEdits, editable, patch_texts, text_paths};
use crate::document::Document;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

impl Document {
    /// Дерево коробок после пакета `batch` без каскада, если ни одна правка не
    /// меняет каскад (`Plan`); `sheets` — тексты всех `<style>` документа, если
    /// уже известны. Ничего в ревизии не меняется.
    pub(super) fn plan(
        &self,
        batch: &Batch,
        theme_css: &str,
        sheets: Option<&[String]>,
        revision: &crate::budget::RevisionBudget,
    ) -> Option<Plan> {
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
            let handles = batch.elements.iter().map(|change| &change.element);
            let ids = source_ids(&values.traversal.elements(), handles);
            for (change, id) in batch.elements.iter().zip(ids) {
                match id {
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
                    // A class the sheets read: its restyle may still reach
                    // only inherited paint values (`repaint`).
                    Some(_) if open(&change.element, &root) => {
                        return self.repaint_plan(batch, theme_css);
                    }
                    _ => return None,
                }
            }
        }
        let handles = batch.texts.iter().map(|change| &change.node);
        let text_ids = source_ids(&values.traversal.texts(), handles);
        let read = values.traversal.read_texts();
        let mut texts: TextEdits<'_> = HashMap::new();
        for (change, id) in batch.texts.iter().zip(text_ids) {
            let id = id
                .and_then(|id| u32::try_from(id).ok())
                .and_then(crate::dom::TextId::new);
            match id {
                Some(id) if open(&change.node, &root) && editable(change, &read) => {
                    texts.insert(id, change);
                }
                _ => return None,
            }
        }
        // Пути к правкам ищутся в текущем дереве; без единого пути дерево
        // коробок после пакета то же самое, и копировать нечего.
        let attrs = attr_paths(&self.nodes, &targets)?;
        let found = text_paths(&self.nodes, &texts)?;
        if attrs.is_empty() && found.is_empty() {
            return Some(Plan::Clean(None));
        }
        let mut nodes = (*self.nodes).clone();
        if !(patch(&mut nodes, attrs, &targets) && patch_texts(&mut nodes, found, &texts)) {
            return None;
        }
        // Цели атрибутов — только `class` и `data-*` (`neutral`): их читают
        // лишь селекторы и `attr()` каскада, а не сборка кадра.
        Some(if texts.is_empty() {
            Plan::Clean(Some(nodes))
        } else {
            Plan::Patched(nodes)
        })
    }
}

/// Номера коробок (позиция в списке адресов сборки плюс один) исходных узлов
/// `handles`, в их порядке. Один проход по списку без карты всех узлов:
/// карта — только из адресов пакета. `None` — узел не построен или его адрес у
/// двух номеров (такой узел правкой не адресуется).
pub(super) fn source_ids<'a>(
    list: &[usize],
    handles: impl Iterator<Item = &'a Handle>,
) -> Vec<Option<u64>> {
    let wanted: Vec<usize> = handles.map(|h| Rc::as_ptr(h) as usize).collect();
    let mut found: HashMap<usize, Option<u64>> = wanted.iter().map(|&a| (a, None)).collect();
    let mut seen = std::collections::HashSet::with_capacity(wanted.len());
    for (slot, address) in list.iter().enumerate() {
        if let Some(id) = found.get_mut(address) {
            // Второе появление адреса делает узел неадресуемым.
            *id = seen.insert(*address).then_some(slot as u64 + 1);
        }
    }
    wanted.iter().map(|a| found[a]).collect()
}

/// Тег коробки элемента HTML: локальное имя (обход снимает только префикс).
pub(super) fn tag_of(handle: &markup5ever_rcdom::Handle) -> String {
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
