//! Теневое дерево: декларативный shadow root, слоты, развёртка.

mod assignment;
mod flatten;
mod prepare;
pub(crate) mod slotted;
use crate::dom::shadow::assignment::assign_slots;
pub(super) use crate::dom::shadow::assignment::declarative_shadow;
pub(super) use crate::dom::shadow::flatten::finish_slots;
pub(super) use prepare::prepare;

use crate::dom::*;
use crate::style::css::{Keyframes, Rule};
use crate::style::select::{Ancestor, Spot};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Таблицы одной области дерева — документа или тени: правила и кадры.
pub(crate) struct Scope {
    pub(crate) rules: Vec<Rule>,
    pub(crate) frames: HashMap<String, Keyframes>,
}

/// Дерево теней хоста (HTML §4.12.3, `<template shadowrootmode>`).
///
/// html5ever тень к хосту не крепит (`attach_declarative_shadow` у `RcDom`
/// возвращает false) и по HTML §13.2.6.4.4 шаг 8.1.1 оставляет обычный
/// `<template>` ребёнком хоста, а разметку тени — в его `template_contents`.
/// Здесь это и есть корень тени.
pub(crate) struct Shadow {
    pub(crate) root: Handle,
    /// Таблицы тени: лист агента + `<style>` тени. Правила документа сюда
    /// не попадают, правила тени — наружу (css-shadow-1 §3.2).
    pub(crate) scope: Rc<Scope>,
    /// Паспорт хоста глазами тени — безликий, с цепочкой светлых предков.
    pub(crate) marker: Ancestor,
}

/// Узел, распределённый в слот, и его место среди СВЕТЛЫХ детей хоста.
pub(crate) struct Slotted {
    pub(crate) node: Handle,
    /// Номер в списке светлых детей (без шаблона тени).
    pub(crate) light_idx: usize,
    /// Сколько элементов стоит ДО узла; для элемента `light_all[elem_pos]` —
    /// он сам (соглашение `Sibs`).
    pub(crate) elem_pos: usize,
    /// Паспорт элемента; None — текст.
    pub(crate) anc: Option<Ancestor>,
}

/// Плоский распределённый элемент со светлым контекстом сопоставления —
/// для аргумента `:has-slotted(S)`.
pub(crate) struct FlatCtx {
    pub(crate) node: Handle,
    pub(crate) anc: Ancestor,
    pub(crate) path: Rc<Vec<Ancestor>>,
    pub(crate) all: Rc<Vec<Ancestor>>,
    pub(crate) pos: usize,
}

/// Слот дерева теней: что в него распределено и чем это стилизовать.
pub(crate) struct SlotInfo {
    origin: Option<slotted::SlotContext>,
    pub(crate) slot: Handle,
    /// Распределённые (DOM §4.2.2.4 «find slottables»); пусто — рисуется
    /// fallback, то есть собственные дети слота.
    pub(crate) assigned: Vec<Slotted>,
    /// Область ХОСТА: распределённые дети стилизуются её таблицами.
    pub(crate) outer: Rc<Scope>,
    /// Цепочка предков распределённого: предки хоста + сам хост.
    pub(crate) host_path: Rc<Vec<Ancestor>>,
    /// Светлые дети хоста без шаблона тени, их места и паспорта.
    pub(crate) light: Vec<Handle>,
    pub(crate) light_spots: Vec<Spot>,
    pub(crate) light_all: Rc<Vec<Ancestor>>,
    /// Плоские распределённые (DOM «find flattened slottables»): текст —
    /// None, но в счёте участвует (`has-slotted-001` зелёная от пробелов).
    pub(crate) flattened: Vec<Option<FlatCtx>>,
}

fn is_slot(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if local_name(&name.local) == "slot")
}

fn attr_of(handle: &Handle, key: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &handle.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|a| &*a.name.local == key)
        .map(|a| a.value.to_string())
}

/// Наличие шаблона тени проверяется после нормализации имён HTML-парсером.
/// Вложенные шаблоны также учитываются; XML не получает HTML-атрибуты.
pub(super) fn has_declarative_shadow(
    root: &Handle,
    admission: &crate::budget::admission::Admission,
) -> Result<bool, crate::budget::admission::Error> {
    use crate::dom::source_walk::{SourceWalk, Templates};
    let mut walk = SourceWalk::new(root, Templates::All, admission.clone())?;
    while let Some(node) = walk.try_next()? {
        if let NodeData::Element {
            attrs,
            template_contents,
            ..
        } = &node.data
            && template_contents.borrow().is_some()
        {
            admission.work(attrs.borrow().len())?;
            if attrs
                .borrow()
                .iter()
                .any(|a| a.name.ns.is_empty() && &*a.name.local == "shadowrootmode")
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

mod walk;
pub(super) use walk::scan_shadows;
