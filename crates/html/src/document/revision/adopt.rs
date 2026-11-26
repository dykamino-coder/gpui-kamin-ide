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

use super::commit::{Edited, Plan};
use super::mentions::{Mentions, mentions};
use super::mutation::{AttrChange, Batch, open};
use super::patch::{Target, attr_paths, patch};
use super::repaint::roots::fall_back_document;
use super::text::{TextEdits, editable, patch_texts, text_paths};
use crate::document::Document;
use crate::paint_path as why;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

impl Document {
    /// Дерево коробок после пакета `batch` без каскада, если ни одна правка не
    /// меняет каскад (`Plan`); `sheets` — тексты всех `<style>` документа, если
    /// уже известны; `changed` — имена и номера узлов пакета (`invalidation`),
    /// посчитанные один раз на пакет. Ничего в ревизии не меняется.
    pub(super) fn plan(
        &self,
        batch: &mut Batch,
        theme_css: &str,
        sheets: Option<&[String]>,
        changed: Option<&crate::dom::invalidation::Changed>,
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
            return why::refuse(&why::REFUSED_PLAN_STATE);
        }
        let root = why::why(values.source_dom(), &why::REFUSED_PLAN_STATE)?;
        let mut targets = HashMap::new();
        if !batch.elements.is_empty() {
            // The names the sheets mention, indexed once for the document.
            let all = self.kept_mentions(&root, sheets, theme_css);
            let ids = match changed {
                Some(changed) => changed.elements.iter().copied().map(Some).collect(),
                None => {
                    let handles = batch.elements.iter().map(|change| &change.element);
                    element_ids(&values.traversal.elements(), handles)
                }
            };
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
                        let Some(changed) = changed else {
                            return fall_back_document("an attribute the build reads");
                        };
                        return self.repaint_plan(batch, changed, theme_css);
                    }
                    _ => return fall_back_document("an element outside an open root"),
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
                _ => {
                    crate::why::note("plan_text.refused", || match id {
                        None => "the edited text has no source id".into(),
                        Some(_) if !open(&change.node, &root) => {
                            "the edited text is outside an open root".into()
                        }
                        Some(_) => super::text::not_editable_why(change, &read).into(),
                    });
                    return why::refuse(&why::REFUSED_PLAN_TEXT);
                }
            }
        }
        // Пути к правкам ищутся в текущем дереве; без единого пути дерево
        // коробок после пакета то же самое, и копировать нечего.
        let attrs = match targets.is_empty() {
            true => Vec::new(),
            false => match self
                .boxes()
                .and_then(|b| attr_paths(&self.nodes, &b, &targets))
            {
                Some(attrs) => attrs,
                None => return fall_back_document("attributes rewritten by the build"),
            },
        };
        let found = why::why(text_paths(&self.nodes, &texts), &why::REFUSED_PLAN_TEXT)?;
        if attrs.is_empty() && found.is_empty() {
            return Some(Plan::Clean(None));
        }
        let mut nodes = (*self.nodes).clone();
        if !(patch(&mut nodes, &attrs, &targets) && patch_texts(&mut nodes, found, &texts)) {
            return fall_back_document("a box path");
        }
        // Цели атрибутов — только `class` и `data-*` (`neutral`): их читают
        // лишь селекторы и `attr()` каскада, а не сборка кадра.
        if !texts.is_empty() {
            self.forget_has();
        }
        Some(if texts.is_empty() {
            Plan::Clean(Some(Edited { nodes, attrs }))
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
    find_sources(list, handles, |_, ordinal| ordinal)
}

/// Ключи исходных элементов, без зависимости от позиции адреса в списке.
pub(super) fn element_ids<'a>(
    list: &[usize],
    handles: impl Iterator<Item = &'a Handle>,
) -> Vec<Option<u64>> {
    find_sources(list, handles, |handle, _| handle.key().packed())
}

fn find_sources<'a>(
    list: &[usize],
    handles: impl Iterator<Item = &'a Handle>,
    id: impl Fn(&Handle, u64) -> u64,
) -> Vec<Option<u64>> {
    let wanted: Vec<_> = handles.collect();
    let mut found: HashMap<usize, Option<u64>> = wanted
        .iter()
        .map(|handle| (Rc::as_ptr(handle) as usize, None))
        .collect();
    let mut seen = std::collections::HashSet::with_capacity(wanted.len());
    for (slot, address) in list.iter().enumerate() {
        if let Some(id) = found.get_mut(address) {
            // Второе появление адреса делает узел неадресуемым.
            *id = seen.insert(*address).then_some(slot as u64 + 1);
        }
    }
    wanted
        .iter()
        .map(|handle| found[&(Rc::as_ptr(handle) as usize)].map(|ordinal| id(handle, ordinal)))
        .collect()
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
fn neutral(change: &AttrChange, sheets: &Mentions) -> bool {
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
            // Compared as selectors match them (`dom::class_tokens`): `A` -> `a`
            // changes the class outside quirks mode.
            classes.extend(crate::dom::class_tokens::changed(
                was.as_deref(),
                is.as_deref(),
            ));
        }
        names.push(local.to_ascii_lowercase());
    }
    let own = value(
        new,
        &html5ever::QualName::new(None, html5ever::ns!(), "style".into()),
    );
    !sheets.any(&names, &classes) && !own.is_some_and(|css| mentions(&css, &names, &classes))
}
