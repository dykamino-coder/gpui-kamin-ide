//! Проверки контракта родительского модуля; вынесены для ограничения размера файлов.

use super::*;
use crate::dom::Element;
use crate::dom::parse;
use crate::layout::block::margins::collapse_margins;
use crate::style::values::value::Len;

fn find_class<'a>(nodes: &'a [Node], class: &str) -> Option<&'a Element> {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.attr("class")
                .is_some_and(|c| c.split_whitespace().any(|x| x == class))
            {
                return Some(e);
            }
            if let Some(found) = find_class(&e.children, class) {
                return Some(found);
            }
        }
    }
    None
}

/// Разворачивает обёртки документа до содержимого страницы.
fn page_children(html: &str) -> Vec<Node> {
    fn dive(n: &[Node]) -> Vec<Node> {
        match n.first() {
            Some(Node::Element(e)) if e.tag == "html" || e.tag == "body" => dive(&e.children),
            _ => n.to_vec(),
        }
    }
    dive(&parse(html, ""))
}

#[test]
fn margin_collapse_matches_the_browser_on_the_fixture_case() {
    // Ровно тот случай, на котором сравнение с Chrome показало сдвиг на
    // 10 точек: блок-обёртка без своего отступа сверху и ребёнок с ним.
    let page = page_children(
        "<div class=\"page\">\
               <div class=\"wrap\" style=\"margin: 0 0 10px\">w</div>\
               <div class=\"stack\" style=\"margin: 0 0 10px\">\
                 <div class=\"mt\" style=\"margin-top: 24px\">m</div>\
               </div>\
             </div>",
    );
    let children = match &page[0] {
        Node::Element(e) => collapse_margins(&e.children, false),
        _ => panic!("нет страницы"),
    };
    let stack = children
        .iter()
        .find_map(|n| match n {
            Node::Element(e) if e.attr("class") == Some("stack") => Some(e),
            _ => None,
        })
        .expect("нет обёртки");
    // Отступ ребёнка вынесен наружу (24) и уменьшен на уже отданные
    // предыдущим блоком 10 — суммарный зазор остаётся 24, как в браузере.
    assert_eq!(stack.style.margin.top, Some(Len::Px(14.0)), "у обёртки");
    let child_top = stack.children.iter().find_map(|n| match n {
        Node::Element(e) => Some(e.style.margin.top),
        _ => None,
    });
    assert_eq!(child_top, Some(Some(Len::Px(0.0))), "у ребёнка снят");
}

#[test]
fn out_of_flow_neighbours_keep_their_margins() {
    // Плавающий блок в схлопывании не участвует: его поле стоит как
    // написано, и соседа он не обкрадывает.
    let nodes = parse(
        "<div style=\"margin: 16px; float: left\">a</div>\
             <div style=\"margin: 16px; float: left\">b</div>",
        "",
    );
    let inner = match &nodes[0] {
        Node::Element(html) => collapse_margins(&html.children, false),
        _ => panic!("нет корня"),
    };
    let body = match &inner[0] {
        Node::Element(b) => collapse_margins(&b.children, false),
        _ => panic!("нет body"),
    };
    for (i, n) in body.iter().enumerate() {
        let Node::Element(e) = n else { continue };
        assert_eq!(
            e.style.margin.top,
            Some(Len::Px(16.0)),
            "плавающий блок {i} потерял поле"
        );
    }
}

#[test]
fn margins_in_em_collapse_too() {
    // `margin: 1em 0` — самая частая запись отступа в разметке: без
    // перевода в точки схлопывание не срабатывало вовсе.
    let nodes = parse(
        "<div style=\"margin-bottom: 1em\">a</div><div style=\"margin-top: 2em\">b</div>",
        "",
    );
    let inner = match &nodes[0] {
        Node::Element(html) => collapse_margins(&html.children, false),
        _ => panic!("нет корня"),
    };
    let body = match &inner[0] {
        Node::Element(b) => collapse_margins(&b.children, false),
        _ => panic!("нет body"),
    };
    let second = match &body[1] {
        Node::Element(e) => e.style.margin.top,
        _ => panic!("нет второго блока"),
    };
    // 32 всего, из них 16 уже дал нижний отступ предыдущего блока.
    assert_eq!(second, Some(Len::Px(16.0)), "получено {second:?}");
}

#[test]
fn adjacent_margins_collapse_into_the_larger() {
    // В CSS нижний отступ одного блока и верхний отступ следующего не
    // складываются: остаётся больший. Иначе документ растёт сверху вниз.
    let nodes = parse(
        "<div style=\"margin-bottom: 10px\">a</div><div style=\"margin-top: 24px\">b</div>",
        "",
    );
    let inner = match &nodes[0] {
        Node::Element(html) => collapse_margins(&html.children, false),
        _ => panic!("нет корня"),
    };
    let body = match &inner[0] {
        Node::Element(b) => collapse_margins(&b.children, false),
        _ => panic!("нет body"),
    };
    let second = match &body[1] {
        Node::Element(e) => e.style.margin.top,
        _ => panic!("нет второго блока"),
    };
    // 24 всего, из них 10 уже дал нижний отступ предыдущего блока.
    assert_eq!(second, Some(Len::Px(14.0)), "получено {second:?}");
}

#[test]
fn first_child_margin_leaks_through_a_borderless_parent() {
    // Отступ первого ребёнка в CSS — тот же отступ, что у родителя, если
    // между ними нет ни рамки, ни внутреннего отступа.
    let nodes = parse(
        "<div class=\"wrap\"><div class=\"in\" style=\"margin-top: 24px\">x</div></div>",
        "",
    );
    // Примыкание ТРАНЗИТИВНО (§8.3.1): поле уходит на самую внешнюю
    // коробку цепи, а у всех внутренних снимается. Пока подъём шёл на
    // один уровень, то же поле поднималось повторно на каждом.
    let inner = match &nodes[0] {
        Node::Element(html) => collapse_margins(&html.children, false),
        _ => panic!("нет корня"),
    };
    let body = match &inner[0] {
        Node::Element(b) => b,
        _ => panic!("нет body"),
    };
    assert_eq!(
        body.style.margin.top,
        Some(Len::Px(24.0)),
        "отступ вынесен на внешнюю коробку"
    );
    let wrap_top =
        find_class(std::slice::from_ref(&inner[0]), "wrap").and_then(|e| e.style.margin.top);
    assert_eq!(wrap_top, Some(Len::Px(0.0)), "у обёртки отступ снят");
    let child_top =
        find_class(std::slice::from_ref(&inner[0]), "in").and_then(|e| e.style.margin.top);
    assert_eq!(child_top, Some(Len::Px(0.0)), "у ребёнка отступ снят");
}
