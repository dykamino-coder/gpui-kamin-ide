//! Итеративное чтение исходного DOM; память зависит от глубины, а не ширины.

use markup5ever_rcdom::{Handle, NodeData};

pub(crate) fn descendants(root: &Handle, templates: bool) -> SourceWalk {
    SourceWalk {
        frames: vec![(root.clone(), 0, false)],
        first: true,
        templates,
    }
}

pub(crate) struct SourceWalk {
    frames: Vec<(Handle, usize, bool)>,
    first: bool,
    templates: bool,
}

impl Iterator for SourceWalk {
    type Item = Handle;

    fn next(&mut self) -> Option<Handle> {
        if self.first {
            self.first = false;
            return self.frames.first().map(|frame| frame.0.clone());
        }
        while let Some((node, next, template_seen)) = self.frames.last_mut() {
            let child = node.children.borrow().get(*next).cloned();
            let child = if child.is_some() {
                *next += 1;
                child
            } else if self.templates && !*template_seen {
                *template_seen = true;
                match &node.data {
                    NodeData::Element {
                        template_contents, ..
                    } => template_contents.borrow().clone(),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(child) = child {
                self.frames.push((child.clone(), 0, false));
                return Some(child);
            }
            self.frames.pop();
        }
        None
    }
}
