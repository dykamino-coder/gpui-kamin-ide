//! Теневое дерево: декларативный shadow root, слоты, развёртка.

mod assignment;
mod flatten;
use crate::dom::shadow::assignment::assign_slots;
use crate::dom::shadow::assignment::declarative_shadow;
use crate::dom::shadow::assignment::shadow_scope;
pub(super) use crate::dom::shadow::flatten::finish_slots;

use crate::dom::*;
use crate::style::css::{Keyframes, Media, Rule};
use crate::style::select::has::mark_has;
use crate::style::select::{Ancestor, Spot, census_of};
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
pub(super) struct Shadow {
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
    pub(crate) anc: Ancestor,
    pub(crate) path: Rc<Vec<Ancestor>>,
    pub(crate) all: Rc<Vec<Ancestor>>,
    pub(crate) pos: usize,
}

/// Слот дерева теней: что в него распределено и чем это стилизовать.
pub(crate) struct SlotInfo {
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

thread_local! {
    /// Тени документа: адрес хоста → тень.
    pub(super) static SHADOWS: std::cell::RefCell<HashMap<usize, Rc<Shadow>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Слоты теней документа: адрес слота → распределение.
    pub(super) static SLOTS: std::cell::RefCell<HashMap<usize, Rc<SlotInfo>>> =
        std::cell::RefCell::new(HashMap::new());
}

fn node_key(handle: &Handle) -> usize {
    Rc::as_ptr(handle) as usize
}

pub(super) fn shadow_of(handle: &Handle) -> Option<Rc<Shadow>> {
    SHADOWS.with(|m| m.borrow().get(&node_key(handle)).cloned())
}

pub(crate) fn slot_of(handle: &Handle) -> Option<Rc<SlotInfo>> {
    SLOTS.with(|m| m.borrow().get(&node_key(handle)).cloned())
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

/// Предпроход по теням в порядке дерева. На хосте: область стилей тени,
/// отметки `:has` по тени, распределение слотов; дальше — в тень с цепочкой
/// из одного безликого хоста (css-shadow-1 §3.1: «the selector match list is
/// initially the shadow host, followed by all children of the shadow root»)
/// и в светлых детей — в текущей области.
pub(super) fn scan_shadows(
    children: &[Handle],
    path: &mut Vec<Ancestor>,
    scope: &Rc<Scope>,
    agent: &Scope,
    media: Media,
    drafts: &mut HashMap<usize, SlotInfo>,
) {
    let (_, all) = census_of(children);
    let mut pos = 0usize;
    for child in children {
        if !matches!(&child.data, NodeData::Element { .. }) {
            continue;
        }
        let me = all[pos].clone();
        pos += 1;
        if let Some((template, root)) = declarative_shadow(child) {
            let (inner, args) = shadow_scope(&root, agent, media);
            if !args.is_empty() {
                mark_has(&root, &args, &mut vec![]);
            }
            let host_path: Rc<Vec<Ancestor>> = Rc::new(
                path.iter()
                    .cloned()
                    .chain(std::iter::once(me.clone()))
                    .collect(),
            );
            let light: Vec<Handle> = child
                .children
                .borrow()
                .iter()
                .filter(|c| !Rc::ptr_eq(c, &template))
                .cloned()
                .collect();
            assign_slots(&root, &light, scope, &host_path, drafts);
            let marker = Ancestor {
                featureless: Some(Rc::new(path.clone())),
                ..me.clone()
            };
            SHADOWS.with(|m| {
                m.borrow_mut().insert(
                    node_key(child),
                    Rc::new(Shadow {
                        root: root.clone(),
                        scope: inner.clone(),
                        marker: marker.clone(),
                    }),
                )
            });
            let shadow_kids: Vec<Handle> = root.children.borrow().clone();
            scan_shadows(
                &shadow_kids,
                &mut vec![marker],
                &inner,
                agent,
                media,
                drafts,
            );
            path.push(me);
            scan_shadows(&light, path, scope, agent, media, drafts);
            path.pop();
        } else {
            let kids: Vec<Handle> = child.children.borrow().clone();
            path.push(me);
            scan_shadows(&kids, path, scope, agent, media, drafts);
            path.pop();
        }
    }
}
