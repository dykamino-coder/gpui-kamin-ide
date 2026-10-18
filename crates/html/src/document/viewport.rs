//! HTML viewport scroll ownership (CSS Overflow 3 §3.3, Scrollbars 1 §2).
//! Colors come from the root; body overflow propagates through an uncontained root.
use crate::dom::Node;
use crate::style::computed::{Computed, Display, Overflow};
use crate::style::values::value::Len;

pub(super) fn policy(nodes: &[Node]) -> Option<Computed> {
    let root = nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "html" => Some(e),
        _ => None,
    })?;
    if super::canvas_background::any_containment(root) {
        return None;
    }
    let mut c =
        crate::style::cascade::inherit::inherit_unpainted(&Computed::default(), &root.style);
    // This is a computed viewport policy, not another currentColor declaration.
    c.scroll.current = [false; 2];
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
        let body_style = crate::style::cascade::inherit::inherit_unpainted(&c, &body.style);
        c.rtl = body_style.rtl;
        c.vertical = body_style.vertical;
        c.vertical_rl = body_style.vertical_rl;
        if visible(c.overflow_x)
            && visible(c.overflow_y)
            && !matches!(body.style.display, Some(Display::None | Display::Contents))
            && !super::canvas_background::any_containment(body)
        {
            c.overflow_x = body.style.overflow_x;
            c.overflow_y = body.style.overflow_y;
            c.scroll.forced = body.style.scroll.forced;
        }
    }
    c.overflow_x = viewport_axis(c.overflow_x);
    c.overflow_y = viewport_axis(c.overflow_y);
    Some(c)
}

pub(super) fn wrap(
    nodes: Vec<Node>,
    policy: Option<Computed>,
    canvas: Option<crate::style::values::value::Color>,
) -> Vec<Node> {
    let Some(c) = policy else { return nodes };
    fn maximum_id(nodes: &[Node]) -> u64 {
        nodes
            .iter()
            .filter_map(|node| match node {
                Node::Element(e) => Some(e.node_id.max(maximum_id(&e.children))),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }
    let id = maximum_id(&nodes) + 1;
    let mut viewport = crate::layout::table::anon::anon_element("html-viewport", nodes);
    viewport.node_id = id;
    viewport.style = Computed {
        display: Some(Display::Block),
        width: Some(Len::Vw(1.0)),
        height: Some(Len::Vh(1.0)),
        border_box: Some(true),
        background: canvas,
        overflow_x: c.overflow_x,
        overflow_y: c.overflow_y,
        rtl: c.rtl,
        vertical: c.vertical,
        vertical_rl: c.vertical_rl,
        scroll: c.scroll,
        viewport_unit_insets: c.viewport_unit_insets,
        ..Computed::default()
    }
    .into();
    vec![Node::Element(viewport)]
}

pub(super) fn prepare_embedded(
    nodes: Vec<Node>,
    responsive: bool,
    owner_dark: Option<bool>,
) -> Vec<Node> {
    let canvas = canvas_backing(&nodes, owner_dark);
    let viewport = policy(&nodes).or_else(|| canvas.map(|_| Computed::default()));
    let (nodes, _) = super::unwrap_document(super::mark_canvas_background(super::resolve_logical(
        super::propagate_writing_mode(super::viewport_overflow(nodes)),
    )));
    let mut nodes = wrap(nodes, viewport, canvas);
    if responsive
        && let [Node::Element(root)] = nodes.as_mut_slice()
        && root.tag == "html-viewport"
    {
        root.style.height = None;
    }
    nodes
}

// Keep document preparation identical for initial, updated, and embedded markup.
pub(super) fn prepare(nodes: Vec<Node>) -> (Vec<Node>, Computed) {
    let canvas = canvas_backing(&nodes, None);
    let viewport = policy(&nodes)
        .or_else(|| canvas.map(|_| Computed::default()))
        .filter(|c| {
            canvas.is_some()
                || c.scroll.colors.flatten().is_some()
                || c.scroll.forced.into_iter().any(|v| v)
                || c.scroll.gutter.is_some_and(|v| v != 0)
        });
    let (nodes, root) =
        super::unwrap_document(super::mark_canvas_background(super::resolve_logical(
            super::propagate_writing_mode(super::viewport_overflow(nodes)),
        )));
    (wrap(nodes, viewport, canvas), root)
}

fn canvas_backing(
    nodes: &[Node],
    owner: Option<bool>,
) -> Option<crate::style::values::value::Color> {
    let dark = nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "html" => Some(e.style.dark_color_scheme),
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
