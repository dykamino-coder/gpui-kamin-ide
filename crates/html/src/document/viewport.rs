//! Область просмотра HTML: кому принадлежит прокрутка (css-overflow-3 §3.3,
//! css-scrollbars-1 §2) и синтетическая коробка `html-viewport`.
//! `policy` собирает стиль области просмотра: цвета и ширина полос — только от
//! `<html>`, `overflow` тела доезжает лишь сквозь необособленный корень с
//! `visible` по обеим осям; вставки `vw`/`vh` — по политике корня (css-values-4
//! §6.1.2, Blink `ComputeScrollbarWidthsForViewportUnits`), кастомные полосы
//! их не уменьшают. `wrap` заворачивает документ в коробку окна с фоном канваса,
//! если есть политика; `prepare`/`prepare_embedded` держат один порядок проходов
//! для начальной, обновлённой и встроенной разметки.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::style::computed::{Computed, Display, Overflow};
#[cfg(test)]
use crate::style::values::value::Len;

mod wrapper;
pub(super) use wrapper::wrap;

pub(super) fn policy(nodes: &[Node], vcx: ValueCx<'_>) -> Option<Computed> {
    let root = nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "html" => Some(e),
        _ => None,
    })?;
    if super::canvas_background::any_containment(root) {
        return None;
    }
    let mut c = crate::style::cascade::inherit::inherit_unpainted(
        &crate::style::computed::BoxStyle::default(),
        root.style.cascaded(),
        vcx,
    );
    // The viewport takes the root's USED scrollbar colors (CSS Scrollbars 1
    // §3): `currentcolor` is the root's color, not the viewport's.
    c.scroll.colors = c
        .scroll
        .colors
        .as_ref()
        .map(|_| c.used_scrollbar_colors().map(|cols| cols.map(Into::into)));
    c.scroll.inherit_width = false;
    // CSS Values 4 §6.1.2; Blink ComputeScrollbarWidthsForViewportUnits:
    // only root policy reduces units, never overflow propagated from body.
    let viewport_axis = |v| match v {
        Some(Overflow::Hidden | Overflow::Clip) => Some(Overflow::Hidden),
        _ => Some(Overflow::Scroll),
    };
    let mut unit_policy = c.clone();
    unit_policy.overflow_x = viewport_axis(c.overflow_x);
    unit_policy.overflow_y = viewport_axis(c.overflow_y);
    let model = crate::interactive::scrollbar_gutter::Model::new(&unit_policy, c.scroll.forced);
    // Blink StyleCascade::ApplyViewportUnitAffecting skips custom scrollbar styles.
    c.viewport_unit_insets = Some(if c.custom_scrollbar {
        [0.0; 2]
    } else {
        model.total()
    });
    let visible = |v| matches!(v, None | Some(Overflow::Visible));
    if let Some(body) = root.children.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) {
        let body_style =
            crate::style::cascade::inherit::inherit_unpainted(&c, body.style.cascaded(), vcx);
        c.rtl = body_style.rtl;
        // The viewport takes the block flow of the body's mode, not `sideways`.
        c.writing_mode = body_style.writing_mode.map(|m| {
            crate::style::computed::writing_mode::WritingModeKw::new(
                m.is_vertical(),
                m.is_rl(),
                false,
            )
        });
        if visible(c.overflow_x)
            && visible(c.overflow_y)
            && !matches!(
                body.style.cascaded().display,
                Some(Display::None | Display::Contents)
            )
            && !super::canvas_background::any_containment(body)
        {
            c.overflow_x = body.style.cascaded().overflow_x;
            c.overflow_y = body.style.cascaded().overflow_y;
            c.scroll.forced = body.style.cascaded().scroll.forced;
        }
    }
    c.overflow_x = viewport_axis(c.overflow_x);
    c.overflow_y = viewport_axis(c.overflow_y);
    Some(c.into_computed())
}

pub(super) fn prepare_embedded(
    nodes: Vec<Node>,
    responsive: bool,
    owner_dark: Option<bool>,
    vcx: ValueCx<'_>,
) -> Vec<Node> {
    let canvas = canvas_backing(&nodes, owner_dark);
    let viewport = policy(&nodes, vcx).or_else(|| canvas.map(|_| Computed::default()));
    let (nodes, _) = super::unwrap_document(
        super::mark_canvas_background(
            super::resolve_logical(
                super::propagate_writing_mode(super::viewport_overflow(nodes), vcx),
                vcx,
            ),
            vcx,
        ),
        vcx,
    );
    let mut nodes = wrap(nodes, viewport, canvas);
    if responsive
        && let [Node::Element(root)] = nodes.as_mut_slice()
        && root.tag == "html-viewport"
    {
        root.style.cascaded_mut().height = None;
    }
    nodes
}

// Keep document preparation identical for initial, updated, and embedded markup.
pub(super) fn prepare(nodes: Vec<Node>, vcx: ValueCx<'_>) -> (Vec<Node>, Computed) {
    let canvas = canvas_backing(&nodes, None);
    let viewport = policy(&nodes, vcx)
        .or_else(|| canvas.map(|_| Computed::default()))
        .filter(|c| {
            canvas.is_some()
                || c.used_scrollbar_colors().is_some()
                || c.scroll.forced.into_iter().any(|v| v)
                || c.scroll.gutter.is_some_and(|v| v != 0)
        });
    let (nodes, root) = super::unwrap_document(
        super::mark_canvas_background(
            super::resolve_logical(
                super::propagate_writing_mode(super::viewport_overflow(nodes), vcx),
                vcx,
            ),
            vcx,
        ),
        vcx,
    );
    (wrap(nodes, viewport, canvas), root)
}

fn canvas_backing(
    nodes: &[Node],
    owner: Option<bool>,
) -> Option<crate::style::values::value::Color> {
    let dark = nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "html" => Some(e.style.cascaded().dark_color_scheme),
        _ => None,
    })?;
    if owner.map_or(!dark, |owner| owner == dark) {
        return None;
    }
    Some(crate::style::values::value::system_color(
        crate::style::values::value::SystemColor::Canvas,
        dark,
    ))
}

#[cfg(test)]
mod tests {
    //! Regression checks for viewport ownership across html and body.
    use super::*;

    #[test]
    fn viewport_units_use_root_unconditional_gutters_only() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for (css, expected) in [
            ("html { overflow: scroll }", [15.0, 15.0]),
            (
                "html { overflow-y: scroll; scrollbar-width: thin }",
                [10.0, 0.0],
            ),
            ("html { overflow: auto }", [0.0, 0.0]),
            (
                "html { overflow: scroll } html::-webkit-scrollbar { width: 300px }",
                [0.0, 0.0],
            ),
            (
                "html { overflow: scroll } body::-webkit-scrollbar { width: 300px }",
                [15.0, 15.0],
            ),
            (
                "html { overflow: scroll } html.unmatched::-webkit-scrollbar { width: 300px }",
                [15.0, 15.0],
            ),
            ("body { overflow: scroll }", [0.0, 0.0]),
            ("html { scrollbar-gutter: stable both-edges }", [30.0, 0.0]),
            (
                "html { overflow: scroll; scrollbar-width: none }",
                [0.0, 0.0],
            ),
        ] {
            let nodes = crate::dom::parse(&format!("<style>{css}</style>"), "");
            let c = policy(&nodes, vcx).unwrap();
            assert_eq!(c.viewport_unit_insets, Some(expected), "{css}");
            let mut child = crate::style::cascade::inherit::inherit_unpainted(
                &crate::style::computed::BoxStyle::from(c.clone()),
                &Computed {
                    width: Some(Len::Vw(1.0)),
                    height: Some(Len::Vh(1.0)),
                    ..Computed::default()
                },
                vcx,
            );
            child.resolve_viewport((800.0, 600.0), vcx);
            assert_eq!(child.width, Some(Len::Px(800.0 - expected[0])));
            assert_eq!(child.height, Some(Len::Px(600.0 - expected[1])));
            let mut image = crate::layout::table::anon::anon_element("img", vec![]);
            image.style.cascaded_mut().width = Some(Len::Vw(1.0));
            let mut image = crate::layout::replaced::image::pct_height_to_px(&image, &c);
            image
                .style
                .cascaded_mut()
                .resolve_viewport((800.0, 600.0), vcx);
            assert_eq!(
                image.style.cascaded().width,
                Some(Len::Px(800.0 - expected[0]))
            );
        }
    }

    #[test]
    fn body_colors_do_not_propagate_to_the_viewport() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let nodes = crate::dom::parse(
            "<style>body { scrollbar-color: red blue; overflow: scroll }</style>",
            "",
        );
        assert!(
            policy(&nodes, vcx)
                .unwrap()
                .scroll
                .colors
                .flatten()
                .is_none()
        );
    }

    #[test]
    fn root_width_and_computed_color_survive_body_propagation() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let nodes = crate::dom::parse(
            "<style>html { color: red; scrollbar-color: currentColor currentColor; scrollbar-width: none } body { color: blue; scrollbar-width: thin; overflow: scroll }</style>",
            "",
        );
        let c = policy(&nodes, vcx).unwrap();
        assert_eq!(
            c.used_scrollbar_colors().unwrap(),
            [crate::style::values::value::Color::parse("red", vcx).unwrap(); 2]
        );
        assert_eq!(c.scroll.width, Some(2));
        assert_eq!(c.scroll.forced, [true; 2]);
    }
}
