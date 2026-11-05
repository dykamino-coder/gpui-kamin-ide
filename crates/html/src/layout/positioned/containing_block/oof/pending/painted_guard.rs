//! Владение списком краски на время приостановки с восстановлением при отмене.
use super::{Output, Painted};
pub(in crate::layout::positioned::containing_block::oof) struct PaintedGuard<'a> {
    slot: &'a Output,
    children: Vec<Painted>,
}
impl<'a> PaintedGuard<'a> {
    pub(in crate::layout::positioned::containing_block::oof) fn take(slot: &'a Output) -> Self {
        Self {
            slot,
            children: slot.take(),
        }
    }
}
impl std::ops::Deref for PaintedGuard<'_> {
    type Target = [Painted];
    fn deref(&self) -> &Self::Target {
        &self.children
    }
}
impl std::ops::DerefMut for PaintedGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.children
    }
}
impl Drop for PaintedGuard<'_> {
    fn drop(&mut self) {
        let mut slot = self.slot.borrow_mut();
        assert!(slot.is_empty(), "same OOF list painted reentrantly");
        *slot = std::mem::take(&mut self.children);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ContentMask, IntoElement};
    #[test]
    fn suspended_paint_owns_children_and_restores_them_on_cancel() {
        let output: Output = Default::default();
        output.borrow_mut().push(Painted(
            gpui::div().into_any_element(),
            ContentMask::default(),
        ));
        {
            let children = PaintedGuard::take(&output);
            assert_eq!(children.len(), 1);
            // No RefMut may survive a child suspension: another scope can read
            // the destination while this continuation owns its child list.
            assert!(output.borrow_mut().is_empty());
            drop(children);
        }
        assert_eq!(output.borrow().len(), 1);
    }
}
