//! Initial cascade produces the same staged raw result as the dirty-root walk.
use crate::dom::{Node, shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, generated::counters::Counters, select::Ancestor};
use crate::values::ValueCx;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(in crate::dom) fn walk_children(
    handle: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    vars: &VarMap,
    counters: &mut Counters,
    path: &mut Vec<Ancestor>,
    preserve: bool,
    out: &mut Vec<Node>,
    vcx: ValueCx<'_>,
    invalidation: Option<&mut crate::dom::invalidation::Invalidation>,
    retain: bool,
) -> Result<Option<crate::dom::source_memo::SourceMemo>, crate::budget::admission::Error> {
    let mut recorder = retain.then(super::record::Recorder::default);
    super::engine::walk(
        handle,
        tree,
        scope,
        vars,
        counters,
        path,
        preserve,
        out,
        vcx,
        invalidation,
        recorder.as_mut(),
    )?;
    recorder
        .map(|r| r.finish_document(handle.key(), out))
        .transpose()
}
