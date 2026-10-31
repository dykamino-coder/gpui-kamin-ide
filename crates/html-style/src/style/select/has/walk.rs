//! Обход исходного дерева :has(): общие паспорта и только глубина в стеке.

use crate::budget::{
    LiveReservation,
    admission::{Buffer, Error},
};
use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Ancestor, census_of};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

type Visit = (Handle, Rc<Vec<Ancestor>>, usize);
enum Work {
    Level(Handle),
    Visit(Handle, Rc<Vec<Ancestor>>, usize),
    Descend(Handle),
    Restore(usize),
}

pub(super) struct Walk {
    work: Buffer<Work>,
    pub(super) depth: usize,
    pub(super) leases: Buffer<Option<LiveReservation>>,
}

impl Walk {
    pub(super) fn new(root: &Handle, tree: &TreeIndex) -> Result<Self, Error> {
        let mut work = Buffer::default();
        work.push(Work::Level(root.clone()), &tree.admission())?;
        Ok(Self {
            work,
            depth: 0,
            leases: Buffer::default(),
        })
    }

    pub(super) fn next(&mut self, tree: &TreeIndex) -> Result<Option<Visit>, Error> {
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
                    let lease = super::storage::level(&children, &admission, tree)?;
                    self.leases.push(lease, &admission)?;
                    let (spots, all) = census_of(&children, tree);
                    let all = Rc::new(all);
                    let mut pos = all.len();
                    for (child, spot) in children.iter().zip(spots).rev() {
                        if spot.index == 0 {
                            continue;
                        }
                        pos -= 1;
                        self.work
                            .push(Work::Visit(child.clone(), all.clone(), pos), &admission)?;
                    }
                }
                Work::Visit(handle, all, pos) => {
                    self.work.push(Work::Descend(handle.clone()), &admission)?;
                    return Ok(Some((handle, all, pos)));
                }
            }
        }
        Ok(None)
    }
}
