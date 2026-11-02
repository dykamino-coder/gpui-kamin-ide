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
//! одном, у элемента с потоком текста. Тогда в дереве коробок меняются узлы,
//! построенные из этого исходного текста (`dom::Text::source`), и только они;
//! текст, склеенный обходом с соседним, номера не несёт и правкой не находится.

use super::patch::{Path, apply};
use super::shape::Edited;
use crate::dom::{Node, TextId};
use markup5ever_rcdom::NodeData;
use std::collections::{HashMap, HashSet};

/// Прежнее и новое содержимое правленого исходного текста.
pub(super) struct TextEdit {
    pub(super) old: String,
    pub(super) new: String,
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

/// Не меняет ли правка `edit` ни одного каскадного значения.
pub(super) fn editable(edit: &Edited) -> bool {
    let NodeData::Element { name, attrs, .. } = &edit.parent.data else {
        return false;
    };
    if name.ns != html5ever::ns!(html) || OPAQUE.contains(&&*name.local) {
        return false;
    }
    if attrs.borrow().iter().any(|a| &*a.name.local == "dir") {
        return false;
    }
    matches!((shape(&edit.old), shape(&edit.new)), (Some(old), Some(new)) if old == new)
}

/// Признаки текста, которые видит каскад: пробельные края и наличие сильного
/// знака слева направо; `None` — текст пустой по CSS или с письмом справа
/// налево.
fn shape(text: &str) -> Option<(&str, &str, bool)> {
    use crate::dom::whitespace::collapsible;
    use unicode_bidi::BidiClass::*;
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

/// Новые тексты в узлах дерева коробок с номерами правленых исходных
/// текстов. Куски и копии исходного текста несут его номер, поэтому правится
/// каждый такой узел. Узел с номером правки, но другим содержимым (сборка
/// разрезала или изменила текст), узел под первой буквой или в
/// псевдоэлементе и правка без единого узла (текст склеен с соседом или
/// выброшен) — `false`, дерево не меняется.
pub(super) fn patch_texts(nodes: &mut Vec<Node>, edits: &HashMap<TextId, TextEdit>) -> bool {
    if edits.is_empty() {
        return true;
    }
    let mut found: Vec<(Path, TextId)> = Vec::new();
    if !collect(nodes, edits, false, &mut Vec::new(), &mut found) {
        return false;
    }
    let seen: HashSet<TextId> = found.iter().map(|(_, id)| *id).collect();
    if seen.len() != edits.len() {
        return false;
    }
    for (path, id) in found {
        match apply(nodes, &path) {
            Some(Node::Text(text)) => edits[&id].new.clone_into(&mut **text),
            _ => return false,
        }
    }
    true
}

/// Пути к узлам текста с номерами `edits` среди `children`; `lettered` —
/// текст здесь режет первая буква блока или он внутри псевдоэлемента.
fn collect(
    children: &[Node],
    edits: &HashMap<TextId, TextEdit>,
    lettered: bool,
    path: &mut Vec<usize>,
    found: &mut Vec<(Path, TextId)>,
) -> bool {
    for (i, child) in children.iter().enumerate() {
        path.push(i);
        let ok = match child {
            Node::Text(text) => match text.source().and_then(|id| Some((id, edits.get(&id)?))) {
                Some((id, edit)) if !lettered && text.as_str() == edit.old => {
                    found.push((path.clone(), id));
                    true
                }
                Some(_) => false,
                None => true,
            },
            Node::Element(e) => {
                let pseudo = e.tag.starts_with("::");
                let synthetic = e.node_id == 0 && !pseudo;
                let lettered = pseudo || e.first_letter.is_some() || (synthetic && lettered);
                collect(&e.children, edits, lettered, path, found)
            }
        };
        path.pop();
        if !ok {
            return false;
        }
    }
    true
}
