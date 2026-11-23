//! A full conversion executes the same hooks without allocating retained records.
use super::Transform;
use crate::budget::admission::{Admission, Error};
use crate::dom::{Node, traversal};

/// The caller transfers staged nodes: failure cannot modify a committed tree.
/// Keeping this evaluator unrecorded avoids creating and discarding a cache for
/// conversions whose owner does not retain a normalization pipeline yet.
pub(crate) fn apply<T: Transform>(
    transform: &T,
    mut nodes: Vec<Node>,
    context: T::Context,
    admission: &Admission,
) -> Result<Vec<Node>, Error> {
    traversal::postorder_context(
        admission,
        &mut nodes,
        context.clone(),
        |element, parent| transform.enter(element, parent, admission),
        |element, parent, children| {
            transform.sequence(&mut element.children, children, admission)?;
            transform.leave(element, parent, admission)
        },
    )?;
    let mut nodes = nodes.into();
    transform.sequence(&mut nodes, &context, admission)?;
    Ok(nodes.into_vec())
}
