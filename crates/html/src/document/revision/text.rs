//! Правка текста без перерасчёта стиля.
//!
//! Каскад от содержимого текста зависит в немногих местах: `:empty` (есть ли
//! текст), схлопывание пробельных узлов при обходе (узел из одних
//! схлопываемых пробелов, края узла), склейка соседних текстов, первая буква
//! блока (`::first-letter` режет текст), первый сильный знак для `dir=auto`
//! (HTML §3.2.6.4) и элементы, чьё содержимое — не поток текста (листы,
//! скрипты, поля форм, заголовок документа). Правка годится, когда ни одно из
//! них не может отличить прежний текст от нового: оба непустые не из одних
//! пробелов, с одинаковыми пробельными краями, без знаков письма справа
//! налево и управляющих bidi, с сильным знаком слева направо в обоих или ни в
//! одном, не склеенные с соседним текстом, у элемента с потоком текста. Тогда
//! в дереве коробок меняется ровно один узел текста в коробках родителя.

use super::patch::{Path, apply};
use super::shape::Edited;
use crate::dom::Node;
use markup5ever_rcdom::NodeData;
use std::collections::HashMap;

/// Правки текстов коробок одного исходного элемента.
pub(super) struct Texts {
    pub(super) tag: String,
    pub(super) edits: Vec<(String, String)>,
}

/// Элементы, текст которых читают не как поток строк (или читают проходы
/// сборки сами): листы, скрипты, поля форм, таблицы, ruby, заголовок, медиа.
const OPAQUE: &[&str] = &[
    "style",
    "script",
    "title",
    "textarea",
    "option",
    "optgroup",
    "select",
    "datalist",
    "noscript",
    "noembed",
    "noframes",
    "xmp",
    "iframe",
    "plaintext",
    "template",
    "head",
    "html",
    "table",
    "tbody",
    "thead",
    "tfoot",
    "tr",
    "colgroup",
    "col",
    "caption",
    "ruby",
    "rt",
    "rp",
    "rb",
    "rtc",
    "details",
    "summary",
    "button",
    "input",
    "meter",
    "progress",
    "legend",
    "fieldset",
    "canvas",
    "video",
    "audio",
    "object",
    "embed",
    "picture",
    "img",
    "frameset",
    "bdi",
    "bdo",
    "slot",
];

/// Тег элемента-родителя, если правка `edit` не меняет ни одного каскадного
/// значения.
pub(super) fn editable(edit: &Edited) -> Option<String> {
    let NodeData::Element { name, attrs, .. } = &edit.parent.data else {
        return None;
    };
    let tag = name.local.to_string();
    if name.ns != html5ever::ns!(html) || OPAQUE.contains(&tag.as_str()) || edit.merged {
        return None;
    }
    if attrs.borrow().iter().any(|a| &*a.name.local == "dir") {
        return None;
    }
    let (old, new) = (shape(&edit.old)?, shape(&edit.new)?);
    (old == new).then_some(tag)
}

/// Признаки текста, которые видит каскад: пробельные края и наличие сильного
/// знака слева направо; `None` — текст пустой по CSS или с письмом справа
/// налево.
fn shape(text: &str) -> Option<(&str, &str, bool)> {
    use unicode_bidi::BidiClass::*;
    let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
    let body = text.trim_matches(collapsible);
    if body.is_empty() {
        return None;
    }
    let mut strong = false;
    for c in body.chars() {
        match unicode_bidi::bidi_class(c) {
            R | AL | AN | RLE | RLO | LRE | LRO | PDF | LRI | RLI | FSI | PDI => return None,
            L => strong = true,
            _ => {}
        }
    }
    let start = &text[..text.len() - text.trim_start_matches(collapsible).len()];
    let end = &text[text.trim_end_matches(collapsible).len()..];
    Some((start, end, strong))
}

/// Новые тексты в коробках адресуемых элементов: у каждой коробки с номером
/// и тегом элемента ровно один узел текста с прежним содержимым среди её
/// синтетических потомков (анонимные обёртки); псевдоэлементы и коробки
/// других элементов не просматриваются. Иначе (текст разрезан первой буквой
/// или фрагментацией, коробки нет) — `false`.
pub(super) fn patch_texts(nodes: &mut Vec<Node>, targets: &HashMap<u64, Texts>) -> bool {
    let mut paths: Vec<(Path, String)> = Vec::new();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    let mut stack: Vec<(&[Node], usize)> = vec![(nodes, 0)];
    let mut path = Vec::new();
    while let Some((list, next)) = stack.last_mut() {
        let list: &[Node] = list;
        let Some(node) = list.get(*next) else {
            stack.pop();
            path.pop();
            continue;
        };
        let index = *next;
        *next += 1;
        let Node::Element(e) = node else { continue };
        path.push(index);
        if let Some(target) = targets.get(&e.node_id).filter(|t| t.tag == e.tag) {
            if e.first_letter.is_some() {
                return false;
            }
            for (old, new) in &target.edits {
                let mut found = Vec::new();
                texts(&e.children, old, &mut path.clone(), &mut found);
                let [one] = found.as_slice() else {
                    return false;
                };
                paths.push((one.clone(), new.clone()));
            }
            *seen.entry(e.node_id).or_default() += 1;
        }
        stack.push((&e.children, 0));
    }
    if targets.keys().any(|id| !seen.contains_key(id)) {
        return false;
    }
    for (path, new) in paths {
        match apply(nodes, &path) {
            Some(Node::Text(text)) => *text = new,
            _ => return false,
        }
    }
    true
}

/// Пути к узлам текста `old` среди детей коробки и её синтетических потомков.
fn texts(children: &[Node], old: &str, path: &mut Vec<usize>, found: &mut Vec<Path>) {
    for (i, child) in children.iter().enumerate() {
        path.push(i);
        match child {
            Node::Text(text) if text == old => found.push(path.clone()),
            Node::Element(e) if e.node_id == 0 && !e.tag.starts_with("::") => {
                if e.first_letter.is_some() {
                    found.push(path.clone());
                    found.push(path.clone());
                } else {
                    texts(&e.children, old, path, found);
                }
            }
            _ => {}
        }
        path.pop();
    }
}
