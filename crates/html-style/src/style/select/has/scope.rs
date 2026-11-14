//! Область селекторов: preorder-связи, индекс признаков и ленивые паспорта братьев.

use super::*;
use crate::budget::admission::Buffer;
use std::{cell::OnceCell, rc::Rc};

#[derive(Default)]
struct Level(OnceCell<Passports>);

struct Passports {
    all: Rc<Vec<Ancestor>>,
    _lease: Option<crate::budget::LiveReservation>,
    _marks: Option<crate::budget::LiveReservation>,
}

pub(in crate::style::select) struct Record {
    pub handle: Handle,
    pub pos: usize,
    pub parent: Option<usize>,
    pub previous: Option<usize>,
    pub end: usize,
    last_child: Option<usize>,
    children: Level,
}

pub(in crate::style::select) struct Scope<'a> {
    pub records: Buffer<Record>,
    initial: &'a [Ancestor],
    root: Handle,
    top: Level,
    pub index: super::index::Index,
}

impl<'a> Scope<'a> {
    pub fn new(root: &Handle, path: &'a [Ancestor], tree: &TreeIndex) -> Result<Self, Error> {
        let admission = tree.admission();
        let mut index = super::index::Index::default();
        let mut records: Buffer<Record> = Buffer::default();
        let mut stack: Buffer<usize> = Buffer::default();
        let mut walk = Walk::new(root, tree)?;
        let mut last_top = None;
        while let Some((handle, pos)) = walk.next(tree)? {
            while stack.len() > walk.depth {
                let i = stack.pop().expect("open ancestor");
                records[i].end = records.len();
            }
            let parent = stack.last().copied();
            let at = records.len();
            let previous = if let Some(parent) = parent {
                records[parent].last_child.replace(at)
            } else {
                last_top.replace(at)
            };
            index.insert_handle(&handle, at, &admission)?;
            records.push(
                Record {
                    handle,
                    pos,
                    parent,
                    previous,
                    end: 0,
                    last_child: None,
                    children: Level::default(),
                },
                &admission,
            )?;
            stack.push(at, &admission)?;
        }
        while let Some(i) = stack.pop() {
            records[i].end = records.len();
        }
        Ok(Self {
            records,
            initial: path,
            root: root.clone(),
            top: Level::default(),
            index,
        })
    }

    pub fn all_at(&self, i: usize, tree: &TreeIndex) -> Result<&Rc<Vec<Ancestor>>, Error> {
        let (level, handle) = self.records[i].parent.map_or((&self.top, &self.root), |p| {
            (&self.records[p].children, &self.records[p].handle)
        });
        if level.0.get().is_none() {
            let children = handle.children.borrow();
            let lease = super::storage::level(&children, &tree.admission(), tree)?;
            let (_, all) = super::super::census_of(&children, tree);
            let _ = level.0.set(Passports {
                all: Rc::new(all),
                _lease: lease,
                _marks: None,
            });
        }
        Ok(&level.0.get().expect("initialized sibling level").all)
    }

    /// Временные Rc-копии предыдущей фазы уже отпущены. Новые отметки
    /// оплачены отдельно; строки и признаки не переписываются.
    pub fn refresh_marks(&mut self, tree: &TreeIndex) -> Result<(), Error> {
        let admission = tree.admission();
        for level in
            std::iter::once(&mut self.top).chain(self.records.iter_mut().map(|r| &mut r.children))
        {
            let Some(passports) = level.0.get_mut() else {
                continue;
            };
            let all = Rc::get_mut(&mut passports.all).expect("phase owns sibling passports");
            admission.work(all.len())?;
            let bytes = all.iter().try_fold(0usize, |bytes, node| {
                let count = tree.has_marks.get(&node.key).map_or(0, Vec::len);
                count
                    .checked_mul(std::mem::size_of::<u64>())
                    .and_then(|n| bytes.checked_add(n))
                    .ok_or(Error::CapacityOverflow)
            })?;
            let lease = admission.storage(bytes)?;
            for node in all {
                node.has_marks.clear();
                if let Some(marks) = tree.has_marks.get(&node.key) {
                    node.has_marks
                        .try_reserve_exact(marks.len())
                        .map_err(|_| Error::Allocation)?;
                    node.has_marks.extend_from_slice(marks);
                }
            }
            passports._marks = lease;
        }
        Ok(())
    }

    pub fn match_local(
        &self,
        sel: &Selector,
        index: usize,
        tree: &TreeIndex,
    ) -> Result<bool, Error> {
        let record = &self.records[index];
        let all = self.all_at(index, tree)?;
        tree.admission().work(1)?;
        Ok(super::super::matching::matches_local(
            sel,
            &all[record.pos],
            &[],
            Sibs {
                all,
                pos: record.pos,
                is_elem: true,
                rc: Some(all),
            },
        ))
    }

    pub fn match_at(&self, sel: &Selector, index: usize, tree: &TreeIndex) -> Result<bool, Error> {
        let admission = tree.admission();
        let record = &self.records[index];
        let all = self.all_at(index, tree)?;
        let sibs = Sibs {
            all,
            pos: record.pos,
            is_elem: true,
            rc: Some(all),
        };
        if sel.prev.is_none() && sel.ancestor.is_none() && super::super::sets::eligible(sel) {
            admission.work(1)?;
            return Ok(matches(sel, &all[record.pos], &[], sibs));
        }
        let mut chain = Buffer::default();
        let mut parent = record.parent;
        while let Some(i) = parent {
            chain.push(i, &admission)?;
            parent = self.records[i].parent;
        }
        let mut path = Buffer::default();
        for ancestor in self.initial {
            path.push(ancestor.clone(), &admission)?;
        }
        for &i in chain.iter().rev() {
            let r = &self.records[i];
            let all = self.all_at(i, tree)?;
            let mut ancestor = all[r.pos].clone();
            ancestor.peers = Some((all.clone(), r.pos));
            path.push(ancestor, &admission)?;
        }
        admission.work(path.len() + 1)?;
        Ok(matches(sel, &all[record.pos], &path, sibs))
    }

    pub fn previous(&self, i: usize) -> Option<usize> {
        self.records[i].previous
    }
}
