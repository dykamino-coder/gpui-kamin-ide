//! Equal decoration paints shared by the boxes of one document (`share`).
//!
//! Boxes of one look (the cells of a table row, the items of a list) compute
//! equal paints one after another; each kept its own copy. The document keeps
//! the latest distinct paints and hands out an equal one instead, as the style
//! sharing caches of Gecko (`StyleSharingCache`) and Blink
//! (`StyleSharingList`) offer a recent sibling's style: a short list of
//! candidates, compared by value, with the found one moved to the front.

use super::DecorationPaint;
use std::cell::Cell;
use std::rc::Rc;

/// Candidates kept, as many as Gecko's style sharing cache holds about.
const KEPT: usize = 16;

#[derive(Default)]
pub struct SharedPaints {
    latest: Cell<Vec<Rc<DecorationPaint>>>,
}

impl SharedPaints {
    /// A paint equal to `paint`: a kept one when there is one, else `paint`,
    /// kept from now on.
    pub fn share(&self, paint: DecorationPaint) -> Rc<DecorationPaint> {
        let mut latest = self.latest.take();
        let shared = match latest.iter().position(|p| **p == paint) {
            Some(i) => latest.remove(i),
            None => {
                latest.truncate(KEPT - 1);
                Rc::new(paint)
            }
        };
        latest.insert(0, shared.clone());
        self.latest.set(latest);
        shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn equal_paints_are_one_and_the_found_one_comes_first() {
        let shared = SharedPaints::default();
        let a = shared.share(DecorationPaint::from_computed(&Computed::default()));
        let mut other = DecorationPaint::from_computed(&Computed::default());
        other.decorations = false;
        let b = shared.share(other.clone());
        let again = shared.share(DecorationPaint::from_computed(&Computed::default()));
        assert!(Rc::ptr_eq(&a, &again));
        assert!(!Rc::ptr_eq(&a, &b));
        let latest = shared.latest.take();
        assert!(Rc::ptr_eq(&latest[0], &a));
        shared.latest.set(latest);
        assert!(Rc::ptr_eq(&shared.share(other), &b));
    }

    #[test]
    fn the_list_keeps_only_the_latest_paints() {
        let shared = SharedPaints::default();
        for n in 0..KEPT + 4 {
            let mut paint = DecorationPaint::from_computed(&Computed::default());
            paint.filter_ops = vec![crate::style::computed::Filter::neutral(); n];
            shared.share(paint);
        }
        let latest = shared.latest.take();
        assert_eq!(latest.len(), KEPT);
        shared.latest.set(latest);
    }
}
