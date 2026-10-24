//! Теневое дерево: декларативный shadow root, слоты, развёртка.

mod assignment;
mod flatten;
mod prepare;
pub(crate) mod slotted;
use crate::dom::shadow::assignment::assign_slots;
pub(super) use crate::dom::shadow::assignment::declarative_shadow;
pub(super) use crate::dom::shadow::flatten::finish_slots;
pub(super) use prepare::prepare;

use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::dom::*;
use crate::style::css::{Keyframes, Rule};
use crate::style::select::has::{HasArg, shadow_marks};
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
pub(super) fn has_declarative_shadow(root: &Handle) -> bool {
    let mut pending = vec![root.clone()];
    while let Some(node) = pending.pop() {
        if let NodeData::Element {
            attrs,
            template_contents,
            ..
        } = &node.data
            && let Some(contents) = template_contents.borrow().as_ref()
        {
            if attrs
                .borrow()
                .iter()
                .any(|a| a.name.ns.is_empty() && &*a.name.local == "shadowrootmode")
            {
                return true;
            }
            pending.push(contents.clone());
        }
        pending.extend(node.children.borrow().iter().cloned());
    }
    false
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
    prepared: &prepare::Prepared,
    args: &[HasArg],
    drafts: &mut HashMap<NodeKey, SlotInfo>,
    tree: &mut TreeIndex,
) {
    if !children
        .iter()
        .any(|node| matches!(&node.data, NodeData::Element { .. }))
    {
        return;
    }
    let doc = tree.doc.clone();
    let Some(_guard) = doc
        .traversal
        .enter(crate::document::TraversalKind::ShadowTree)
    else {
        return;
    };
    let (_, all) = census_of(children, tree);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for child in children {
        if !matches!(&child.data, NodeData::Element { .. }) {
            continue;
        }
        let mut me = all[pos].clone();
        me.peers = Some((all.clone(), pos));
        if let Some(slot) = drafts.get_mut(&NodeKey::of(child)) {
            slot.origin = Some(slotted::SlotContext {
                scope: scope.clone(),
                node: me.clone(),
                path: path.clone(),
                peers: all.clone(),
                pos,
            });
        }
        pos += 1;
        if let Some((template, root)) = declarative_shadow(child) {
            let inner = prepared[&NodeKey::of(child)].1.clone();
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
            assign_slots(&root, &light, scope, &host_path, drafts, tree);
            let marker = Ancestor {
                featureless: Some(Rc::new(path.clone())),
                shadow_has_marks: shadow_marks(&root, args, &me, tree),
                ..me.clone()
            };
            tree.shadows.insert(
                NodeKey::of(child),
                Rc::new(Shadow {
                    root: root.clone(),
                    scope: inner.clone(),
                    marker: marker.clone(),
                }),
            );
            let shadow_kids: Vec<Handle> = root.children.borrow().clone();
            scan_shadows(
                &shadow_kids,
                &mut vec![marker],
                &inner,
                prepared,
                args,
                drafts,
                tree,
            );
            path.push(me);
            scan_shadows(&light, path, scope, prepared, args, drafts, tree);
            path.pop();
        } else {
            let kids: Vec<Handle> = child.children.borrow().clone();
            path.push(me);
            scan_shadows(&kids, path, scope, prepared, args, drafts, tree);
            path.pop();
        }
    }
}
