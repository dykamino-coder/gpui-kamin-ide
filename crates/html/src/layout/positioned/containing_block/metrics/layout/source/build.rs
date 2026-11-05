//! Построение source и root-layout продолжаются на общем стеке обхода.
use super::*;
use gpui::{AvailableSpace, IntoElement, ParentElement, Styled, div, size};
/// One source tree laid out at `width` with its own recorder.
#[allow(clippy::too_many_arguments)]
pub(super) async fn build_async(
    kids: &Children<'_>,
    parent: &BoxStyle,
    width: f32,
    page: Option<(f32, &dyn Fn() -> Div)>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> (
    Rc<Recorder>,
    gpui::AnyElement,
    crate::layout::fragment::geometry::Node,
    gpui::Size<gpui::Pixels>,
) {
    let recorder = Rc::new(Recorder::for_fragments(kids.elements(), parent, opts));
    let mut opts = opts.clone();
    opts.cx.position = Default::default();
    opts.cx.deferred = Default::default();
    opts.cx.position.geometry_record = Some(recorder.clone());
    opts.cx.position.geometry = None;
    opts.cx.constraints.avail_w = Some(width);
    opts.frag = Default::default();
    // Начальная page area — настоящий ICB; normal потомки остаются overflow.
    let icb = page.as_ref().map(|_| {
        crate::layout::positioned::containing_block::LayerScope::icb(&opts.cx.position.state)
    });
    let paged = page
        .as_ref()
        .map(|_| crate::layout::page::paged::PagedBuild::begin());
    opts.frag.paged = paged.clone();
    let children = kids.build(parent, &opts);
    let is_page = page.is_some();
    let mut frame = match page {
        Some((height, frame)) => frame()
            .relative()
            .w(px(width))
            .h(px(height))
            .children(children),
        None => div().block().w(px(width)).children(children),
    };
    if let Some(icb) = icb {
        frame = frame
            .children(icb.finish())
            .children(opts.take_paged_fixed());
    }
    if let Some(paged) = paged {
        paged.end();
    }
    let mut root = frame.into_any_element();
    // Нулевой owner — прямой OOF, а настоящий id родителя нужен его CB.
    // Обе метки ссылаются на тот же LayoutId без дополнительной коробки.
    root =
        crate::layout::positioned::containing_block::metrics::record::wrap(0, parent, root, &opts);
    if !is_page && parent.self_node != 0 {
        root = crate::layout::positioned::containing_block::metrics::record::wrap(
            parent.self_node,
            parent,
            root,
            &opts,
        );
    }
    let layout = crate::layout::fragment::geometry::Node::capture(
        root.request_layout_async(window, cx, stack).await,
        window,
    );
    let root_size = root
        .layout_as_root_async(
            size(
                AvailableSpace::Definite(px(width)),
                AvailableSpace::MaxContent,
            ),
            window,
            cx,
            stack,
        )
        .await;
    (recorder, root, layout, root_size)
}
