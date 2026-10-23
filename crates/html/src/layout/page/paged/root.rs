//! Корень постраничного режима: поля и имя первой страницы, группы строчных прогонов.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::dom::whitespace::is_blank;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

#[derive(PartialEq)]
pub(super) enum Run {
    None,
    Inline,
    Float,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_root_page(
    none: &mut bool,
    canvas: &mut Option<gpui::Hsla>,
    canvas_img: &mut Option<Computed>,
    root: &mut Computed,
    nodes: &mut Vec<Node>,
    left: &mut f32,
    right: &mut f32,
    top: &mut f32,
    root_page: &mut String,
    vcx: ValueCx<'_>,
) {
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else {
            break;
        };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        // Корень без коробки: один пустой лист, и свойства `@page` к нему не
        // применяются (Blink `StyleForPage`: «The root is display:none. One
        // page box will still be created, but no properties should apply»;
        // `root-element-display-none-print` против `blank-print-ref`).
        if matches!(e.style.computed().display, Some(Display::None)) {
            *none = true;
            *nodes = Vec::new();
            break;
        }
        let e = (*e).clone();
        let side = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
        let b = e.style.computed().borders();
        // Обёртку с видимой рамкой или своим `display` (сетка, флекс) не
        // снимаем: снятая теряла рамку и раскладку (`page-box-011-print-ref`:
        // `body { border: 10px solid }` — чёрной рамки не было;
        // `page-box-000-print-ref`: `html { display: grid; border: 20px }`).
        // Она остаётся одним ребёнком стопки; фон всё равно уходит в канвас
        // (§painting: фон корня/тела красит канвас листа).
        let framed = [&b.top, &b.right, &b.bottom, &b.left]
            .iter()
            .any(|l| side(l) > 0.0);
        let boxy = matches!(
            e.style.computed().display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::GridLanes)
        );
        if framed || boxy {
            if let Some(c) = e.style.computed().background.filter(|c| c.a > 0.0) {
                *canvas = Some(c.to_hsla());
            }
            // Gradients and images of the propagated background (CSS 2.1 §14.2)
            // paint the canvas too, above its colour (`page_stack::canvas`).
            if let Some(l) = crate::layout::page::page_stack::image_layers(&e.style.computed()) {
                *canvas_img = Some(l);
            }
            break;
        }
        *left += side(&e.style.computed().margin.left)
            + side(&b.left)
            + side(&e.style.computed().padding.left);
        *right += side(&e.style.computed().margin.right)
            + side(&b.right)
            + side(&e.style.computed().padding.right);
        *top += side(&e.style.computed().margin.top)
            + side(&b.top)
            + side(&e.style.computed().padding.top);
        if let Some(c) = e.style.computed().background.filter(|c| c.a > 0.0) {
            *canvas = Some(c.to_hsla());
        }
        // Gradients and images of the propagated background (CSS 2.1 §14.2)
        // paint the canvas too, above its colour (`page_stack::canvas`).
        if let Some(l) = crate::layout::page::page_stack::image_layers(&e.style.computed()) {
            *canvas_img = Some(l);
        }
        if let Some(p) = &e.style.computed().page {
            *root_page = p.clone();
        }
        *root = box_style(&*root, &e.style, vcx);
        *nodes = e.children;
    }
}

pub(super) fn group_runs(
    nodes: Vec<Node>,
    inline_level: impl Fn(&Node) -> bool,
    floated: impl Fn(&Node) -> bool,
    groups: &mut Vec<Vec<Node>>,
    mut run: Run,
) {
    for n in nodes.iter() {
        if is_blank(n) {
            // Пробел внутри строчного пробега — его часть, вне — пропуск.
            if run != Run::None
                && let Some(g) = groups.last_mut()
            {
                g.push(n.clone());
            }
            continue;
        }
        let (fl, il) = (floated(n), inline_level(n));
        let join = match run {
            Run::None => false,
            Run::Inline => il || fl,
            Run::Float => true,
        };
        match groups.last_mut() {
            Some(g) if join => g.push(n.clone()),
            _ => groups.push(vec![n.clone()]),
        }
        run = if fl || (run == Run::Float && il) {
            Run::Float
        } else if il {
            Run::Inline
        } else {
            Run::None
        };
    }
}

/// Block axis of the root writing mode (css-writing-modes-4 §3.1): the
/// direction the page stack fragments in; `None` — horizontal.
pub(super) fn root_axis(root: &Computed) -> Option<crate::layout::fragment::types::StackAxis> {
    let axis = crate::layout::fragment::types::StackAxis::of(root.wm());
    axis.is_vertical().then_some(axis)
}
