//! Параметры для коробок внутри текущей, когда коробка может ничего в контексте не менять:
//! тогда потомки читают параметры родителя, а копия (все группы контекста и их общие
//! владельцы) не делается. Blink передаёт потомку тот же `ConstraintSpace`, пока его входы
//! те же; здесь то же правило для `RenderOpts`.

use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;

/// The options of the boxes inside a box: its own when it changes the context, else its
/// parent's.
pub(crate) enum Inner<'a> {
    Same(&'a RenderOpts),
    Changed(Box<RenderOpts>),
}

impl std::ops::Deref for Inner<'_> {
    type Target = RenderOpts;

    fn deref(&self) -> &RenderOpts {
        match self {
            Inner::Same(opts) => opts,
            Inner::Changed(opts) => opts,
        }
    }
}

impl RenderOpts {
    /// [`RenderOpts::inner`] when `changes` says `f` gives the context another value;
    /// the same options otherwise (`f` is not run).
    pub(crate) fn inner_if(&self, changes: bool, f: impl FnOnce(&mut LayoutCx)) -> Inner<'_> {
        if changes {
            Inner::Changed(self.inner(f))
        } else {
            Inner::Same(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> RenderOpts {
        RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: LayoutCx::default(),
            frag: Default::default(),
        }
    }

    #[test]
    fn an_unchanged_context_is_the_parents_own() {
        let parent = opts();
        let inner = parent.inner_if(false, |_| unreachable!("nothing changes"));
        assert!(std::ptr::eq(&*inner, &parent));
    }

    #[test]
    fn a_changed_context_is_a_copy_and_leaves_the_parent_as_it_was() {
        let parent = opts();
        let inner = parent.inner_if(true, |cx| cx.contracts.reflow_depth = 3);
        assert!(!std::ptr::eq(&*inner, &parent));
        assert_eq!(
            (
                inner.cx.contracts.reflow_depth,
                parent.cx.contracts.reflow_depth
            ),
            (3, 0)
        );
    }
}
