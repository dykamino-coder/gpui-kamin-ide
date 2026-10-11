//! Tree parsing for tests; split out to keep the owning module within 250 lines.

use super::*;

/// Цвета детей по порядку — короткая запись для проверок каскада.
pub(super) fn child_colors(html: &str) -> Vec<Option<crate::style::values::value::Color>> {
    fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.attr("id") == Some(id) {
                    return Some(e);
                }
                if let Some(found) = find(&e.children, id) {
                    return Some(found);
                }
            }
        }
        None
    }
    let nodes = parse(html, "");
    find(&nodes, "box")
        .map(|e| {
            e.children
                .iter()
                .filter_map(|n| match n {
                    Node::Element(c) => Some(c.style.color),
                    Node::Text(_) => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Тексты псевдоэлементов документа в порядке обхода.
pub(super) fn pseudo_texts(html: &str) -> Vec<String> {
    fn walk(nodes: &[Node], out: &mut Vec<String>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag.starts_with("::") {
                    let text = e.children.iter().find_map(|c| match c {
                        Node::Text(t) => Some(t.clone()),
                        _ => None,
                    });
                    out.push(text.unwrap_or_default());
                }
                walk(&e.children, out);
            }
        }
    }
    let mut out = vec![];
    walk(&parse(html, ""), &mut out);
    out
}

#[test]
pub(super) fn list_item_counter_is_implicit() {
    // Пункт списка двигает `list-item` сам; `start` задаёт начало,
    // `value` — номер конкретного пункта (css-lists-3 §ua-stylesheet).
    let texts = pseudo_texts(
        "<style>li::after { content: counter(list-item) }</style>             <ol start=\"5\"><li></li><li value=\"9\"></li><li></li></ol>",
    );
    assert_eq!(texts, vec!["5", "9", "10"]);
    // Явное упоминание `list-item` отменяет неявное увеличение.
    let texts = pseudo_texts(
        "<style>li { counter-increment: list-item 3 } li::after { content: counter(list-item) }</style>             <ol><li></li><li></li></ol>",
    );
    assert_eq!(texts, vec!["3", "6"]);
}

#[test]
pub(super) fn has_relational_pseudo() {
    let red = crate::style::values::value::Color::parse("red");
    let green = crate::style::values::value::Color::parse("green");
    // Предметная позиция: якорь с потомком-предметом.
    let colors = child_colors(
        "<style>div { color: red } div:has(span) { color: green }</style>             <div id=\"box\"><div><span></span></div><div><b></b></div></div>",
    );
    assert_eq!(colors, vec![green, red]);
    // Ведущий `>`: только прямой ребёнок; `+`: следующий брат.
    let colors = child_colors(
        "<style>p { color: red } p:has(> em) { color: green }             p:has(+ p) { background: yellow }</style>             <div id=\"box\"><p><i><em>x</em></i></p><p><em>y</em></p></div>",
    );
    assert_eq!(colors, vec![red, green]);
    // Непредметная позиция: `div:has(.x) b` красит b только в div с .x.
    let colors = child_colors(
        "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i class=\"x\"></i><b></b></div></div>",
    );
    assert_eq!(colors, vec![None, green]);
    let colors = child_colors(
        "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i></i><b></b></div></div>",
    );
    assert_eq!(colors, vec![None, red]);
}

#[test]
pub(super) fn nth_child_of_selector_list() {
    let red = crate::style::values::value::Color::parse("red");
    let green = crate::style::values::value::Color::parse("green");
    // Индекс считается среди совпавших с S братьев, а не среди всех:
    // второй `.a` — это :nth-child(2 of .a), хотя среди детей он третий.
    let colors = child_colors(
        "<style>p { color: red } p:nth-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
    );
    assert_eq!(colors, vec![red, red, green]);
    // nth-last-child(of S): совпавшие считаются с конца.
    let colors = child_colors(
        "<style>p { color: red } p:nth-last-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
    );
    assert_eq!(colors, vec![green, red, red]);
}

#[test]
pub(super) fn custom_properties_cascade_and_inherit() {
    let red = crate::style::values::value::Color::parse("red");
    let blue = crate::style::values::value::Color::parse("blue");
    // Переключение темы классом: у потомка внутри `.dark` своё значение
    // переменной, у остальных — корневое. Пока переменные собирались в
    // один плоский словарь на документ, последнее объявление красило ВЕСЬ
    // документ, и тема классом не переключалась в принципе.
    let colors = child_colors(
        "<style>:root { --c: red } .dark { --c: blue } i { color: var(--c) }</style>             <div id=box><i></i><i class=dark></i></div>",
    );
    assert_eq!(colors, vec![red, blue], "получено {colors:?}");
}

#[test]
pub(super) fn nth_child_selects_by_position() {
    let red = crate::style::values::value::Color::parse("red");
    let colors = child_colors(
        "<style>i:nth-child(2) { color: red }</style>\
             <div id=box><i></i><i></i><i></i></div>",
    );
    assert_eq!(colors, vec![None, red, None], "получено {colors:?}");
}

#[test]
pub(super) fn nth_child_understands_an_plus_b() {
    let red = crate::style::values::value::Color::parse("red");
    let colors = child_colors(
        "<style>i:nth-child(2n+1) { color: red }</style>\
             <div id=box><i></i><i></i><i></i><i></i></div>",
    );
    assert_eq!(colors, vec![red, None, red, None], "получено {colors:?}");
}

#[test]
pub(super) fn last_child_counts_from_the_end() {
    let red = crate::style::values::value::Color::parse("red");
    let colors = child_colors(
        "<style>i:last-child { color: red }</style>\
             <div id=box><i></i><i></i></div>",
    );
    assert_eq!(colors, vec![None, red], "получено {colors:?}");
}

#[test]
pub(super) fn of_type_counts_only_the_same_tag() {
    let red = crate::style::values::value::Color::parse("red");
    // Второй `<i>` — четвёртый ребёнок, но второй своего тега.
    let colors = child_colors(
        "<style>i:nth-of-type(2) { color: red }</style>\
             <div id=box><b></b><i></i><b></b><i></i></div>",
    );
    assert_eq!(colors, vec![None, None, None, red], "получено {colors:?}");
}

pub(super) fn first_element(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "body" || e.tag == "html" {
                    if let Some(inner) = find(&e.children) {
                        return Some(inner);
                    }
                    continue;
                }
                return Some(e);
            }
        }
        None
    }
    find(nodes).expect("нет элементов")
}
