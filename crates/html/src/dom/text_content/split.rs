//! Деление DOM по байтовому смещению: один проход с явными кадрами родителей.

use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Element, Node};

/// Смещение измеряется в исходном тексте; разрез сдвигается к концу UTF-8 символа.
pub(crate) fn split_nodes(
    admission: &Admission,
    nodes: &[Node],
    at: usize,
) -> Result<(Vec<Node>, Vec<Node>), Error> {
    struct Frame<'a> {
        nodes: std::slice::Iter<'a, Node>,
        before: Vec<Node>,
        after: Vec<Node>,
        owner: Option<&'a Element>,
        start: usize,
    }
    fn frame<'a>(nodes: &'a [Node], owner: Option<&'a Element>, start: usize) -> Frame<'a> {
        Frame {
            nodes: nodes.iter(),
            before: vec![],
            after: vec![],
            owner,
            start,
        }
    }
    let mut stack = Buffer::default();
    stack.push(frame(nodes, None, 0), admission)?;
    let mut seen = 0usize;
    loop {
        let current = stack.last_mut().expect("active text split");
        let Some(node) = current.nodes.next() else {
            let done = stack.pop().expect("completed text split");
            let Some(owner) = done.owner else {
                return Ok((done.before, done.after));
            };
            let parent = stack.last_mut().expect("text split parent");
            if done.start >= at {
                parent.after.push(Node::Element(owner.clone()));
            } else if seen <= at {
                parent.before.push(Node::Element(owner.clone()));
            } else {
                let mut a = owner.clone();
                a.children = done.before.into();
                let mut b = owner.clone();
                b.children = done.after.into();
                parent.before.push(Node::Element(a));
                parent.after.push(Node::Element(b));
            }
            continue;
        };
        admission.work(1)?;
        match node {
            Node::Text(text) => {
                let end = seen
                    .checked_add(text.len())
                    .ok_or(Error::CapacityOverflow)?;
                if seen >= at {
                    current.after.push(node.clone());
                } else if end <= at {
                    current.before.push(node.clone());
                } else {
                    let mut cut = at - seen;
                    while cut < text.len() && !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    if cut > 0 {
                        current.before.push(Node::Text(text[..cut].into()));
                    }
                    if cut < text.len() {
                        current.after.push(Node::Text(text[cut..].into()));
                    }
                }
                seen = end;
            }
            Node::Element(element) => {
                stack.push(frame(&element.children, Some(element), seen), admission)?
            }
        }
    }
}

#[cfg(test)]
mod tests;
