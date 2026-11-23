//! Sorted persistent counter-name index; updating one name shares all other stacks.
use super::{Entry, Path, Sequence, remove_stale};
use std::rc::Rc;
#[derive(Default, Clone, PartialEq)]
pub(super) struct Names(Sequence<(Rc<str>, Sequence<Entry>)>);
impl Names {
    fn find(&self, name: &str) -> Result<usize, usize> {
        let mut low = 0;
        let mut high = self.0.len();
        while low < high {
            let mid = low + (high - low) / 2;
            match self.0.get(mid).expect("name index").0.as_ref().cmp(name) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }
        Err(low)
    }
    pub fn put(&mut self, name: &str, stack: Sequence<Entry>) {
        match self.find(name) {
            Ok(i) => {
                let key = self.0.get(i).expect("name").0.clone();
                self.0.set(i, (key, stack));
            }
            Err(i) => self.0.insert(i, (Rc::from(name), stack)),
        }
    }
    pub fn remove(&mut self, name: &str) {
        if let Ok(i) = self.find(name) {
            self.0.remove(i);
        }
    }
    pub fn pruned(&mut self, name: &str, path: &Path) -> Option<Sequence<Entry>> {
        let i = self.find(name).ok()?;
        let mut stack = self.0.get(i)?.1.clone();
        let before = stack.len();
        remove_stale(&mut stack, path);
        if stack.len() != before {
            if stack.is_empty() {
                self.0.remove(i);
            } else {
                let key = self.0.get(i)?.0.clone();
                self.0.set(i, (key, stack.clone()));
            }
        }
        Some(stack)
    }
}
