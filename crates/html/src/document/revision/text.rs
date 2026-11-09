//! Правка текста без перерасчёта стиля.
//!
//! Сборка дерева коробок читает содержимое текста двумя способами. Почти все
//! читатели спрашивают лишь, пустой ли узел по CSS (только схлопываемые
//! пробелы, CSS Text 3 §4.1.1): обход (пустые узлы выбрасываются, пробельный
//! узел между строчными становится пробелом), `:empty` (Selectors 4 §14.2;
//! узел из пробелов делает элемент непустым, а пустой узел HTML-разбор не
//! создаёт), анонимные ячейки таблиц (CSS 2.1 §17.2.1), руби (css-ruby-1
//! §2.2), run-in, `filter: url()` без содержимого, `float_tail`. Ответ этих
//! читателей не меняется, если прежний и новый текст оба непустые: так Blink
//! пересчитывает `:empty` (`AffectedByEmpty`) лишь при смене пустоты.
//!
//! Остальные читатели берут само содержимое и отмечают прочитанные тексты в
//! `TraversalState` (`read_texts`): первый сильный знак для `dir=auto` и
//! `<bdi>` (HTML §3.2.6.4, §4.5.24; тексты до найденного знака) и листы
//! `<style>`. Правка отмеченного текста может изменить стиль; правка
//! неотмеченного — нет, при любом письме, цифрах и управляющих bidi:
//! направление строк решает раскладка, которая идёт заново. Тогда в дереве
//! коробок меняются узлы, построенные из этого исходного текста
//! (`dom::Text::source`), и только они; текст, склеенный обходом с соседним,
//! номера не несёт и правкой не находится.

use super::mutation::TextChange;
use super::patch::{Path, apply};
use crate::dom::{Node, TextId};
use std::collections::{HashMap, HashSet};

/// Правки текстов по номерам исходных текстов.
pub(super) type TextEdits<'a> = HashMap<TextId, &'a TextChange>;

/// Не меняет ли правка `edit` ни одного каскадного значения: оба текста
/// непустые по CSS, и содержимое прежнего не читал ни один читатель сборки
/// (`read` — адреса прочитанных исходных текстов).
pub(super) fn editable(edit: &TextChange, read: &HashSet<usize>) -> bool {
    use crate::dom::whitespace::blank_text;
    !blank_text(&edit.old)
        && !blank_text(&edit.new)
        && !read.contains(&(std::rc::Rc::as_ptr(&edit.node) as usize))
}

/// Пути к узлам дерева коробок с номерами правленых исходных текстов.
/// Куски и копии исходного текста несут его номер, поэтому правится
/// каждый такой узел. Узел с номером правки, но другим содержимым (сборка
/// разрезала или изменила текст), узел внутри псевдоэлемента и правка без
/// единого узла (текст склеен с соседом или выброшен) — `None`. Без правок
/// дерево не обходится.
pub(super) fn text_paths(nodes: &[Node], edits: &TextEdits<'_>) -> Option<Vec<(Path, TextId)>> {
    let mut found: Vec<(Path, TextId)> = Vec::new();
    if edits.is_empty() {
        return Some(found);
    }
    if !collect(nodes, edits, false, &mut Vec::new(), &mut found) {
        return None;
    }
    let seen: HashSet<TextId> = found.iter().map(|(_, id)| *id).collect();
    (seen.len() == edits.len()).then_some(found)
}

/// Записать новые тексты по путям `text_paths` (копия только путей).
pub(super) fn patch_texts(
    nodes: &mut Vec<Node>,
    found: Vec<(Path, TextId)>,
    edits: &TextEdits<'_>,
) -> bool {
    for (path, id) in found {
        match apply(nodes, &path) {
            Some(Node::Text(text)) => edits[&id].new.clone_into(&mut **text),
            _ => return false,
        }
    }
    true
}

/// Пути к узлам текста с номерами `edits` среди `children`; `generated` —
/// текст внутри псевдоэлемента.
fn collect(
    children: &[Node],
    edits: &TextEdits<'_>,
    generated: bool,
    path: &mut Vec<usize>,
    found: &mut Vec<(Path, TextId)>,
) -> bool {
    for (i, child) in children.iter().enumerate() {
        path.push(i);
        let ok = match child {
            Node::Text(text) => match text.source().and_then(|id| Some((id, edits.get(&id)?))) {
                Some((id, edit)) if !generated && text.as_str() == edit.old => {
                    found.push((path.clone(), id));
                    true
                }
                Some(_) => false,
                None => true,
            },
            Node::Element(e) => {
                let generated = generated || e.tag.starts_with("::");
                collect(&e.children, edits, generated, path, found)
            }
        };
        path.pop();
        if !ok {
            return false;
        }
    }
    true
}
