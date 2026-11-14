//! Обход исходного дерева селекторов: preorder без создания паспортов.

use crate::budget::admission::{Buffer, Error};
use crate::dom::tree_index::TreeIndex;
use markup5ever_rcdom::{Handle, NodeData};

enum Work {
    Level(Handle),
    Visit(Handle, usize),
    Descend(Handle),
    Restore(usize),
}

pub(super) struct Walk {
    work: Buffer<Work>,
    pub(super) depth: usize,
}

impl Walk {
    pub(super) fn new(root: &Handle, tree: &TreeIndex) -> Result<Self, Error> {
        let mut work = Buffer::default();
        work.push(Work::Level(root.clone()), &tree.admission())?;
        Ok(Self { work, depth: 0 })
    }

    pub(super) fn next(&mut self, tree: &TreeIndex) -> Result<Option<(Handle, usize)>, Error> {
        let admission = tree.admission();
        while let Some(work) = self.work.pop() {
            admission.work(1)?;
            match work {
                Work::Restore(depth) => self.depth = depth,
                Work::Descend(handle) => {
                    self.work.push(Work::Restore(self.depth), &admission)?;
                    self.depth += 1;
                    self.work.push(Work::Level(handle), &admission)?;
                }
                Work::Level(handle) => {
                    let children = handle.children.borrow();
                    admission.work(children.len())?;
                    let mut pos = children
                        .iter()
                        .filter(|c| matches!(c.data, NodeData::Element { .. }))
                        .count();
                    for child in children.iter().rev() {
                        if !matches!(child.data, NodeData::Element { .. }) {
                            continue;
                        }
                        pos -= 1;
                        self.work
                            .push(Work::Visit(child.clone(), pos), &admission)?;
                    }
                }
                Work::Visit(handle, pos) => {
                    self.work.push(Work::Descend(handle.clone()), &admission)?;
                    return Ok(Some((handle, pos)));
                }
            }
        }
        Ok(None)
    }
}
