//! A full conversion executes the same hooks without allocating retained records.
use super::Transform;
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Element, Node};

struct Frame<C> {
    element: Element,
    next: usize,
    parent: C,
    children: C,
}

/// The caller transfers staged nodes: failure cannot modify a committed tree.
/// Keeping this evaluator unrecorded avoids creating and discarding a cache for
/// conversions whose owner does not retain a normalization pipeline yet.
pub(crate) fn apply<T: Transform>(
    transform: &T,
    mut nodes: Vec<Node>,
    context: T::Context,
    admission: &Admission,
) -> Result<Vec<Node>, Error> {
    let mut stack = Buffer::default();
    for root in &mut nodes {
        admission.work(1)?;
        if !matches!(root, Node::Element(_)) {
            continue;
        }
        let Node::Element(mut element) = std::mem::replace(root, Node::Text(Default::default()))
        else {
            unreachable!();
        };
        let children = transform.enter(&mut element, &context, admission)?;
        stack.push(
            Frame {
                element,
                next: 0,
                parent: context.clone(),
                children,
            },
            admission,
        )?;
        while let Some(frame) = stack.last_mut() {
            if frame.next < frame.element.children.len() {
                admission.work(1)?;
                let at = frame.next;
                frame.next += 1;
                let child = &mut frame.element.children[at];
                if !matches!(child, Node::Element(_)) {
                    continue;
                }
                let Node::Element(mut element) =
                    std::mem::replace(child, Node::Text(Default::default()))
                else {
                    unreachable!();
                };
                let parent = frame.children.clone();
                let children = transform.enter(&mut element, &parent, admission)?;
                stack.push(
                    Frame {
                        element,
                        next: 0,
                        parent,
                        children,
                    },
                    admission,
                )?;
            } else {
                let mut frame = stack.pop().expect("completed transform element");
                transform.sequence(&mut frame.element.children, &frame.children, admission)?;
                transform.leave(&mut frame.element, &frame.parent, admission)?;
                let node = Node::Element(frame.element);
                if let Some(parent) = stack.last_mut() {
                    parent.element.children[parent.next - 1] = node;
                } else {
                    *root = node;
                }
            }
        }
    }
    let mut nodes = nodes.into();
    transform.sequence(&mut nodes, &context, admission)?;
    Ok(nodes.into_vec())
}
