//! Доказательство ограничения max-height по определённой стопке блоков.
use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::block::struts::{
    adjoin, margin_or_bail, margin_px, solve, strut_of, through_strut, top_edge_open, zero_len,
};
use crate::layout::box_kind::{in_flow, inline_level_box, replaced_inline};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// CSS2 §10.7 повторяет расчёт высоты и полей с ограничивающей высотой.
/// Blink block_layout_algorithm.cc:1459-1466 отделяет конечное поле, когда
/// используемая высота отличается от intrinsic. Доказательство применимо
/// к стопке с вычислимыми border edges; неизвестное содержимое её не даёт.
pub(crate) fn lowers(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let st = e.style.used();
    if !matches!(
        st.display,
        None | Some(
            crate::style::computed::Display::Block | crate::style::computed::Display::ListItem
        )
    ) {
        return false;
    }
    let Some(mut cap) = margin_px(st.max_height, &st, vcx) else {
        return false;
    };
    let edges = |style: &Computed| -> Option<f32> {
        let b = style.borders();
        [style.padding.top, style.padding.bottom, b.top, b.bottom]
            .into_iter()
            .try_fold(0.0, |sum, value| match value {
                None => Some(sum),
                Some(Len::Px(v)) => Some(sum + v),
                _ => None,
            })
    };
    if st.border_box == Some(true) {
        let Some(inset) = edges(&st) else {
            return false;
        };
        cap = (cap - inset).max(0.0);
    }
    if let Some(min) = margin_px(st.min_height, &st, vcx) {
        cap = cap.max(min);
    } else if !zero_len(st.min_height) {
        return false;
    }
    let mut bottom = None;
    let mut previous_margin = strut_of(0.0);
    for node in &e.children {
        let child = match node {
            Node::Text(text) if blank_text(text) => continue,
            Node::Text(_) => return false,
            Node::Element(child) if !in_flow(&child.style.used()) => continue,
            Node::Element(child) => child,
        };
        let cs = child.style.used();
        if inline_level_box(child)
            || cs.clear.is_some()
            || cs.min_height.is_some()
            || cs.max_height.is_some()
        {
            return false;
        }
        if let Some(through) = through_strut(child, vcx) {
            previous_margin = adjoin(previous_margin, through);
            continue;
        }
        let height = match margin_px(cs.height, &cs, vcx) {
            Some(height) => height,
            // Пустой блочный контейнер без строк имеет auto-высоту 0
            // (CSS2 §10.6.3/§10.6.7), в том числе пустой flow-root.
            None if matches!(cs.height, None | Some(Len::Auto))
                && matches!(
                    cs.display,
                    None | Some(crate::style::computed::Display::Block)
                )
                && !replaced_inline(&child.tag)
                && !crate::layout::fragment::table_bands::table_box(child)
                && child.list_item.is_none()
                && child.children.iter().all(crate::dom::whitespace::is_blank) =>
            {
                0.0
            }
            None => return false,
        };
        let Some(inset) = edges(&cs) else {
            return false;
        };
        let Some(top) = margin_or_bail(cs.margin.top, &cs, vcx) else {
            return false;
        };
        let height = if cs.border_box == Some(true) {
            height.max(inset)
        } else {
            height + inset
        };
        // Первое поле у открытого края схлопывается наружу и не входит
        // в auto-высоту содержимого (CSS2 §8.3.1, §10.6.3).
        let gap = if bottom.is_none() && top_edge_open(e) {
            0.0
        } else {
            solve(adjoin(previous_margin, strut_of(top)))
        };
        bottom = Some(bottom.unwrap_or(0.0) + gap + height);
        let Some(margin) = margin_or_bail(cs.margin.bottom, &cs, vcx) else {
            return false;
        };
        previous_margin = strut_of(margin);
    }
    bottom.is_some_and(|bottom| bottom > cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_start_margin_is_not_intrinsic_content_height() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut child_style = Computed {
            height: Some(Len::Px(100.0)),
            ..Default::default()
        };
        child_style.margin.top = Some(Len::Px(200.0));
        let mut parent = Element::synthetic(
            "div",
            Computed {
                max_height: Some(Len::Px(150.0)),
                ..Default::default()
            },
            Default::default(),
            vec![Node::Element(Element::synthetic(
                "div",
                child_style,
                Default::default(),
                vec![],
            ))],
        );
        assert!(!lowers(&parent, doc.values()));
        parent.style.used_mut().flow_root = Some(true);
        assert!(lowers(&parent, doc.values()));
    }

    #[test]
    fn constrained_stack_keeps_the_last_end_margin_inside() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut first = Computed {
            height: Some(Len::Px(150.0)),
            ..Default::default()
        };
        first.margin.bottom = Some(Len::Px(10.0));
        let mut last = Computed {
            flow_root: Some(true),
            ..Default::default()
        };
        last.margin.top = Some(Len::Px(20.0));
        last.margin.bottom = Some(Len::Px(80.0));
        let children = vec![
            Node::Element(Element::synthetic("div", first, Default::default(), vec![])),
            Node::Element(Element::synthetic(
                "section",
                last,
                Default::default(),
                vec![],
            )),
        ];
        let mut parent = Element::synthetic(
            "div",
            Computed {
                max_height: Some(Len::Px(0.0)),
                ..Default::default()
            },
            Default::default(),
            children,
        );
        assert!(lowers(&parent, doc.values()));
        assert!(super::super::separate(&mut parent, doc.values()));
        let Node::Element(first) = &parent.children[0] else {
            panic!("first")
        };
        assert_eq!(first.style.used().margin.bottom, Some(Len::Px(10.0)));
        parent.style.used_mut().max_height = Some(Len::Px(170.0));
        assert!(!lowers(&parent, doc.values()));
        parent.style.used_mut().max_height = Some(Len::Px(169.0));
        assert!(lowers(&parent, doc.values()));
    }
}
