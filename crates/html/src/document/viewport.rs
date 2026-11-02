//! Область просмотра HTML: кому принадлежит прокрутка (css-overflow-3 §3.3,
//! css-scrollbars-1 §2) и синтетическая коробка `html-viewport`.
//! `policy` собирает стиль области просмотра: цвета и ширина полос — только от
//! `<html>`, `overflow` тела доезжает лишь сквозь необособленный корень с
//! `visible` по обеим осям; вставки `vw`/`vh` — по политике корня (css-values-4
//! §6.1.2, Blink `ComputeScrollbarWidthsForViewportUnits`), кастомные полосы
//! их не уменьшают. `wrap` заворачивает документ в коробку окна с фоном канваса,
//! если есть политика; `prepare`/`prepare_embedded` держат один порядок проходов
//! для начальной, обновлённой и встроенной разметки.

use crate::dom::Node;
use crate::style::cascade::inherit::inherit_unpainted;
use crate::style::computed::{BoxStyle, Computed, Display, Overflow};
#[cfg(test)]
use crate::style::values::value::Len;
use crate::values::ValueCx;

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
    let mut c = inherit_unpainted(&BoxStyle::default(), root.style.cascaded(), vcx);
    // The viewport takes the root's USED scrollbar colors (CSS Scrollbars 1
    // §3): `currentcolor` is the root's color, not the viewport's.
    c.reset_box_layout.scroll.colors = c
        .reset_box_layout
        .scroll
        .colors
        .as_ref()
        .map(|_| c.used_scrollbar_colors().map(|cols| cols.map(Into::into)));
    c.reset_box_layout.scroll.inherit_width = false;
    // CSS Values 4 §6.1.2; Blink ComputeScrollbarWidthsForViewportUnits:
    // only root policy reduces units, never overflow propagated from body.
    let viewport_axis = |v| match v {
        Some(Overflow::Hidden | Overflow::Clip) => Some(Overflow::Hidden),
        _ => Some(Overflow::Scroll),
    };
    let mut unit_policy = c.clone();
    unit_policy.reset_box_layout.overflow_x = viewport_axis(c.reset_box_layout.overflow_x);
    unit_policy.reset_box_layout.overflow_y = viewport_axis(c.reset_box_layout.overflow_y);
    let model = crate::interactive::scrollbar_gutter::Model::new(
        &unit_policy,
        c.reset_box_layout.scroll.forced,
    );
    // Blink StyleCascade::ApplyViewportUnitAffecting skips custom scrollbar styles.
    c.reset_positioned.viewport_unit_insets = Some(if c.reset_box_layout.custom_scrollbar {
        [0.0; 2]
    } else {
        model.total()
    });
    let visible = |v| matches!(v, None | Some(Overflow::Visible));
    if let Some(body) = root.children.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) {
        let body_style = inherit_unpainted(&c, body.style.cascaded(), vcx);
        c.inh.writing.rtl = body_style.inh.writing.rtl;
        // The viewport takes the block flow of the body's mode, not `sideways`.
        c.inh.writing.writing_mode = body_style.inh.writing.writing_mode.map(|m| {
            crate::style::computed::writing_mode::WritingModeKw::new(
                m.is_vertical(),
                m.is_rl(),
                false,
            )
        });
        let decl = body.style.cascaded();
        if visible(c.reset_box_layout.overflow_x)
            && visible(c.reset_box_layout.overflow_y)
            && !matches!(
                decl.reset_box_layout.display,
                Some(Display::None | Display::Contents)
            )
            && !super::canvas_background::any_containment(body)
        {
            c.reset_box_layout.overflow_x = decl.reset_box_layout.overflow_x;
            c.reset_box_layout.overflow_y = decl.reset_box_layout.overflow_y;
            c.reset_box_layout.scroll.forced = decl.reset_box_layout.scroll.forced;
        }
    }
    c.reset_box_layout.overflow_x = viewport_axis(c.reset_box_layout.overflow_x);
    c.reset_box_layout.overflow_y = viewport_axis(c.reset_box_layout.overflow_y);
    Some(c.into_computed())
}

pub(super) fn prepare_embedded(
    nodes: Vec<Node>,
    responsive: bool,
    owner_dark: Option<bool>,
    vcx: ValueCx<'_>,
) -> Vec<Node> {
    crate::values::image_values::prepare(&nodes, vcx);
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
        root.style.cascaded_mut().reset_box_layout.height = None;
    }
    nodes
}

// Keep document preparation identical for initial, updated, and embedded markup.
pub(super) fn prepare(nodes: Vec<Node>, vcx: ValueCx<'_>) -> (Vec<Node>, Computed) {
    crate::values::image_values::prepare(&nodes, vcx);
    let canvas = canvas_backing(&nodes, None);
    let viewport = policy(&nodes, vcx)
        .or_else(|| canvas.map(|_| Computed::default()))
        .filter(|c| {
            canvas.is_some()
                || c.used_scrollbar_colors().is_some()
                || c.reset_box_layout.scroll.forced.into_iter().any(|v| v)
                || c.reset_box_layout.scroll.gutter.is_some_and(|v| v != 0)
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
        Node::Element(e) if e.tag == "html" => {
            Some(e.style.cascaded().reset_cascade.dark_color_scheme)
        }
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
mod tests;
