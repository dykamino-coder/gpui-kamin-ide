//! Фрагменты предков спаннера: видимость и разделение декораций slice/clone.

use super::spanner_edge;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
mod parts;
pub(super) use parts::spanner_parts;

/// Виден ли фрагмент предка: непустое содержимое или кромка коробки на
/// своей стороне разреза. Пустой фрагмент с кромкой обязателен —
/// css-multicol-1 §column-span, `Overview.bs:1540-1541`: «If the fragment
/// before the spanner is empty, nothing special happens; the top
/// margin/border/padding is above the spanning element, as an empty
/// fragment».
pub(super) fn spanner_frag_visible(c: &Element, kids: &[Node], first: bool, last: bool) -> bool {
    if kids.iter().any(|n| !is_blank(n)) {
        return true;
    }
    let b = &c.style.used().border_width;
    let clone = c.style.used().bdb_clone;
    (first
        && (spanner_edge(&c.style.used().margin.top)
            || spanner_edge(&c.style.used().padding.top)
            || spanner_edge(&b.top)))
        || (last
            && (spanner_edge(&c.style.used().margin.bottom)
                || spanner_edge(&c.style.used().padding.bottom)
                || spanner_edge(&b.bottom)))
        || (clone
            && (spanner_edge(&c.style.used().padding.top)
                || spanner_edge(&c.style.used().padding.bottom)
                || spanner_edge(&b.top)
                || spanner_edge(&b.bottom)))
}

/// Фрагмент предка спаннера. css-break-3 §4.3 (вид `slice`, умолчание
/// `box-decoration-break`): верхние поле/рамка/отбивка — только у первого
/// фрагмента, нижние — только у последнего. При `clone` рамки и padding
/// остаются у каждого фрагмента; клонированные поля на разрыве нулевые.
///
/// `keep_size` оставляет исходное ограничение только неразрезанной коробке.
/// Для разрезанной коробки ParentResume задаёт остаток content-box после
/// native-измерения предыдущего фрагмента; спаннер в этот расход не входит.
pub(super) fn spanner_fragment(
    c: &Element,
    kids: Vec<Node>,
    first: bool,
    last: bool,
    keep_size: bool,
    ix: usize,
) -> Element {
    let mut f = c.clone();
    f.children = kids.into();
    if !first {
        f.style.used_mut().margin.top = None;
        if !c.style.used().bdb_clone {
            f.style.used_mut().padding.top = None;
            f.style.used_mut().border_width.top = None;
        }
        // Устойчивый номер узла у продолжения свой: по нему GPUI хранит
        // состояние (анимация, буферы линеек промежутков), и два фрагмента
        // с одним номером слились бы в один.
        f.node_id = c.node_id ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    if !last {
        f.style.used_mut().margin.bottom = None;
        if !c.style.used().bdb_clone {
            f.style.used_mut().padding.bottom = None;
            f.style.used_mut().border_width.bottom = None;
        }
    }
    if !keep_size {
        f.style.used_mut().height = None;
        f.marks.spanner_content_box = true;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn spanner_break_slices_edges_but_clone_repeats_only_decorations() {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        for clone in [false, true] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='margin:10px; padding:20px; border:3px solid; box-decoration-break:{}'></div>",
                    if clone { "clone" } else { "slice" }
                ),
                "",
            );
            let parent = find(&tree).expect("parent");
            let fragment = spanner_fragment(parent, vec![], false, false, false, 1);
            let style = fragment.style.used();
            assert!(style.margin.top.is_none() && style.margin.bottom.is_none());
            assert_eq!(style.padding.top, clone.then_some(Len::Px(20.)));
            assert_eq!(style.padding.bottom, clone.then_some(Len::Px(20.)));
            assert_eq!(style.border_width.top, clone.then_some(Len::Px(3.)));
            assert_eq!(style.border_width.bottom, clone.then_some(Len::Px(3.)));
            assert_ne!(fragment.node_id, parent.node_id);
            assert_eq!(spanner_frag_visible(parent, &[], false, false), clone);
        }
    }
}
