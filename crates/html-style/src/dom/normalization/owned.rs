//! A full conversion runs the common evaluator without retaining pass records.
use super::Transform;
use crate::budget::admission::{Admission, Error};
use crate::dom::Node;

/// The caller transfers staged nodes: failure cannot modify a committed tree.
pub(crate) fn apply<T: Transform>(
    transform: &T,
    nodes: Vec<Node>,
    context: T::Context,
    admission: &Admission,
) -> Result<Vec<Node>, Error> {
    let result = super::engine::evaluate(transform, nodes.into(), context, false, None, admission)?;
    Ok(result.output.into_vec())
}
