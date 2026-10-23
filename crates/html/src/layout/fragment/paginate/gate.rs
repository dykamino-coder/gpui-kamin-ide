//! Гейт пагинации по раскладке: какие дети многоколоночника идут этим путём — блочного уровня,
//! не монолит, с видимым переполнением, содержащие flex-контейнер, измеренные флоаты или
//! принудительные разрывы; строки, уже раскрытые в параллельные потоки, зазоры без модели
//! усечения и поддеревья с неизвестной геометрией остаются на пути оценки `shape_full`.
//! `refusal` называет причину отказа.

use super::candidates;
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::Display;

/// Gate of the path: a block-level, non-monolithic child of a horizontal
/// multicol containing a flex container, measured floats or forced breaks.
/// Everything else keeps the shape-estimate path.
pub(crate) fn gate(c: &Element, col_w: Option<f32>, auto_fill: bool, vcx: ValueCx<'_>) -> bool {
    refusal(c, col_w, auto_fill, vcx).is_none()
}

/// Why `gate` refuses `c` (`None` — accepted).
pub(crate) fn refusal(
    c: &Element,
    _col_w: Option<f32>,
    _auto_fill: bool,
    vcx: ValueCx<'_>,
) -> Option<&'static str> {
    // Flex gaps adjoining a break are truncated (Blink
    // `UpdateOffsetAdjustmentForSuppressedRowGap`); struts do not model that.
    fn gapped(e: &Element) -> bool {
        (matches!(
            e.style.used().display,
            Some(Display::Flex) | Some(Display::Grid)
        ) && e.style.used().gap.is_some()
            && super::row_gap(&e.style.used()).is_none())
            || (children(&e.children)
                .any(|n| matches!(n, Node::Element(k) if !monolith_box(k) && gapped(k))))
    }
    fn has_flex(e: &Element) -> bool {
        e.style.used().display == Some(Display::Flex)
            || (children(&e.children)
                .any(|n| matches!(n, Node::Element(k) if !monolith_box(k) && has_flex(k))))
    }
    if c.inline
        || out_of_flow(&c.style.used())
        || !matches!(
            c.style.used().display,
            Some(Display::Block) | Some(Display::Flex) | None
        )
    {
        return Some("display");
    }
    if monolith_box(c)
        || crate::layout::multicol::spanner::multicol_container(&c.style.used())
        || !crate::layout::page::paged::visible_overflow(&c.style.used())
        || c.style.used().bdb_clone
        || c.style.used().column_span == Some(true)
    {
        return Some("box");
    }
    if gapped(c) {
        return Some("gap");
    }
    match candidates(c, vcx) {
        None => Some("subtree"),
        Some(v)
            if !has_flex(c)
                && !v.iter().any(|x| {
                    matches!(x.1, super::Cand::Float { .. } | super::Cand::Forced { .. })
                }) =>
        {
            Some("no-layout-candidate")
        }
        // Row gaps alone keep the old path: this path paints gap decorations
        // half a gap low (css-gaps flex-gap-decorations-fragmentation-023/024/
        // 031), and a test with rules must take the same path as its ref.
        Some(v)
            if !v
                .iter()
                .any(|x| !matches!(x.1, super::Cand::Item | super::Cand::Gap { .. }))
                && !super::avoid_row(c) =>
        {
            Some("no-cand")
        }
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_keep_their_row_fragmentation_planner() {
        for html in [
            "<table><tr><td>A</td></tr><tr style='break-before:column'><td>B</td></tr></table>",
            "<div style='display:table'><div style='break-before:column'>B</div></div>",
            "<div><table><tr style='break-before:column'><td>B</td></tr></table></div>",
        ] {
            let tree = crate::dom::parse(html, "");
            let Node::Element(parent) = &tree[0] else {
                panic!("table container")
            };
            assert!(!gate(parent, Some(50.0), false, tree.values()));
        }
    }

    #[test]
    fn inline_content_floats_use_the_measured_entry_strut() {
        let tree = crate::dom::parse(
            "<div><div style='float:left;line-height:70px'><br></div></div>",
            "",
        );
        let Node::Element(parent) = &tree[0] else {
            panic!("block")
        };
        let candidates = super::candidates(parent, tree.values()).expect("candidates");
        assert!(
            candidates
                .iter()
                .any(|(_, c)| matches!(c, crate::layout::fragment::paginate::Cand::Float { .. }))
        );
        assert!(gate(parent, Some(50.0), true, tree.values()));
    }

    #[test]
    fn forced_breaks_use_measured_layout_without_flex_or_float() {
        let tree = crate::dom::parse(
            "<div><div style='break-after:column'>A<br>A</div><div>A<br>A</div></div>",
            "",
        );
        let Node::Element(parent) = &tree[0] else {
            panic!("block")
        };
        let candidates = super::candidates(parent, tree.values()).expect("candidates");
        assert!(
            candidates
                .iter()
                .any(|(_, c)| matches!(c, crate::layout::fragment::paginate::Cand::Forced { .. }))
        );
        // CSS Break 3 section 3.1: forced breaks apply to ordinary block flow;
        // they do not require a flex or float formatting context.
        assert!(gate(parent, Some(9.0), true, tree.values()));
    }
    #[test]
    fn deep_float_and_forced_candidates_keep_native_geometry() {
        for body in [
            "<div style='float:left;overflow:scroll;width:10px'>float</div>",
            "<div style='break-before:column'>forced</div>",
        ] {
            let html = format!(
                "<div>{}{}{}</div>",
                "<div>".repeat(24),
                body,
                "</div>".repeat(24)
            );
            let tree = crate::dom::parse(&html, "");
            let Node::Element(parent) = &tree[0] else {
                panic!("block")
            };
            assert!(gate(parent, None, true, tree.values()));
        }
        let html = format!(
            "<div>{}<div style='display:flex;gap:10%'><div>gap</div></div>{}</div>",
            "<div>".repeat(24),
            "</div>".repeat(24)
        );
        let tree = crate::dom::parse(&html, "");
        let Node::Element(parent) = &tree[0] else {
            panic!("block")
        };
        assert_eq!(refusal(parent, None, true, tree.values()), Some("gap"));
    }
}
