//! The stack as the measure callback prepared it, kept for the prepaint of the
//! same frame: the column content is built and laid out once (css-multicol-1
//! §3 needs the column width first, and the width is known in the callback).

use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::RetainedLayout;

/// What `Recipe::prepare` left in the measured stack, with the layout engine
/// its children were laid out in.
pub(crate) struct Prepared {
    col_w: f32,
    engine: RetainedLayout,
    children: Vec<StackChild>,
    geometry: std::rc::Rc<crate::layout::positioned::containing_block::metrics::Snapshot>,
    forced_h: Option<f32>,
    balance_failure: Option<crate::layout::fragment::token::Error>,
    copies: usize,
}

impl Prepared {
    /// Takes the prepared state out of the measured `stack`.
    pub(super) fn of(stack: &mut ColumnStack, engine: RetainedLayout) -> Self {
        Self {
            col_w: stack.col_w.get(),
            engine,
            children: std::mem::take(&mut stack.children),
            geometry: std::mem::take(&mut stack.oof.geometry),
            forced_h: stack.forced_h,
            balance_failure: stack.balance_failure.take(),
            copies: stack.copies,
        }
    }

    /// The column width the stack was prepared at.
    pub(super) fn col_w(&self) -> f32 {
        self.col_w
    }

    /// Puts the prepared state into the frame's `stack`; the children keep
    /// their layout in the returned engine.
    pub(crate) fn adopt(self, stack: &mut ColumnStack) -> RetainedLayout {
        stack.col_w.set(self.col_w);
        stack.children = self.children;
        stack.oof.geometry = self.geometry;
        stack.forced_h = self.forced_h;
        stack.balance_failure = self.balance_failure;
        stack.copies = self.copies;
        self.engine
    }
}
