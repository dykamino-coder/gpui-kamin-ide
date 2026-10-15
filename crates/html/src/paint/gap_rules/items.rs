//! Fragment geometry collected before painting gap decorations.
use gpui::{Bounds, Pixels};

mod probe;
pub use probe::gap_item_probe;

/// Each rendered container instance owns its geometry, including copies of
/// the same DOM node in separate fragmentainers (CSS Gaps 1 §fragmentation).
pub fn fresh_gap_items(mut key: u64) -> (u64, super::GapItems) {
    super::GAP_ITEMS.with(|m| {
        let mut m = m.borrow_mut();
        while m.contains_key(&key) {
            key = key.wrapping_add(0x9E37_79B9_7F4A_7C15);
        }
        let items = super::GapItems::default();
        m.insert(key, items.clone());
        (key, items)
    })
}
#[derive(Default)]
pub struct GapBuffer {
    pub items: Vec<Bounds<Pixels>>,
    pub margins: Vec<[f32; 2]>,
    pub multicol: Option<MulticolGaps>,
}
impl std::ops::Deref for GapBuffer {
    type Target = Vec<Bounds<Pixels>>;
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
impl std::ops::DerefMut for GapBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}
#[derive(Default)]
pub struct MulticolGaps {
    pub columns: Vec<Bounds<Pixels>>,
    pub rows: Vec<(Bounds<Pixels>, usize)>,
    pub spanners: Vec<Bounds<Pixels>>,
}

#[cfg(test)]
mod tests {
    use super::fresh_gap_items;

    #[test]
    fn copies_of_one_node_collect_independent_geometry() {
        let (first, a) = fresh_gap_items(0);
        let (second, b) = fresh_gap_items(0);
        assert_ne!(first, second);
        assert!(!std::rc::Rc::ptr_eq(&a, &b));
        assert!(std::rc::Rc::ptr_eq(
            &a,
            &crate::paint::gap_rules::gap_items_for(first)
        ));
        assert!(std::rc::Rc::ptr_eq(
            &b,
            &crate::paint::gap_rules::gap_items_for(second)
        ));
    }
}
