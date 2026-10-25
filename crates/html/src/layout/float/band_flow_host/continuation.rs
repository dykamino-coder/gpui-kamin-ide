//! Каждая проба ребёнка использует устойчивый адрес внутри хоста BFC.
use crate::layout::fragment::flow::FlowMeasure;
use crate::layout::options::RenderOpts;
use std::rc::Rc;

pub(super) fn watched(
    flow: Rc<FlowMeasure>,
) -> Rc<crate::layout::fragment::paginate::watch::Watched> {
    let mut watched = crate::layout::fragment::paginate::watch::Watched::for_lines();
    Rc::get_mut(&mut watched).expect("new flow recorder").flow = flow;
    watched
}

pub(super) fn child(opts: &RenderOpts, key: usize) -> RenderOpts {
    let mut own = opts.clone();
    own.frag.watch = opts
        .frag
        .watch
        .as_ref()
        .map(|w| watched(w.flow.context(key)));
    own
}
