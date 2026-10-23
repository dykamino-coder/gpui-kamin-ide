//! Отложенная ширина не требует оценки: цельные floats меряет тот же решатель копии.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::flow::{empty_floats, independent_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::layout::fragment::{Shape, paginate};
use crate::style::computed::{BoxStyle, Display};
use std::cell::RefCell;

/// TRANSITIONAL(F2-1): разрываемый float и обтекающий текст имеют разные consumed/resume.
/// Этот путь переносит только вход цельного float, как существующий Cand::Float.
pub(crate) fn float_copy(e: &Element, vcx: ValueCx<'_>) -> bool {
    fn separate(e: &Element) -> bool {
        // ADR P-19 TRANSITIONAL(F2-1): поля float не усекаются как поля обычного
        // блока; перенос margin box требует отдельного consumed/resume.
        (e.style.computed().float.is_some_and(|f| f != 0)
            && [
                e.style.computed().margin.top,
                e.style.computed().margin.bottom,
            ]
            .iter()
            .any(|l| !matches!(l, None | Some(crate::style::values::value::Len::Px(0.0)))))
            || crate::layout::fragment::flow::parallel_overflow(e)
            || e.style.computed().bdb_clone
            || e.style.computed().text_box_trim_start
            || e.style.computed().text_box_trim_end
            || e.marks.host_trim_start
            || e.marks.host_trim_end
            || e.style.computed().wm().vertical()
            || e.style.computed().column_span == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style.computed())
            || matches!(
                e.style.computed().display,
                Some(Display::Flex | Display::Grid)
            )
            || (out_of_flow(&e.style.computed())
                && (e.style.computed().float.unwrap_or(0) == 0
                    || !crate::layout::fragment::clone::content_monolith_blockified(e)))
            || e.children
                .iter()
                .any(|n| matches!(n, Node::Element(k) if separate(k)))
    }
    !separate(e)
        && paginate::gate(e, None, false, vcx)
        && paginate::candidates(e, vcx).is_some_and(|v| {
            v.iter()
                .any(|(_, c)| matches!(c, paginate::Cand::Float { .. }))
        })
}

pub(crate) fn pending(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Vec<(Element, Shape)>> {
    prepare(host, merged, vcx, None, &RefCell::default())
}

pub(crate) fn children(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: f32,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    prepare(host, merged, vcx, Some(width), measured)
}

fn prepare(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: Option<f32>,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    // ADR P-19 TRANSITIONAL(F2-1): clearance следующей копии зависит от float bands предыдущей.
    if crate::layout::fragment::flow::copy_clearance(&host.children)
        || merged.text_box_trim_start
        || merged.text_box_trim_end
    {
        return None;
    }
    let kids: Option<Vec<_>> = host
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            let independent = independent_copy(c) || empty_floats(c);
            if !independent && !float_copy(c, vcx) {
                return None;
            }
            let mut c = resolved_lengths(c, merged, vcx);
            let Some(w) = width else {
                // Поля разрешаются по настоящей ширине в children(), как у
                // остальных native-копий; pending не нуждается в их оценке.
                return Some((c, Shape::default()));
            };
            c = fragment_box(c, w, vcx);
            if !independent {
                c.marks.paginate = true;
            }
            let (margin_top, margin_bottom) = fragment_margins(&c)?;
            Some((
                c,
                Shape {
                    margin_top,
                    margin_bottom,
                    ..Shape::default()
                },
            ))
        })
        .collect();
    let kids = kids?;
    if let Some(width) = width {
        measured
            .borrow_mut()
            .extend(kids.iter().map(|(c, _)| (c.node_id, width)));
    }
    Some(kids)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::layout::fragment::types::StackAxis;
    use crate::layout::multicol::deferred::Recipe;
    use crate::layout::options::RenderOpts;

    fn host(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored host")
    }

    #[test]
    fn unknown_width_float_reuses_native_solver_and_resolves_calc_margins() {
        let tree = crate::dom::parse(
            "<div><div style='margin:calc(10% + 3px) 0 0'><div style='float:left;width:20px;overflow:scroll'>float</div>text</div><div>ordinary</div></div>",
            "",
        );
        let mut opts = RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        let recipe = Recipe::new(
            host(&tree),
            &BoxStyle::default(),
            &opts,
            2,
            0.0,
            Some(100.0),
            StackAxis::Horizontal,
            None,
        )
        .expect("deferred native float");
        let kids = recipe.children(120.0);
        assert_eq!(kids.len(), 2);
        assert!(kids[0].paginate.is_some());
        assert_eq!(kids[0].measure, Some(120.0));
        assert_eq!(kids[0].mt, 15.0);
        assert!(kids[1].regrow.is_some());
        #[cfg(feature = "wpt-harness")]
        assert!(
            opts.frag
                .probe
                .take()
                .values()
                .all(|p| p.estimate_calls == 0)
        );
    }

    #[test]
    fn empty_float_host_is_measured_without_a_height_estimate() {
        let tree = crate::dom::parse(
            "<div><div><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div></div>",
            "",
        );
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let recipe = Recipe::new(
            host(&tree),
            &BoxStyle::default(),
            &opts,
            2,
            0.0,
            Some(100.0),
            StackAxis::Horizontal,
            None,
        )
        .unwrap();
        let kids = recipe.children(100.0);
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].measure, Some(100.0));
        assert!(kids[0].paginate.is_none());
        assert!(kids[0].watched.as_ref().is_some_and(|w| w.has_blocks()));
        #[cfg(feature = "wpt-harness")]
        assert!(
            opts.frag
                .probe
                .take()
                .values()
                .all(|p| p.estimate_calls == 0)
        );
    }

    #[test]
    fn parallel_float_continuations_are_not_silently_replaced() {
        for special in [
            "<div style='height:100px'><div style='float:left;width:100%;height:100px;break-inside:avoid'></div></div>",
            "<div style='float:left;width:20px'>several flowing lines</div>",
            "<div style='float:left;width:100%;height:20px;margin:40px 0;contain:size'>float</div>",
            "<div style='float:left'><div>A</div><div>B</div></div>",
            "<div style='float:left;break-inside:avoid'><div>A</div><div>B</div></div>",
            "<div style='float:left'>float</div><div style='box-decoration-break:clone'>x</div>",
            "<div style='float:left'>float</div><div style='text-box-trim:trim-both'>x</div>",
            "<div style='float:left'>float</div><div style='position:absolute'>x</div>",
            "<div style='float:left'>float</div><div style='columns:2'>x</div>",
        ] {
            let tree = crate::dom::parse(&format!("<div><div>{special}</div></div>"), "");
            assert!(
                pending(host(&tree), &BoxStyle::default(), tree.values()).is_none(),
                "{special}"
            );
        }
    }
}
