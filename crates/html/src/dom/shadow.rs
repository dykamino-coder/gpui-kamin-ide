//! Теневое дерево: декларативный shadow root, слоты, развёртка.

use crate::dom::*;
use crate::style::css::{Keyframes, Media, Rule, parse_stylesheet_media};
use crate::style::select::has::{HasArg, collect_has_args, mark_has, parse_has_arg};
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
    pub(crate) static SHADOWS: std::cell::RefCell<HashMap<usize, Rc<Shadow>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Слоты теней документа: адрес слота → распределение.
    pub(crate) static SLOTS: std::cell::RefCell<HashMap<usize, Rc<SlotInfo>>> =
        std::cell::RefCell::new(HashMap::new());
}

pub(crate) fn node_key(handle: &Handle) -> usize {
    Rc::as_ptr(handle) as usize
}

pub(crate) fn shadow_of(handle: &Handle) -> Option<Rc<Shadow>> {
    SHADOWS.with(|m| m.borrow().get(&node_key(handle)).cloned())
}

pub(crate) fn slot_of(handle: &Handle) -> Option<Rc<SlotInfo>> {
    SLOTS.with(|m| m.borrow().get(&node_key(handle)).cloned())
}

pub(crate) fn is_slot(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if local_name(&name.local) == "slot")
}

pub(crate) fn attr_of(handle: &Handle, key: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &handle.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|a| &*a.name.local == key)
        .map(|a| a.value.to_string())
}

/// Шаблон объявленной тени среди детей хоста: ПЕРВЫЙ `<template
/// shadowrootmode="open|closed">` (HTML §13.2.6.4.4: второй такой шаблон к
/// хосту не крепится и остаётся обычным `<template>`). Возвращает сам
/// шаблон (его надо вычесть из светлых детей) и содержимое — корень тени.
pub(crate) fn declarative_shadow(host: &Handle) -> Option<(Handle, Handle)> {
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
pub(crate) fn shadow_scope(root: &Handle, agent: &Scope, media: Media) -> (Rc<Scope>, Vec<HasArg>) {
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
pub(crate) fn collect_slots(handle: &Handle, out: &mut Vec<Handle>) {
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
pub(crate) fn assign_slots(
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

/// Предпроход по теням в порядке дерева. На хосте: область стилей тени,
/// отметки `:has` по тени, распределение слотов; дальше — в тень с цепочкой
/// из одного безликого хоста (css-shadow-1 §3.1: «the selector match list is
/// initially the shadow host, followed by all children of the shadow root»)
/// и в светлых детей — в текущей области.
pub(crate) fn scan_shadows(
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
            scan_shadows(&shadow_kids, &mut vec![marker], &inner, agent, media, drafts);
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

/// Плоские распределённые слота (DOM «find flattened slottables»):
/// распределённые, иначе fallback-дети; слот среди них раскрывается
/// рекурсивно. Глубина ограничена: цикла распределений в дереве быть не
/// может, но стража дешевле доказательства.
pub(crate) fn flatten_slot(
    key: usize,
    drafts: &HashMap<usize, SlotInfo>,
    depth: usize,
    out: &mut Vec<Option<FlatCtx>>,
) {
    let Some(info) = drafts.get(&key) else { return };
    if depth > 32 {
        return;
    }
    if !info.assigned.is_empty() {
        for s in &info.assigned {
            if is_slot(&s.node) && drafts.contains_key(&node_key(&s.node)) {
                flatten_slot(node_key(&s.node), drafts, depth + 1, out);
            } else {
                out.push(s.anc.clone().map(|anc| FlatCtx {
                    anc,
                    path: info.host_path.clone(),
                    all: info.light_all.clone(),
                    pos: s.elem_pos,
                }));
            }
        }
        return;
    }
    let kids: Vec<Handle> = info.slot.children.borrow().clone();
    let (_, all) = census_of(&kids);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for kid in &kids {
        match &kid.data {
            NodeData::Element { .. } => {
                if is_slot(kid) && drafts.contains_key(&node_key(kid)) {
                    flatten_slot(node_key(kid), drafts, depth + 1, out);
                } else {
                    out.push(Some(FlatCtx {
                        anc: all[pos].clone(),
                        path: Rc::new(vec![]),
                        all: all.clone(),
                        pos,
                    }));
                }
                pos += 1;
            }
            NodeData::Text { .. } => out.push(None),
            _ => {}
        }
    }
}

/// Досчитать плоские списки и выложить слоты на склад.
pub(crate) fn finish_slots(mut drafts: HashMap<usize, SlotInfo>) {
    let keys: Vec<usize> = drafts.keys().copied().collect();
    let mut flats: HashMap<usize, Vec<Option<FlatCtx>>> = HashMap::new();
    for key in &keys {
        let mut flat = vec![];
        flatten_slot(*key, &drafts, 0, &mut flat);
        flats.insert(*key, flat);
    }
    SLOTS.with(|m| {
        let mut m = m.borrow_mut();
        for key in keys {
            if let Some(mut info) = drafts.remove(&key) {
                info.flattened = flats.remove(&key).unwrap_or_default();
                m.insert(key, Rc::new(info));
            }
        }
    });
}
