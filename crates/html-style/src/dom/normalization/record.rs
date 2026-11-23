//! Shared current pass records; weak input marks cannot retain obsolete raw trees.
use crate::budget::LiveReservation;
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Children, ChildrenMark};
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

pub(super) struct Recording<C> {
    input: Children,
    pub branches: Buffer<Rc<Segment<C>>>,
    old: Option<Rc<Segment<C>>>,
    lookup: HashMap<usize, usize>,
    _lookup_lease: Option<LiveReservation>,
}

impl<C: Clone + PartialEq> Recording<C> {
    pub fn new(
        input: Children,
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
            branches: Buffer::default(),
            old,
            lookup,
            _lookup_lease: lease,
        })
    }

    pub fn previous(&self, input: &Children) -> Option<Rc<Segment<C>>> {
        let old = self.old.as_ref()?;
        let branch = &old.branches[*self.lookup.get(&input.identity())?];
        branch.input.holds(input).then(|| branch.clone())
    }

    pub fn finish(
        self,
        context: C,
        output: Children,
        admission: &Admission,
    ) -> Result<Rc<Segment<C>>, Error> {
        let lease =
            admission.storage(mem::size_of::<Segment<C>>() + 2 * mem::size_of::<usize>())?;
        Ok(Rc::new(Segment {
            input: self.input.mark(),
            identity: self.input.identity(),
            context,
            output,
            branches: self.branches,
            _lease: lease,
        }))
    }
}
