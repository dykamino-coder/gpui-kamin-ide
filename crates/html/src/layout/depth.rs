//! Граница reflow: глубокие боксы сохранены, но не входят в фазы кадра.

use crate::document::{TraversalKind, state::DocState};
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Style, Window};
use std::rc::Rc;

/// Отдельно от предела построения боксов 2052, как MAX_REFLOW_DEPTH Gecko.
pub(crate) const MAX_REFLOW_DEPTH: usize = crate::budget::DocumentLimits::LAYOUT_DEPTH;
// Корни документа и viewport не уменьшают допустимую авторскую вложенность.
const ROOT_FRAMES: usize = 4;

pub(crate) fn wrap(child: AnyElement, opts: &RenderOpts) -> AnyElement {
    let depth = opts.cx.contracts.reflow_depth;
    if depth <= MAX_REFLOW_DEPTH + ROOT_FRAMES {
        child
    } else {
        Boundary {
            _child: child,
            doc: opts.cx.doc().clone(),
            attempted: depth - ROOT_FRAMES,
        }
        .hooked()
    }
}

struct Boundary {
    // Построенное поддерево остаётся живым, даже когда layout его пропускает.
    _child: AnyElement,
    doc: Rc<DocState>,
    attempted: usize,
}

impl Hooks for Boundary {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.doc
            .traversal_limit_reached(TraversalKind::Layout, MAX_REFLOW_DEPTH, self.attempted);
        // Нулевой размер и пустое overflow: состояние фазы остаётся корректным.
        let style = Style {
            size: gpui::size(gpui::px(0.).into(), gpui::px(0.).into()),
            flex_shrink: 0.,
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}
