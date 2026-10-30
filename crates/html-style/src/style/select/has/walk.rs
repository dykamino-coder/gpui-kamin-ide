//! Обход исходного дерева с восстановлением единого пути между братьями.

use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Ancestor, census_of};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

type Visit = (Handle, Rc<Vec<Ancestor>>, usize);

enum Work {
    Level(Vec<Handle>),
    Visit(Handle, Rc<Vec<Ancestor>>, usize),
    Descend(Handle, Box<Ancestor>),
    Restore(usize),
}

pub(super) struct Walk {
    work: Vec<Work>,
    pub(super) path: Vec<Ancestor>,
}

impl Walk {
    pub(super) fn new(
        root: &Handle,
        path: &[Ancestor],
        tree: &TreeIndex,
    ) -> Result<Self, crate::budget::admission::Error> {
        tree.admission()
            .work(root.children.borrow().len() + path.len())?;
        Ok(Self {
            work: vec![Work::Level(root.children.borrow().clone())],
            path: path.to_vec(),
        })
    }

    pub(super) fn next(
        &mut self,
        tree: &TreeIndex,
    ) -> Result<Option<Visit>, crate::budget::admission::Error> {
        let admission = tree.admission();
        while let Some(work) = self.work.pop() {
            admission.work(1)?;
            match work {
                Work::Restore(len) => self.path.truncate(len),
                Work::Descend(handle, me) => {
                    admission.work(handle.children.borrow().len())?;
                    self.work.push(Work::Restore(self.path.len()));
                    self.path.push(*me);
                    self.work
                        .push(Work::Level(handle.children.borrow().clone()));
                }
                Work::Level(children) => {
                    admission.work(children.len())?;
                    let (spots, all) = census_of(&children, tree);
                    let all = Rc::new(all);
                    let mut pos = all.len();
                    for (child, spot) in children.into_iter().zip(spots).rev() {
                        if spot.index == 0 {
                            continue;
                        }
                        pos -= 1;
                        self.work.push(Work::Visit(child, all.clone(), pos));
                    }
                }
                Work::Visit(handle, all, pos) => {
                    self.work
                        .push(Work::Descend(handle.clone(), Box::new(all[pos].clone())));
                    return Ok(Some((handle, all, pos)));
                }
            }
        }
        Ok(None)
    }
}
