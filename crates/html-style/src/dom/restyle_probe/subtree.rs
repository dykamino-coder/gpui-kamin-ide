//! The boxes of the subtrees an edit rebuilds (`walk::subtree`), entered
//! with the probe's rules, `:has()` marks and kept contexts, and the passes
//! of a full build over a list of boxes (`finish_document`).

use super::{RestyleCache, RestyleProbe};
use crate::dom::Node;
use crate::dom::input::Admitted;
use crate::dom::outcome::DocumentFailure;
use crate::dom::tree_index::{HandleKey, TreeIndex};
use crate::dom::walk::{self, Subtree};
use crate::values::DocumentValues;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

impl RestyleCache {
    /// The subtree of each of `roots` at the current state of the source DOM,
    /// as the walk of a full build gives it (`walk::subtree`). `mutated` —
    /// the elements of the batch: a kept context whose ancestor records hold
    /// one of them holds its attributes in one state of the DOM only, and the
    /// walk enters the chain again.
    pub fn subtrees(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        roots: &[Handle],
        mutated: &[Handle],
    ) -> Result<Vec<Option<Subtree>>, DocumentFailure> {
        self.walks(probe, input, values, roots, mutated, walk::subtree)
    }

    /// The box of each of `roots` with its generated boxes and no children
    /// (`walk::shallow`), as `subtrees` enters it.
    pub fn shallows(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        roots: &[Handle],
        mutated: &[Handle],
    ) -> Result<Vec<Option<Vec<Node>>>, DocumentFailure> {
        self.walks(probe, input, values, roots, mutated, walk::shallow)
    }

    fn walks<T>(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
        roots: &[Handle],
        mutated: &[Handle],
        walk: impl for<'v> Fn(
            &Handle,
            &TreeIndex,
            &Rc<crate::dom::shadow::Scope>,
            &Handle,
            &mut walk::Chains,
            crate::values::ValueCx<'v>,
        ) -> Result<Option<T>, crate::budget::admission::Error>,
    ) -> Result<Vec<Option<T>>, DocumentFailure> {
        let mut tree = TreeIndex::new(values.clone(), input.xhtml);
        let document = &input.dom.document;
        self.chains.forget_parents(&self.mutated);
        let holding = mutated.iter().map(HandleKey::of).collect();
        self.chains.forget_marked(&holding);
        if !probe.has_args.is_empty() {
            self.marks(document, &probe.has_args, &mut tree)?;
        }
        let vcx = values.values().with_scheme(probe.scheme);
        let mut built = Vec::with_capacity(roots.len());
        for root in roots {
            built.push(walk(
                document,
                &tree,
                &probe.doc,
                root,
                &mut self.chains,
                vcx,
            )?);
        }
        if !probe.has_args.is_empty() {
            self.has = Some(tree.take_has());
        }
        Ok(built)
    }
}

/// The passes a full build makes over its boxes after the walk
/// (`finish_document`), over `nodes`: the boxes of a subtree under copies of
/// its ancestors' boxes.
pub fn finish_boxes(
    values: &Rc<DocumentValues>,
    xhtml: bool,
    nodes: Vec<Node>,
) -> Result<Vec<Node>, DocumentFailure> {
    let tree = TreeIndex::new(values.clone(), xhtml);
    let admission = tree.admission();
    crate::dom::finish_document::finish(&tree, &admission, nodes, values.values())
}
