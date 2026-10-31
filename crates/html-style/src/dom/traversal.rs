//! Обходы стилизованного DOM: курсор братьев и inherited-состояние на уровень.

use super::{Element, Node};
use crate::budget::admission::{Admission, Buffer, Error};

/// Прямой порядок. `None` запрещает спуск, но не посещение следующего брата.
pub fn preorder<C>(
    admission: &Admission,
    nodes: &mut [Node],
    inherited: C,
    mut enter: impl FnMut(&mut Element, &C) -> Option<C>,
) -> Result<(), Error> {
    preorder_try(admission, nodes, inherited, |e, parent| {
        Ok(enter(e, parent))
    })
}

pub fn preorder_try<C>(
    admission: &Admission,
    nodes: &mut [Node],
    inherited: C,
    mut enter: impl FnMut(&mut Element, &C) -> Result<Option<C>, Error>,
) -> Result<(), Error> {
    let mut stack = Buffer::default();
    stack.push((nodes.iter_mut(), inherited), admission)?;
    while let Some((siblings, parent)) = stack.last_mut() {
        let Some(node) = siblings.next() else {
            stack.pop();
            continue;
        };
        admission.work(1)?;
        if let Node::Element(element) = node
            && let Some(next) = enter(element, parent)?
        {
            stack.push((element.children.iter_mut(), next), admission)?;
        }
    }
    Ok(())
}

/// Обратный порядок. Владение кадром позволяет менять весь элемент после детей
/// без aliasing и без повторного поиска пути от корня. Списки детей не копируются.
pub fn postorder(
    admission: &Admission,
    nodes: &mut [Node],
    mut leave: impl FnMut(&mut Element),
) -> Result<(), Error> {
    postorder_try(admission, nodes, |element| {
        leave(element);
        Ok(())
    })
}

pub fn postorder_try(
    admission: &Admission,
    nodes: &mut [Node],
    mut leave: impl FnMut(&mut Element) -> Result<(), Error>,
) -> Result<(), Error> {
    postorder_inherited(admission, nodes, (), |_, _| (), |element, _| leave(element))
}

/// Обратный порядок с контекстом, вычисленным до посещения детей.
pub fn postorder_inherited<C: Clone>(
    admission: &Admission,
    nodes: &mut [Node],
    inherited: C,
    mut enter: impl FnMut(&Element, &C) -> C,
    mut leave: impl FnMut(&mut Element, &C) -> Result<(), Error>,
) -> Result<(), Error> {
    struct Frame<C> {
        element: Element,
        next: usize,
        parent: C,
        children: C,
    }
    let mut stack: Buffer<Frame<C>> = Buffer::default();
    for root in nodes {
        admission.work(1)?;
        if !matches!(root, Node::Element(_)) {
            continue;
        }
        let Node::Element(element) = std::mem::replace(root, Node::Text(String::new())) else {
            unreachable!();
        };
        let children = enter(&element, &inherited);
        stack.push(
            Frame {
                element,
                next: 0,
                parent: inherited.clone(),
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
                if matches!(child, Node::Element(_)) {
                    let Node::Element(element) =
                        std::mem::replace(child, Node::Text(String::new()))
                    else {
                        unreachable!();
                    };
                    let parent = frame.children.clone();
                    let children = enter(&element, &parent);
                    stack.push(
                        Frame {
                            element,
                            next: 0,
                            parent,
                            children,
                        },
                        admission,
                    )?;
                }
            } else {
                let mut frame = stack.pop().expect("active DOM frame");
                leave(&mut frame.element, &frame.parent)?;
                let node = Node::Element(frame.element);
                if let Some(parent) = stack.last_mut() {
                    parent.element.children[parent.next - 1] = node;
                } else {
                    *root = node;
                }
            }
        }
    }
    Ok(())
}

/// Прямой порядок чтения с возможностью пропустить поддерево.
pub fn nodes<'a>(
    admission: &'a Admission,
    roots: &'a [Node],
    descend: impl Fn(&Element) -> bool + 'a,
) -> impl Iterator<Item = Result<&'a Node, Error>> {
    let mut stack = Buffer::default();
    let mut failure = stack.push(roots.iter(), admission).err();
    std::iter::from_fn(move || {
        loop {
            if let Some(error) = failure.take() {
                return Some(Err(error));
            }
            let siblings = stack.last_mut()?;
            let Some(node) = siblings.next() else {
                stack.pop();
                continue;
            };
            if let Err(error) = admission.work(1) {
                while stack.pop().is_some() {}
                return Some(Err(error));
            }
            if let Node::Element(element) = node
                && descend(element)
            {
                if let Err(error) = stack.push(element.children.iter(), admission) {
                    while stack.pop().is_some() {}
                    return Some(Err(error));
                }
            }
            return Some(Ok(node));
        }
    })
}

#[cfg(test)]
mod tests;
