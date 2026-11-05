//! Освобождение вложенных мест плана не использует стек глубины документа.
use super::Slot;
#[derive(Debug, Default)]
pub(super) struct Children(Vec<Slot>);
impl From<Vec<Slot>> for Children {
    fn from(slots: Vec<Slot>) -> Self {
        Self(slots)
    }
}
impl std::ops::Deref for Children {
    type Target = [Slot];
    fn deref(&self) -> &[Slot] {
        &self.0
    }
}
impl Drop for Children {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.0);
        while let Some(mut slot) = pending.pop() {
            pending.append(&mut slot.kids.0);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twenty_thousand_plan_levels_drop_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut root = Slot::default();
                for _ in 0..20_000 {
                    root = Slot {
                        kids: vec![root].into(),
                        ..Default::default()
                    };
                }
                drop(root);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
