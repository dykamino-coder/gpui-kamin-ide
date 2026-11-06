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

use super::patch::{Path, apply};
use super::shape::Edited;
use crate::dom::{Node, TextId};
use std::collections::{HashMap, HashSet};

/// Прежнее и новое содержимое правленого исходного текста.
pub(super) struct TextEdit {
    pub(super) old: String,
    pub(super) new: String,
}

/// Не меняет ли правка `edit` ни одного каскадного значения: оба текста
/// непустые по CSS, и содержимое прежнего не читал ни один читатель сборки
/// (`read` — адреса прочитанных исходных текстов).
pub(super) fn editable(edit: &Edited, read: &HashSet<usize>) -> bool {
    use crate::dom::whitespace::blank_text;
    !blank_text(&edit.old) && !blank_text(&edit.new) && !read.contains(&edit.node)
}

/// Новые тексты в узлах дерева коробок с номерами правленых исходных
/// текстов. Куски и копии исходного текста несут его номер, поэтому правится
/// каждый такой узел. Узел с номером правки, но другим содержимым (сборка
/// разрезала или изменила текст), узел внутри псевдоэлемента и правка без
/// единого узла (текст склеен с соседом или выброшен) — `false`, дерево не
/// меняется.
pub(super) fn patch_texts(nodes: &mut Vec<Node>, edits: &HashMap<TextId, TextEdit>) -> bool {
    if edits.is_empty() {
        return true;
    }
    let mut found: Vec<(Path, TextId)> = Vec::new();
    if !collect(nodes, edits, &mut found) {
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

/// Пути к узлам текста с номерами `edits`. Кадр хранит продолжение списка
/// детей и признак псевдоэлемента; глубина не расходует стек вызовов.
fn collect(
    children: &[Node],
    edits: &HashMap<TextId, TextEdit>,
    found: &mut Vec<(Path, TextId)>,
) -> bool {
    let mut stack = vec![(children, 0, false)];
    let mut path = Vec::new();
    while let Some((children, next, generated)) = stack.last_mut() {
        let children: &[Node] = children;
        let Some(child) = children.get(*next) else {
            stack.pop();
            path.pop();
            continue;
        };
        path.push(*next);
        *next += 1;
        match child {
            Node::Text(text) => match text.source().and_then(|id| Some((id, edits.get(&id)?))) {
                Some((id, edit)) if !*generated && text.as_str() == edit.old => {
                    found.push((path.clone(), id));
                }
                Some(_) => return false,
                None => {}
            },
            Node::Element(e) => {
                let generated = *generated || e.tag.starts_with("::");
                stack.push((&e.children, 0, generated));
                continue;
            }
        }
        path.pop();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{Element, Marks, Text};
    use crate::style::computed::Computed;

    fn text(id: u32, value: &str) -> Node {
        Node::Text(Text::from_source(value.into(), TextId::new(id)))
    }

    fn element(tag: &str, children: Vec<Node>) -> Node {
        Node::Element(Element::synthetic(
            tag,
            Computed::default(),
            Marks::default(),
            children,
        ))
    }

    fn edits() -> HashMap<TextId, TextEdit> {
        HashMap::from([(
            TextId::new(1).unwrap(),
            TextEdit {
                old: "old".into(),
                new: "new".into(),
            },
        )])
    }

    #[test]
    fn deep_text_patch_uses_a_bounded_native_stack() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let mut nodes = vec![text(1, "old")];
                for _ in 0..20_000 {
                    nodes = vec![element("div", nodes)];
                }
                // Exercise copy-on-write paths, retaining the previous revision.
                let old = nodes.clone();
                assert!(patch_texts(&mut nodes, &edits()));
                let path = vec![0; 20_001];
                let Some(Node::Text(value)) = apply(&mut nodes, &path) else {
                    panic!("missing text")
                };
                assert_eq!(value.as_str(), "new");
                let mut found = Vec::new();
                assert!(collect(&old, &edits(), &mut found));
                assert_eq!(found.len(), 1);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn text_patch_preserves_sibling_paths_and_generated_scope() {
        let mut nodes = vec![
            element("div", vec![text(2, "untouched"), text(1, "old")]),
            element(
                "::before",
                vec![element("span", vec![text(2, "generated")])],
            ),
            element("span", vec![text(1, "old")]),
        ];
        assert!(patch_texts(&mut nodes, &edits()));
        for path in [&[0, 1][..], &[2, 0][..]] {
            let Some(Node::Text(value)) = apply(&mut nodes, path) else {
                panic!("missing text")
            };
            assert_eq!(value.as_str(), "new");
        }
    }

    #[test]
    fn rejected_text_patch_is_atomic() {
        for rejected in [text(1, "split"), element("::after", vec![text(1, "old")])] {
            let mut nodes = vec![text(1, "old"), rejected];
            assert!(!patch_texts(&mut nodes, &edits()));
            let Node::Text(value) = &nodes[0] else {
                panic!("missing text")
            };
            assert_eq!(value.as_str(), "old");
        }
        assert!(!patch_texts(&mut vec![text(2, "old")], &edits()));
    }
}
