//! Обход исходного дерева с восстановлением единого пути между братьями.

use crate::dom::tree_index::TreeIndex;
use crate::style::select::{Ancestor, census_of};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

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
    pub(super) fn new(root: &Handle, path: &[Ancestor]) -> Self {
        Self {
            work: vec![Work::Level(root.children.borrow().clone())],
            path: path.to_vec(),
        }
    }

    pub(super) fn next(&mut self, tree: &TreeIndex) -> Option<(Handle, Rc<Vec<Ancestor>>, usize)> {
        while let Some(work) = self.work.pop() {
            match work {
                Work::Restore(len) => self.path.truncate(len),
                Work::Descend(handle, me) => {
                    self.work.push(Work::Restore(self.path.len()));
                    self.path.push(*me);
                    self.work
                        .push(Work::Level(handle.children.borrow().clone()));
                }
                Work::Level(children) => {
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
                    return Some((handle, all, pos));
                }
            }
        }
        None
    }
}
