//! Продолжение сборки: всё состояние ожидающего родителя лежит в куче.
use super::sibling_survey::Visit;
use crate::dom::Node;
use crate::dom::shadow::Shadow;
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, VarMap};
use crate::style::generated::counters::Scope;
use crate::style::select::Ancestor;
use std::rc::Rc;

pub(super) struct Pending<'a> {
    pub visit: Visit<'a>,
    pub me: Ancestor,
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub style: Box<Computed>,
    pub vars: Rc<VarMap>,
    pub shadow: Option<Rc<Shadow>>,
    pub important: Option<DeclBlock>,
    pub hover: Option<Box<Computed>>,
    pub first_letter: Option<Box<Computed>>,
    pub first_line: Option<Box<Computed>>,
    pub list_item: Option<i64>,
    pub box_level: bool,
    pub style_scope: Option<Scope>,
    pub hidden: bool,
    pub children: Vec<Node>,
}
pub(super) enum Restore {
    Truncate(usize),
    Replace(Vec<Ancestor>),
}
impl Restore {
    pub fn apply(self, path: &mut Vec<Ancestor>) {
        match self {
            Self::Truncate(n) => path.truncate(n),
            Self::Replace(p) => *path = p,
        }
    }
}
