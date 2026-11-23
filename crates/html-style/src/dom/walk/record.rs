//! Capture raw output at the same enter/leave boundaries that perform cascade.
use super::{pending::Pending, sibling_survey::Visit};
use crate::budget::admission::{Admission, Error};
use crate::dom::source_memo::{
    CascadeContext, CascadeEffects, CascadeEntry, EntryInputs, GeneratedContext, GroupInput,
    MemoUpdate, OutputOwnership, RootOutput, SourceMemo,
};
use crate::dom::{Node, NodeKey};
use crate::style::{
    cascade::sharing::Environment, generated::counters::Counters, select::Ancestor,
};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

pub(super) struct Start {
    handle: Handle,
    inputs: EntryInputs,
    first: usize,
    parent: Option<NodeKey>,
}

struct Owner {
    node: NodeKey,
    children: Vec<NodeKey>,
    sequence: Option<NodeKey>,
}

#[derive(Default)]
pub(in crate::dom) struct Recorder<'a, 'm> {
    resume: Option<super::resume::Resume<'a, 'm>>,
    memo: MemoUpdate,
    active: HashMap<NodeKey, Rc<CascadeContext>>,
    owners: Vec<Owner>,
    roots: Vec<NodeKey>,
    environment: Option<Rc<Environment>>,
}

fn group(path: &[Ancestor]) -> Option<GroupInput> {
    path.iter().rev().find_map(|ancestor| {
        ancestor.target_group.as_ref().map(|selected| GroupInput {
            node: ancestor.key.handle().key(),
            selected: selected.get(),
        })
    })
}

impl<'a, 'm> Recorder<'a, 'm> {
    pub fn retained(
        stage: &'a crate::dom::source_memo::MemoStage<'m>,
        bindings: &'a mut dyn crate::dom::source_memo::ResumeBindings,
    ) -> Self {
        Self {
            resume: Some(super::resume::Resume { stage, bindings }),
            ..Self::default()
        }
    }

    pub(super) fn reuse(
        &mut self,
        start: &Start,
        path: &[Ancestor],
        counters: &mut Counters,
        out: &mut super::kids::Kids,
        vcx: crate::values::ValueCx<'_>,
    ) -> Result<bool, Error> {
        self.resume.as_mut().map_or(Ok(false), |resume| {
            resume.append(start.handle.key(), &start.inputs, path, counters, out, vcx)
        })
    }

    fn environment(&mut self, vcx: crate::values::ValueCx<'_>) -> Rc<Environment> {
        if self
            .environment
            .as_ref()
            .is_none_or(|old| !old.matches(vcx))
        {
            self.environment = Some(Rc::new(Environment::new(vcx)));
        }
        self.environment
            .as_ref()
            .expect("captured environment")
            .clone()
    }

    pub(super) fn begin(
        &mut self,
        visit: &Visit<'_>,
        path: &[Ancestor],
        counters: &Counters,
        first: usize,
    ) -> Result<Start, Error> {
        let parent = self.owners.last().map(|owner| owner.node);
        if let Some(owner) = self.owners.last_mut() {
            owner
                .children
                .try_reserve(1)
                .map_err(|_| Error::Allocation)?;
            owner.children.push(visit.handle().key());
        } else {
            self.roots.try_reserve(1).map_err(|_| Error::Allocation)?;
            self.roots.push(visit.handle().key());
        }
        Ok(Start {
            handle: visit.handle().clone(),
            inputs: EntryInputs {
                parent: path
                    .last()
                    .and_then(|p| self.active.get(&p.key.handle().key()))
                    .cloned(),
                scope: visit.scope.clone(),
                vars: visit.vars.clone(),
                preserve: visit.preserve,
                depth: visit.depth,
                environment: self.environment(visit.vcx),
                counters: counters.clone(),
                group: group(path),
            },
            first,
            parent,
        })
    }

    pub(super) fn entered(&mut self, pending: &Pending<'_>, start: &Start) -> Result<(), Error> {
        self.active.try_reserve(1).map_err(|_| Error::Allocation)?;
        self.owners.try_reserve(1).map_err(|_| Error::Allocation)?;
        let node = pending.visit.handle().key();
        self.owners.push(Owner {
            node,
            children: Vec::new(),
            sequence: None,
        });
        let me = &pending.me;
        self.active.insert(
            node,
            Rc::new(CascadeContext {
                parent: start.inputs.parent.clone(),
                node,
                tag: me.tag.clone(),
                attrs: me.attrs.clone(),
                generated: GeneratedContext {
                    counters: me.counter_style.clone(),
                    counter_style: me.counter_context,
                    content: me.content_style.clone(),
                    content_none: me.content_none,
                    language: me.lang.clone(),
                    target_group: me.target_group.is_some(),
                },
            }),
        );
        Ok(())
    }

    pub(super) fn sequence(&mut self, sequence: Option<NodeKey>) {
        self.owners
            .last_mut()
            .expect("entered output owner")
            .sequence = sequence;
    }

    pub(super) fn finish(
        &mut self,
        start: Start,
        path: &[Ancestor],
        counters: &Counters,
        out: &[Node],
        admission: &Admission,
        vcx: crate::values::ValueCx<'_>,
    ) -> Result<(), Error> {
        let node = start.handle.key();
        self.active.remove(&node);
        let parent = start.handle.parent.take();
        let source_parent = parent.as_ref().and_then(|p| p.upgrade()).map(|p| p.key());
        start.handle.parent.set(parent);
        let owner = self
            .owners
            .last()
            .is_some_and(|owner| owner.node == node)
            .then(|| self.owners.pop().expect("entered output owner"));
        let (children, sequence) = owner.map_or_else(
            || (Vec::new(), None),
            |owner| (owner.children, owner.sequence),
        );
        let ownership = OutputOwnership {
            parent: start.parent,
            children,
            sequence,
        };
        let selects_group = start
            .inputs
            .group
            .filter(|old| !old.selected)
            .filter(|old| group(path).is_some_and(|new| new.node == old.node && new.selected))
            .map(|old| old.node);
        let effects =
            CascadeEffects::capture(&start.inputs.environment, vcx, counters, selects_group);
        let entry = CascadeEntry::new(
            node,
            source_parent,
            ownership,
            start.inputs,
            copy_nodes(&out[start.first..])?.into(),
            effects,
            admission,
        )?;
        self.memo.replace(entry)
    }

    pub fn finish_document(self, source: NodeKey, out: &[Node]) -> Result<SourceMemo, Error> {
        let mut memo = SourceMemo::default();
        let (update, output) = self.finish_update(source, out)?;
        memo.publish(update, output)?;
        Ok(memo)
    }

    pub fn finish_update(
        self,
        source: NodeKey,
        out: &[Node],
    ) -> Result<(MemoUpdate, RootOutput), Error> {
        Ok((
            self.memo,
            RootOutput {
                raw: copy_nodes(out)?.into(),
                ownership: OutputOwnership {
                    parent: None,
                    children: self.roots,
                    sequence: Some(source),
                },
            },
        ))
    }
}

fn copy_nodes(nodes: &[Node]) -> Result<Vec<Node>, Error> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(nodes.len())
        .map_err(|_| Error::Allocation)?;
    copied.extend_from_slice(nodes);
    Ok(copied)
}
