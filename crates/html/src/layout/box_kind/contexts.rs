//! Определение собственного контекста форматирования элемента.

use crate::dom::Element;
use crate::layout::multicol::spanner::multicol_container;
pub(crate) use crate::style::computed::formatting_context::own_context_style;

/// Заводит ли коробка СВОЙ блочный контекст форматирования: через её край
/// поля не схлопываются ни с детьми, ни насквозь (CSS 2.1 §8.3.1).
pub(crate) fn own_context(e: &Element) -> bool {
    // A table caption is a block container that is not a block box: it
    // establishes a new block formatting context (CSS 2.2 section 9.4.1), so
    // its children's margins stay inside it
    // (`margin-collapsing-in-table-caption-002`).
    own_context_style(&e.style.used())
        // Полноширинный float в стопке представлен блочной коробкой, но
        // сохраняет собственный BFC: поля его детей через край не выходят
        // (CSS2 §9.4.1, §8.3.1). Снятие float относится лишь к размещению.
        || e.marks.float_block
        // HTML §15.3.13: анонимная коробка fieldset — независимый BFC;
        // её дочерние поля не переходят в поля fieldset до измерения копии.
        || (e.tag == "fieldset"
            && matches!(e.style.used().reset_box_layout.display, None | Some(crate::style::computed::Display::Block)
                | Some(crate::style::computed::Display::ListItem)))
        || (e.tag == "caption" && e.style.used().reset_box_layout.display.is_none())
        || e.style.used().reset_context.is_caption == Some(true)
        // A table (UA `display: table`, not written into `display`) never
        // collapses through (CSS 2.2 section 17.4: the table wrapper box
        // establishes a block formatting context); as a body's last child it
        // let the preceding paragraph's end margin escape
        // (`visibility-collapse-border-spacing-002`).
        || (e.tag == "table" && e.style.used().reset_box_layout.display.is_none())
        // `continue: collapse` (`line-clamp: <N>`/`auto`, у легаси — пара
        // `-webkit-box` по вертикали) делает блочный контейнер line-clamp
        // контейнером — НЕЗАВИСИМЫМ блочным контекстом (css-overflow-4
        // §continue «must establish an independent formatting context»,
        // §line-clamp-containers): поле первого ребёнка через его верх не
        // схлопывается (`line-clamp-auto-027`). Только собственный стиль:
        // слитый несёт `line_clamp` потомкам для текста.
        || ((e.style.used().clamp_lines().is_some() || e.style.used().clamp_by_height())
            && !multicol_container(&e.style.used()))
}

/// Line-clamp container with a clamp point to place (css-overflow-4
/// §line-clamp-containers): its whole content is laid out by its clamp host,
/// which alone knows the clamp point. `max-lines: auto` needs a block-size
/// limit (`c` — the box style with resolved lengths); on a multicol container
/// `continue: collapse` behaves as `auto` (§continue).
pub(crate) fn clamp_host(e: &Element, c: &crate::style::computed::Computed) -> bool {
    let used = e.style.used();
    (used.clamp_lines().is_some()
        || (used.clamp_by_height()
            && used.clamp_block_container()
            && crate::layout::replaced::limits::auto_clamp_limit(c).is_some()))
        && !multicol_container(&used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;

    #[test]
    fn block_representation_preserves_the_float_formatting_context() {
        fn find(nodes: &mut [Node]) -> Option<&mut Element> {
            nodes.iter_mut().find_map(|node| match node {
                Node::Element(e) => {
                    if e.tag == "div" {
                        Some(e)
                    } else {
                        find(&mut e.children)
                    }
                }
                _ => None,
            })
        }
        let mut tree = crate::document::parsed::parse("<div style='float:left'></div>", "");
        let float = find(&mut tree).unwrap();
        assert!(own_context(float));
        float.style.used_mut().reset_box_layout.float = None;
        float.marks.float_block = true;
        assert!(own_context(float));
        assert!(bfc_root(float));
        float.marks.float_block = false;
        assert!(!own_context(float));
    }

    #[test]
    fn clamp_host_needs_a_clamp_point_and_a_block_container_that_is_not_multicol() {
        for (css, host) in [
            ("line-clamp:2", true),
            ("line-clamp:auto;max-height:40px", true),
            ("line-clamp:auto", false),
            ("line-clamp:2;columns:2", false),
            ("display:-webkit-box;-webkit-line-clamp:2", false),
        ] {
            let doc = crate::document::state::DocState::new(Default::default());
            let mut style = crate::style::computed::Computed::default();
            style.apply_decls(&crate::style::css::parse_decls(css), doc.values());
            let e = Element::synthetic("div", style, Default::default(), Vec::new());
            assert_eq!(clamp_host(&e, &e.style.used()), host, "{css}");
        }
    }
}

/// Блочный контейнер — корень своего блочного контекста: поля детей через
/// его край не схлопываются (CSS 2.1 §8.3.1; `fieldset` — HTML §15.3.13
/// «establishes a new block formatting context»).
pub(crate) fn bfc_root(c: &Element) -> bool {
    use crate::style::computed::Display;
    matches!(
        c.style.used().reset_box_layout.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) && !c.inline
        && (crate::layout::box_kind::own_context(c) || c.tag == "fieldset")
}
