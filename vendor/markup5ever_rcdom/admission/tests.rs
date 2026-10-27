//! Refusal at every internal operation frees partial trees and temporary work.

use super::*;
use crate::{Attribute, QualName};
use markup5ever::{
    interface::tree_builder::{ElementFlags, NodeOrText},
    local_name, ns,
};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Error {
    Refused,
    Allocation,
    Overflow,
}

struct Ticket {
    live: Arc<AtomicUsize>,
    bytes: usize,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.live.fetch_sub(self.bytes, Relaxed);
    }
}

struct Policy {
    live: Arc<AtomicUsize>,
    steps: Cell<usize>,
    fail: usize,
}
impl Policy {
    fn new(fail: usize) -> Self {
        Self {
            live: Arc::new(AtomicUsize::new(0)),
            steps: Cell::new(0),
            fail,
        }
    }
    fn step(&self) -> Result<(), Error> {
        let step = self.steps.get() + 1;
        self.steps.set(step);
        if step == self.fail {
            Err(Error::Refused)
        } else {
            Ok(())
        }
    }
    fn ticket(&self, bytes: usize) -> Result<Arc<dyn Any + Send + Sync>, Error> {
        self.step()?;
        self.live.fetch_add(bytes, Relaxed);
        Ok(Arc::new(Ticket {
            live: self.live.clone(),
            bytes,
        }))
    }
    fn element(&self, dom: &RcDom, name: &str, attrs: &[(&str, &str)]) -> Result<Handle, Error> {
        let mut flags = ElementFlags::default();
        flags.template = name == "template";
        dom.try_create_element(
            QualName::new(None, ns!(html), name.into()),
            attrs
                .iter()
                .map(|(key, value)| Attribute {
                    name: QualName::new(None, ns!(), (*key).into()),
                    value: (*value).into(),
                })
                .collect(),
            flags,
            self,
        )
    }
}
impl Admission for Policy {
    type Error = Error;
    fn storage(&self, bytes: usize) -> Result<Lease, Error> {
        self.ticket(bytes).map(Some)
    }
    fn node(&self, bytes: usize) -> Result<Lease, Error> {
        self.ticket(bytes).map(Some)
    }
    fn payload(&self, text: usize, attrs: usize) -> Result<Lease, Error> {
        self.ticket(text + attrs).map(Some)
    }
    fn buffer(&self, text: &mut StrTendril, additional: usize) -> Result<(), Error> {
        self.step()?;
        text.try_reserve_with(
            additional.try_into().map_err(|_| Error::Overflow)?,
            |bytes| self.ticket(bytes),
        )
        .map_err(|error| match error {
            tendril::ReserveError::Admission(error) => error,
            tendril::ReserveError::Allocation(_) => Error::Allocation,
            tendril::ReserveError::CapacityOverflow => Error::Overflow,
        })
    }
    fn visit(&self) -> Result<(), Error> {
        self.step()
    }
    fn allocation(&self, _: TryReserveError) -> Error {
        Error::Allocation
    }
    fn overflow(&self) -> Error {
        Error::Overflow
    }
}

fn document(policy: &Policy) -> Result<RcDom, Error> {
    let dom = RcDom::try_new(policy)?;
    let select = policy.element(&dom, "select", &[])?;
    let target = policy.element(&dom, "selectedcontent", &[])?;
    dom.try_append(&select, NodeOrText::AppendNode(target), policy)?;
    for value in ["KEEP", "LOST"] {
        let option = policy.element(&dom, "option", &[("selected", "")])?;
        let template = policy.element(&dom, "template", &[("data-test", "value")])?;
        let NodeData::Element {
            template_contents, ..
        } = &template.data
        else {
            unreachable!()
        };
        dom.try_append(
            template_contents.borrow().as_ref().unwrap(),
            NodeOrText::AppendText(value.into()),
            policy,
        )?;
        dom.try_append(&option, NodeOrText::AppendNode(template.clone()), policy)?;
        dom.try_append(&option, NodeOrText::AppendText(value.into()), policy)?;
        dom.try_append(
            &option,
            NodeOrText::AppendText("-long-enough-for-storage".into()),
            policy,
        )?;
        dom.try_append(&select, NodeOrText::AppendNode(option.clone()), policy)?;
        dom.try_pop(&option, policy)?;
    }
    dom.try_append(&dom.document, NodeOrText::AppendNode(select), policy)?;
    dom.try_parse_error("recovery".into(), policy)?;
    dom.try_finish(policy)
}

#[test]
fn every_internal_refusal_releases_partial_state() {
    let full = Policy::new(usize::MAX);
    let dom = document(&full).unwrap();
    let steps = full.steps.get();
    assert!(steps > 100);
    drop(dom);
    assert_eq!(full.live.load(Relaxed), 0);
    for fail in 1..=steps {
        let policy = Policy::new(fail);
        assert!(
            matches!(document(&policy), Err(Error::Refused)),
            "step {fail}"
        );
        assert_eq!(policy.steps.get(), fail, "work after refusal {fail}");
        assert_eq!(policy.live.load(Relaxed), 0, "leaked reservation at {fail}");
    }
}

#[test]
fn retained_subtree_and_weak_allocation_keep_their_tickets() {
    let policy = Policy::new(usize::MAX);
    let dom = RcDom::try_new(&policy).unwrap();
    let child = policy.element(&dom, "div", &[]).unwrap();
    let weak = crate::WeakHandle::new(&dom.document);
    dom.try_append(
        &child,
        NodeOrText::AppendText("KEEP and LOST".into()),
        &policy,
    )
    .unwrap();
    dom.try_append(
        &dom.document,
        NodeOrText::AppendNode(child.clone()),
        &policy,
    )
    .unwrap();
    drop(dom);
    assert!(weak.upgrade().is_none());
    assert_eq!(child.children.borrow().len(), 1);
    assert!(policy.live.load(Relaxed) > 0);
    drop(child);
    assert!(
        policy.live.load(Relaxed) > 0,
        "weak Rc still owns physical allocation"
    );
    drop(weak);
    assert_eq!(policy.live.load(Relaxed), 0);
}

#[test]
fn selectedcontent_clones_template_storage_and_repairs_parent_edges() {
    let policy = Policy::new(usize::MAX);
    let dom = document(&policy).unwrap();
    let select = dom.document.children.borrow()[0].clone();
    let target = select.children.borrow()[0].clone();
    let selected = select.children.borrow().last().unwrap().clone();
    let copied = target.children.borrow()[0].clone();
    assert!(!Rc::ptr_eq(&copied, &selected.children.borrow()[0]));
    assert!(Rc::ptr_eq(
        &copied.parent().unwrap().upgrade().unwrap(),
        &target
    ));
    let NodeData::Element {
        template_contents, ..
    } = &copied.data
    else {
        unreachable!()
    };
    let fragment = template_contents.borrow().as_ref().unwrap().clone();
    let NodeData::Text { contents } = &fragment.children.borrow()[0].data else {
        unreachable!()
    };
    assert_eq!(&**contents.borrow(), "LOST");
    assert_eq!(
        local_name!("template"),
        match &copied.data {
            NodeData::Element { name, .. } => name.local.clone(),
            _ => unreachable!(),
        }
    );
}
