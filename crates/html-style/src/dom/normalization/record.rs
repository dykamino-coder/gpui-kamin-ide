//! Shared current pass records; weak input marks cannot retain obsolete raw trees.
use crate::budget::LiveReservation;
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Children, ChildrenMark, Element, Node};
use std::{collections::HashMap, mem, rc::Rc};

pub(super) struct Segment<C> {
    input: ChildrenMark,
    identity: usize,
    context: C,
    pub output: Children,
    branches: Buffer<Rc<Segment<C>>>,
    _lease: Option<LiveReservation>,
}

impl<C: PartialEq> Segment<C> {
    pub fn matches(&self, input: &Children, context: &C) -> bool {
        self.identity == input.identity() && self.context == *context && self.input.holds(input)
    }
}

impl<C> Drop for Segment<C> {
    fn drop(&mut self) {
        // Shared branches belong to another current/staged pass. Only the
        // uniquely owned suffix is drained, without a recursive Rc destructor.
        let mut pending = Vec::new();
        while let Some(branch) = self.branches.pop() {
            pending.push(branch);
        }
        while let Some(branch) = pending.pop() {
            if let Ok(mut segment) = Rc::try_unwrap(branch) {
                while let Some(child) = segment.branches.pop() {
                    pending.push(child);
                }
            }
        }
    }
}

pub(super) struct Frame<C> {
    pub input: Children,
    pub context: C,
    pub next: usize,
    pub pending: Option<Element>,
    pub branches: Buffer<Rc<Segment<C>>>,
    old: Option<Rc<Segment<C>>>,
    lookup: HashMap<usize, usize>,
    changed: Option<Vec<Node>>,
    output_lease: Option<LiveReservation>,
    _lookup_lease: Option<LiveReservation>,
}

impl<C: Clone + PartialEq> Frame<C> {
    pub fn new(
        input: Children,
        context: C,
        old: Option<Rc<Segment<C>>>,
        admission: &Admission,
    ) -> Result<Self, Error> {
        let count = old.as_ref().map_or(0, |old| old.branches.len());
        let bytes = count
            .checked_mul(2 * mem::size_of::<(usize, usize)>())
            .ok_or(Error::CapacityOverflow)?;
        let lease = admission.storage(bytes)?;
        admission.work(count)?;
        let mut lookup = HashMap::new();
        lookup.try_reserve(count).map_err(|_| Error::Allocation)?;
        if let Some(old) = &old {
            for (index, branch) in old.branches.iter().enumerate() {
                lookup.insert(branch.identity, index);
            }
        }
        Ok(Self {
            input,
            context,
            next: 0,
            pending: None,
            branches: Buffer::default(),
            old,
            lookup,
            changed: None,
            output_lease: None,
            _lookup_lease: lease,
        })
    }

    pub fn previous(&self, input: &Children) -> Option<Rc<Segment<C>>> {
        let old = self.old.as_ref()?;
        let branch = &old.branches[*self.lookup.get(&input.identity())?];
        branch.input.holds(input).then(|| branch.clone())
    }

    pub fn append(&mut self, changed: Option<Element>, admission: &Admission) -> Result<(), Error> {
        let index = self.next - 1;
        if changed.is_some() && self.changed.is_none() {
            let bytes = Children::storage_bytes(self.input.len())?;
            self.output_lease = admission.storage(bytes)?;
            admission.work(index)?;
            let mut nodes = Vec::new();
            nodes
                .try_reserve_exact(self.input.len())
                .map_err(|_| Error::Allocation)?;
            nodes.extend_from_slice(&self.input[..index]);
            self.changed = Some(nodes);
        }
        if let Some(nodes) = &mut self.changed {
            nodes.push(changed.map_or_else(|| self.input[index].clone(), Node::Element));
        }
        Ok(())
    }

    pub fn output(&mut self) -> Children {
        match self.changed.take() {
            Some(nodes) => Children::from_reserved(nodes, self.output_lease.take()),
            None => self.input.clone(),
        }
    }

    pub fn finish(self, output: Children, admission: &Admission) -> Result<Rc<Segment<C>>, Error> {
        let lease =
            admission.storage(mem::size_of::<Segment<C>>() + 2 * mem::size_of::<usize>())?;
        Ok(Rc::new(Segment {
            input: self.input.mark(),
            identity: self.input.identity(),
            context: self.context,
            output,
            branches: self.branches,
            _lease: lease,
        }))
    }
}
