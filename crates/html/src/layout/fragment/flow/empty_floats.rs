//! Пустые float-потомки не несут продолжения строк; геометрию отдаёт BandFlow.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::Display;

pub(crate) fn empty_floats(e: &Element) -> bool {
    fn walk(e: &Element) -> Option<bool> {
        if !matches!(
            e.style.computed().display,
            None | Some(Display::Block | Display::ListItem)
        ) || !crate::layout::page::paged::visible_overflow(&e.style.computed())
        {
            return None;
        }
        // TRANSITIONAL(F2-1): Переполнение ограниченного блока требует собственного resume; удалить отказ в P05 после раздельного продолжения normal flow и overflow.
        if super::parallel_overflow(e) {
            return None;
        }
        let mut root = e.clone();
        root.children.clear();
        if !super::independent_copy(&root) {
            return None;
        }
        let mut floats = false;
        for n in children(&e.children).filter(|n| !is_blank(n)) {
            let Node::Element(f) = n else { return None };
            if f.style.computed().float.is_some_and(|s| s != 0) {
                let mut flow = f.clone();
                flow.style.used_mut().float = None;
                if !children(&f.children).all(is_blank) || !super::independent_copy(&flow) {
                    return None;
                }
                floats = true;
            } else {
                floats |= walk(f)?;
            }
        }
        Some(floats)
    }
    // Вся проверка относится к блочной оси. BandFlow сохраняет физическое
    // дерево; транспонирование нужно только для метаданных допуска.
    if e.style.computed().wm().vertical() {
        super::logical_copy(e, e.style.computed().wm().block_rl())
            .is_some_and(|logical| walk(&logical) == Some(true))
    } else {
        walk(e) == Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authored(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored tree")
    }

    #[test]
    fn nested_empty_floats_and_clearance_keep_native_geometry() {
        let tree = crate::dom::parse(
            "<div><div><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div><div style='clear:both;height:20px'></div></div>",
            "",
        );
        assert!(empty_floats(authored(&tree)));
    }

    #[test]
    fn text_and_separate_contexts_are_not_hidden_by_empty_float_dispatch() {
        for body in [
            "<div style='height:100px'><div style='float:left;width:100%;height:100px;break-inside:avoid'></div></div>",
            "<div style='float:left'>flowing text</div>",
            "<div style='float:left;height:30px'></div>wrapping text",
            "<div style='overflow:hidden'><div style='float:left'></div></div>",
            "<div style='position:absolute'></div><div style='float:left'></div>",
            "<div style='box-decoration-break:clone'><div style='float:left'></div></div>",
        ] {
            let tree = crate::dom::parse(&format!("<div>{body}</div>"), "");
            assert!(!empty_floats(authored(&tree)), "{body}");
        }
    }
}
