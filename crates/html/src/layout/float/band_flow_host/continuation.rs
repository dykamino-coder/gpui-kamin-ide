//! Каждая проба ребёнка использует устойчивый адрес внутри хоста BFC.
use crate::dom::Element;
use crate::layout::fragment::flow::FlowMeasure;
use crate::layout::fragment::{ShapeCx, flow::blocks::BlockBox, types::StackAxis};
use crate::layout::options::RenderOpts;
use std::rc::Rc;

pub(super) fn watched(
    flow: Rc<FlowMeasure>,
) -> Rc<crate::layout::fragment::paginate::watch::Watched> {
    crate::layout::fragment::paginate::watch::Watched::with_flow(flow)
}

pub(super) fn child(opts: &RenderOpts, key: usize, node: &Element, axis: StackAxis) -> RenderOpts {
    let mut own = opts.clone();
    own.frag.watch = opts.frag.watch.as_ref().map(|w| {
        let blocks = BlockBox::collect(node, ShapeCx::COLUMNS, opts.cx.vcx());
        crate::layout::fragment::paginate::watch::Watched::for_blocks_flow(
            blocks,
            axis,
            w.flow.context(key),
        )
    });
    own
}
