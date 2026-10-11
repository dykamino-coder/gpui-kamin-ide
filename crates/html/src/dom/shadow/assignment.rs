//! Assignment for shadow; split out to keep the owning module within 250 lines.

use super::{Scope, SlotInfo, Slotted, attr_of, is_slot, node_key};
use crate::dom::*;
use crate::style::css::{Media, Rule, parse_stylesheet_media};
use crate::style::select::has::{HasArg, collect_has_args, parse_has_arg};
use crate::style::select::{Ancestor, census_of};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Шаблон объявленной тени среди детей хоста: ПЕРВЫЙ `<template
/// shadowrootmode="open|closed">` (HTML §13.2.6.4.4: второй такой шаблон к
/// хосту не крепится и остаётся обычным `<template>`). Возвращает сам
/// шаблон (его надо вычесть из светлых детей) и содержимое — корень тени.
pub(super) fn declarative_shadow(host: &Handle) -> Option<(Handle, Handle)> {
    host.children.borrow().iter().find_map(|child| {
        let NodeData::Element {
            name,
            template_contents,
            ..
        } = &child.data
        else {
            return None;
        };
        if local_name(&name.local) != "template" {
            return None;
        }
        let mode = attr_of(child, "shadowrootmode")?.to_ascii_lowercase();
        if mode != "open" && mode != "closed" {
            return None;
        }
        let root = template_contents.borrow().clone()?;
        Some((child.clone(), root))
    })
}

/// Таблицы тени: копия листа агента + каждый `<style>` тени отдельной
/// таблицей происхождения документа. Возвращает область и аргументы её
/// `:has()` — отметки для них ставятся по дереву тени.
pub(super) fn shadow_scope(root: &Handle, agent: &Scope, media: Media) -> (Rc<Scope>, Vec<HasArg>) {
    let mut rules = agent.rules.clone();
    let mut frames = agent.frames.clone();
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(root, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_stylesheet_media(css, media).into_iter().enumerate() {
            rules.push(Rule {
                order: base + i,
                origin: 1,
                ..r
            });
        }
        frames.extend(crate::style::css::parse_keyframes_in(css, Some(media)));
    }
    let mut raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut raw));
    let args = raw.iter().filter_map(|a| parse_has_arg(a)).collect();
    (Rc::new(Scope { rules, frames }), args)
}

/// Все `<slot>` дерева тени в порядке дерева. Вложенные тени лежат в
/// `template_contents`, а не в `children`, поэтому сюда не попадают; светлые
/// дети вложенных хостов — попадают, они в этом же дереве.
pub(super) fn collect_slots(handle: &Handle, out: &mut Vec<Handle>) {
    for child in handle.children.borrow().iter() {
        if is_slot(child) {
            out.push(child.clone());
        }
        collect_slots(child, out);
    }
}

/// Распределение по слотам (DOM §4.2.2.4 «find a slot»): каждый светлый
/// ребёнок хоста — элемент или ТЕКСТ — уходит в первый слот тени с его
/// именем (`slot=""` элемента; у текста и без атрибута — пустое). Ребёнок без
/// подходящего слота не рисуется вовсе.
pub(super) fn assign_slots(
    root: &Handle,
    light: &[Handle],
    outer: &Rc<Scope>,
    host_path: &Rc<Vec<Ancestor>>,
    drafts: &mut HashMap<usize, SlotInfo>,
) {
    let mut slots: Vec<Handle> = vec![];
    collect_slots(root, &mut slots);
    if slots.is_empty() {
        return;
    }
    let (spots, all) = census_of(light);
    let light_all = Rc::new(all);
    let mut assigned: HashMap<usize, Vec<Slotted>> = HashMap::new();
    let mut pos = 0usize;
    for (idx, node) in light.iter().enumerate() {
        let (name, anc, elem_pos) = match &node.data {
            NodeData::Element { .. } => {
                let anc = light_all[pos].clone();
                pos += 1;
                let name = anc
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "slot")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                (name, Some(anc), pos - 1)
            }
            NodeData::Text { .. } => (String::new(), None, pos),
            _ => continue,
        };
        let Some(slot) = slots
            .iter()
            .find(|s| attr_of(s, "name").unwrap_or_default() == name)
        else {
            continue;
        };
        assigned.entry(node_key(slot)).or_default().push(Slotted {
            node: node.clone(),
            light_idx: idx,
            elem_pos,
            anc,
        });
    }
    for slot in slots {
        let key = node_key(&slot);
        drafts.insert(
            key,
            SlotInfo {
                slot,
                assigned: assigned.remove(&key).unwrap_or_default(),
                outer: outer.clone(),
                host_path: host_path.clone(),
                light: light.to_vec(),
                light_spots: spots.clone(),
                light_all: light_all.clone(),
                flattened: vec![],
            },
        );
    }
}
