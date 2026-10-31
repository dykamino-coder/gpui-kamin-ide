//! Однократная перепись области :has(): preorder-интервалы и общие паспорта братьев.

use super::*;
use crate::budget::admission::Buffer;
use std::rc::Rc;

pub(super) struct Record {
    pub handle: Handle,
    pub all: Rc<Vec<Ancestor>>,
    pub pos: usize,
    pub parent: Option<usize>,
    pub end: usize,
}

pub(super) struct Scope<'a> {
    pub records: Buffer<Record>,
    initial: &'a [Ancestor],
    _leases: Buffer<Option<crate::budget::LiveReservation>>,
    pub index: super::index::Index,
}

impl<'a> Scope<'a> {
    pub fn new(root: &Handle, path: &'a [Ancestor], tree: &TreeIndex) -> Result<Self, Error> {
        let admission = tree.admission();
        let mut index = super::index::Index::default();
        let mut records: Buffer<Record> = Buffer::default();
        let mut stack: Buffer<usize> = Buffer::default();
        let mut walk = Walk::new(root, tree)?;
        while let Some((handle, all, pos)) = walk.next(tree)? {
            let depth = walk.depth;
            while stack.len() > depth {
                let i = stack.pop().expect("open ancestor");
                records[i].end = records.len();
            }
            let parent = stack.last().copied();
            let record_index = records.len();
            index.insert(&all[pos], records.len(), &admission)?;
            records.push(
                Record {
                    handle,
                    all,
                    pos,
                    parent,
                    end: 0,
                },
                &admission,
            )?;
            stack.push(record_index, &admission)?;
        }
        while let Some(i) = stack.pop() {
            records[i].end = records.len();
        }
        Ok(Self {
            records,
            initial: path,
            _leases: walk.leases,
            index,
        })
    }

    pub fn match_at(&self, sel: &Selector, index: usize, tree: &TreeIndex) -> Result<bool, Error> {
        let admission = tree.admission();
        let record = &self.records[index];
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
            let mut ancestor = r.all[r.pos].clone();
            ancestor.peers = Some((r.all.clone(), r.pos));
            path.push(ancestor, &admission)?;
        }
        admission.work(path.len() + 1)?;
        Ok(matches(
            sel,
            &record.all[record.pos],
            &path,
            Sibs {
                all: &record.all,
                pos: record.pos,
                is_elem: true,
                rc: Some(&record.all),
            },
        ))
    }

    pub fn previous(&self, i: usize) -> Option<usize> {
        if self.records[i].pos == 0 {
            return None;
        }
        let mut previous = i - 1;
        while self.records[previous].parent != self.records[i].parent {
            previous = self.records[previous].parent?;
        }
        Some(previous)
    }
}
