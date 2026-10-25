//! Полная перепись братьев для итеративного обхода и :nth-last-child().
use crate::document::state::ValueCx;
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::css::VarMap;
use crate::style::select::{Ancestor, Sibs, Spot, census_of};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

pub(super) struct Survey {
    pub nodes: Vec<Handle>,
    pub spots: Vec<Spot>,
    pub all: Rc<Vec<Ancestor>>,
}
impl Survey {
    pub fn children(handle: &Handle, tree: &TreeIndex) -> Rc<Self> {
        let nodes = handle.children.borrow().clone();
        let (spots, all) = census_of(&nodes, tree);
        Rc::new(Self {
            nodes,
            spots,
            all: Rc::new(all),
        })
    }
}
#[derive(Clone)]
pub(super) struct Visit<'a> {
    pub survey: Rc<Survey>,
    pub index: usize,
    pub pos: usize,
    pub scope: Rc<Scope>,
    pub vars: Rc<VarMap>,
    pub preserve: bool,
    pub depth: usize,
    pub vcx: ValueCx<'a>,
}
impl Visit<'_> {
    pub fn handle(&self) -> &Handle {
        &self.survey.nodes[self.index]
    }
    pub fn sibs(&self) -> Sibs<'_> {
        Sibs {
            all: &self.survey.all,
            pos: self.pos,
            is_elem: self.survey.spots[self.index].index != 0,
            rc: Some(&self.survey.all),
        }
    }
}
