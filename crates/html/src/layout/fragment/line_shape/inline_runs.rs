//! Группы строчных прогонов и боковые поля обёртки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::table::anon::anon_element;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// Копия ребёнка стопки встаёт КОРНЕМ (`layout_as_root` во всю
/// колонку), а корень taffy своих полей не кладёт: боковое поле `margin: 0 1em`
/// пропадало, текст ложился от края колонки (`multicol-nested-002`). Обёртка-
/// колонка делает копию обычным ребёнком: её поля и растяжение решает
/// раскладка (CSS 2.1 §10.3.3). Только горизонтальная стопка и только при
/// полях, которым нужна раскладка относительно ширины колонки.
pub(crate) fn side_margin_wrap(el: AnyElement, copy: &Element, vertical: bool) -> AnyElement {
    if vertical || !has_side_margins(copy) {
        return el;
    }
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(el)
        .into_any_element()
}

fn has_side_margins(copy: &Element) -> bool {
    let margin = &copy.style.used().margin;
    [margin.left, margin.right]
        .iter()
        .any(|l| !matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0))))
}

/// Строчные прогоны среди блочных детей многоколоночника — в анонимные блоки
/// (CSS 2.1 §9.2.1.1: «If a block container box has a block-level box inside
/// it, then we force it to have only block-level boxes inside it» — строчное
/// содержимое оборачивается анонимной блочной коробкой). Тогда стопка колонок
/// видит их обычными детьми и режет по строкам. `None` — заворачивать нечего.
pub(crate) fn group_inline_runs(e: &Element) -> Option<Element> {
    let inline_level = |n: &Node| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            k.inline && !out_of_flow(&k.style.used()) && k.style.used().display.is_none()
        }
    };
    if !e.children.iter().any(|n| inline_level(n) && !is_blank(n)) {
        return None;
    }
    let mut out: Vec<Node> = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.iter().all(is_blank) {
            out.append(run);
        } else {
            out.push(Node::Element(anon_element(
                "anon-block",
                std::mem::take(run),
            )));
        }
    };
    for n in &e.children {
        if inline_level(n) {
            run.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    // `text-box-trim` хоста режет его ПЕРВУЮ/ПОСЛЕДНЮЮ отформатированную
    // строку (css-inline-3 §4.2). Строки ушли в анонимные блоки — флаг едет
    // туда, где строка: первому анонимному, если он первый ребёнок, и
    // последнему, если последний (`text-box-trim-multicol-001`).
    if e.style.used().text_box_trim_start || e.style.used().text_box_trim_end {
        let flow: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n))
            .map(|(i, _)| i)
            .collect();
        let anon = |n: &Node| matches!(n, Node::Element(k) if k.tag == "anon-block");
        if e.style.used().text_box_trim_start
            && let Some(&i) = flow.first()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.used_mut().text_box_trim_start = true;
        }
        if e.style.used().text_box_trim_end
            && let Some(&i) = flow.last()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.used_mut().text_box_trim_end = true;
        }
    }
    let mut g = e.clone();
    g.children = out;
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{Hooks, IntoHooked};
    use crate::layout::fragment::geometry::Node as GeometryNode;
    use gpui::{AvailableSpace, Edges, Length, Style, point, px, size};
    use std::{cell::Cell, rc::Rc};

    struct Child {
        margin: Length,
        geometry: Rc<Cell<Option<GeometryNode>>>,
    }

    impl Hooks for Child {
        type Layout = ();
        type Prepaint = ();
        fn request_layout(
            &mut self,
            window: &mut gpui::Window,
            cx: &mut gpui::App,
        ) -> (gpui::LayoutId, ()) {
            let id = window.request_layout(
                Style {
                    size: size(px(12.).into(), px(40.).into()),
                    margin: Edges {
                        left: self.margin,
                        right: self.margin,
                        ..Edges::all(px(0.).into())
                    },
                    ..Style::default()
                },
                [],
                cx,
            );
            self.geometry.set(Some(GeometryNode::capture(id, window)));
            (id, ())
        }
        fn prepaint(
            &mut self,
            _: gpui::Bounds<gpui::Pixels>,
            _: &mut (),
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
        fn paint(
            &mut self,
            _: gpui::Bounds<gpui::Pixels>,
            _: &mut (),
            _: &mut (),
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    }

    #[gpui::test]
    fn layout_root_keeps_auto_and_percentage_inline_margins(cx: &mut gpui::TestAppContext) {
        let cx = cx.add_empty_window();
        for (value, margin, x) in [
            ("auto", Length::Auto, 94.),
            ("10%", gpui::relative(0.1).into(), 20.),
            ("-2px", px(-2.).into(), -2.),
            ("0", px(0.).into(), 0.),
        ] {
            let tree = crate::dom::parse(&format!("<p style='margin:0 {value};width:0'></p>"), "");
            fn find(nodes: &[Node]) -> Option<&Element> {
                nodes.iter().find_map(|n| match n {
                    Node::Element(e) if e.tag == "p" => Some(e),
                    Node::Element(e) => find(&e.children),
                    _ => None,
                })
            }
            let geometry = Rc::new(Cell::new(None));
            cx.draw(
                point(px(0.), px(0.)),
                size(px(200.), px(100.)),
                |window, app| {
                    let child = Child {
                        margin,
                        geometry: geometry.clone(),
                    }
                    .hooked();
                    let mut el = side_margin_wrap(child, find(&tree).expect("paragraph"), false);
                    el.layout_as_root(
                        size(
                            AvailableSpace::Definite(px(200.)),
                            AvailableSpace::MaxContent,
                        ),
                        window,
                        app,
                    );
                    let bounds = geometry
                        .get()
                        .expect("child node")
                        .bounds(window)
                        .expect("final geometry");
                    assert_eq!(bounds.origin.x, px(x), "{value}");
                    assert_eq!(bounds.size.width, px(12.), "{value}");
                    gpui::div()
                },
            );
        }
    }
}
