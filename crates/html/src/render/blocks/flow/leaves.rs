//! The flow's part in standing far blocks as their leaves (`layout::reuse::offscreen`).

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::layout::reuse::{Far, FlowReuse};
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::rc::Rc;

/// The leaves of one block list: floats of the list reach only its later
/// blocks and its parent's, which learns of them from the container.
pub(super) struct Leaves<'a> {
    reuse: Option<&'a Rc<FlowReuse>>,
    floated: bool,
    /// The root flow keeps its far blocks in its own records (`FlowCx`):
    /// its children stand built.
    root: bool,
}

/// A child decided by `Leaves::leaf`: standing as its leaf, or to be built.
pub(super) enum Child {
    Leaf,
    Build(Option<Far>),
}

impl<'a> Leaves<'a> {
    pub(super) fn new(opts: &'a RenderOpts, root: bool) -> Self {
        let reuse = opts.cx.contracts.reuse.as_ref();
        let floated = reuse.is_some_and(|r| r.floated());
        Self {
            reuse,
            floated,
            root,
        }
    }

    /// Where the next child starts at least (`FlowReuse::above`).
    pub(super) fn before(&self) -> Option<f32> {
        self.reuse.map(|r| r.above())
    }

    /// Child `n` was built (or placed) from `before` into `out` from `start`
    /// (`None` — an inline node of a run).
    pub(super) fn built(
        &self,
        n: &Node,
        before: Option<f32>,
        out: Option<(&mut Vec<AnyElement>, usize)>,
        opts: &RenderOpts,
    ) {
        if let Some(reuse) = self.reuse {
            let zone = out.and_then(|(out, start)| match out.len() == start + 1 {
                true => out.last_mut(),
                false => None,
            });
            reuse.built_block(n, before, zone, opts);
        }
    }

    /// Block child `e`: its leaf goes to `out` if it lies far from the screen.
    pub(super) fn leaf(
        &self,
        e: &Element,
        edge: bool,
        inherited: &BoxStyle,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) -> Child {
        let reuse = self.reuse.filter(|_| !self.root);
        match reuse.and_then(|r| r.offscreen_leaf(e, edge, inherited, opts)) {
            Some(Far::Leaf(el)) => {
                out.push(el);
                Child::Leaf
            }
            far => Child::Build(far),
        }
    }

    /// Wrap the element `far` watches (`Far::wrap`), built from `start` in `out`.
    pub(super) fn wrap(&self, far: Option<Far>, start: usize, out: &mut Vec<AnyElement>) {
        if let Some(far) = far {
            far.wrap(start, out);
        }
    }

    /// An inline run `nodes` of the list: its text pushes later blocks down.
    /// Its paragraph is built while the guard lives (`FlowReuse::inline`).
    pub(super) fn run(&self, nodes: &[Node], opts: &RenderOpts) -> Option<impl Drop + 'a> {
        let reuse = self.reuse?;
        reuse.note_run(nodes, opts);
        Some(reuse.inline())
    }

    /// The list is built: the floats seen before it hold again.
    pub(super) fn end(self) {
        if let Some(reuse) = self.reuse {
            reuse.restore_floated(self.floated);
        }
    }
}
