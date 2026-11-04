//! KaminIDE patch: equal div base style refinements share one allocation.
//!
//! Every `Div` owned the refinement it was styled with (about 480 bytes), although the boxes
//! of a page repeat a few styles (selectors-3200: 217 604 divs). A finished div (`into_element`)
//! keeps its refinement for reading (`compute_style` in request layout, prepaint and paint);
//! a later write goes through `Rc::make_mut` and copies, so sharing never changes a value.
//! As in the taffy node style share (`taffy::style_share`), the candidates are the most
//! recently finished distinct refinements and a hit needs full equality. Sharing when a div is
//! finished, not at layout, frees the duplicate while the next sibling is built, so its
//! allocation reuses the freed block instead of growing the frame's working set. The
//! candidates are per thread and bounded: at most `CANDIDATES` refinements outlive their frame.

use crate::StyleRefinement;
use std::cell::RefCell;
use std::rc::Rc;

/// Distinct recent refinements kept as sharing candidates.
const CANDIDATES: usize = 8;

thread_local! {
    static RECENT: RefCell<RefinementShare> = RefCell::default();
}

/// Point `style` at an equal recent refinement of this thread, or make it a candidate.
pub(crate) fn share(style: &mut Rc<StyleRefinement>) {
    RECENT.with(|recent| recent.borrow_mut().share(style));
}

#[derive(Default)]
struct RefinementShare {
    recent: [Option<Rc<StyleRefinement>>; CANDIDATES],
    /// Slot of the next new candidate; the newest candidate sits just before it.
    next: usize,
}

impl RefinementShare {
    /// Point `style` at an equal earlier refinement, or make it a candidate for later ones.
    /// The value seen through `style` never changes.
    fn share(&mut self, style: &mut Rc<StyleRefinement>) {
        for back in 1..=CANDIDATES {
            let slot = (self.next + CANDIDATES - back) % CANDIDATES;
            match &self.recent[slot] {
                Some(candidate) if Rc::ptr_eq(candidate, style) => return,
                Some(candidate) if **candidate == **style => {
                    *style = candidate.clone();
                    return;
                }
                Some(_) => {}
                None => break,
            }
        }
        self.recent[self.next] = Some(style.clone());
        self.next = (self.next + 1) % CANDIDATES;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Display, px};

    fn sized(width: f32) -> Rc<StyleRefinement> {
        let mut style = StyleRefinement::default();
        style.size.width = Some(px(width).into());
        Rc::new(style)
    }

    #[test]
    fn equal_refinements_share_one_allocation() {
        let mut share = RefinementShare::default();
        let mut first = sized(8.0);
        share.share(&mut first);
        let mut other = Rc::new(StyleRefinement {
            display: Some(Display::Grid),
            ..StyleRefinement::default()
        });
        share.share(&mut other);
        let mut again = sized(8.0);
        share.share(&mut again);
        assert!(Rc::ptr_eq(&first, &again));
        assert!(!Rc::ptr_eq(&first, &other));
        assert_eq!(*again, *sized(8.0));
        // Sharing the same allocation again is a no-op.
        let before = again.clone();
        share.share(&mut again);
        assert!(Rc::ptr_eq(&before, &again));
    }

    #[test]
    fn different_refinements_keep_their_values_and_writes_copy() {
        let mut share = RefinementShare::default();
        let mut narrow = sized(8.0);
        share.share(&mut narrow);
        let mut wide = sized(9.0);
        share.share(&mut wide);
        assert_eq!(*narrow, *sized(8.0));
        assert_eq!(*wide, *sized(9.0));
        let mut same = sized(8.0);
        share.share(&mut same);
        Rc::make_mut(&mut same).size.width = Some(px(7.0).into());
        assert_eq!(*narrow, *sized(8.0));
        assert_eq!(*same, *sized(7.0));
    }

    #[test]
    fn old_candidates_are_replaced() {
        let mut share = RefinementShare::default();
        let mut first = sized(0.0);
        share.share(&mut first);
        for width in 1..=CANDIDATES {
            share.share(&mut sized(width as f32));
        }
        let mut after = sized(0.0);
        share.share(&mut after);
        assert!(!Rc::ptr_eq(&first, &after));
        assert_eq!(*after, *first);
    }

    #[test]
    fn finished_divs_with_equal_styles_share_and_styling_after_copies() {
        use crate::{InteractiveElement, IntoElement, Styled, div};
        let mut a = div().w(px(12.)).into_element();
        let mut b = div().w(px(12.)).into_element();
        let b_style = b.interactivity().base_style.clone();
        assert!(Rc::ptr_eq(&a.interactivity().base_style, &b_style));
        a.style().size.width = Some(px(13.).into());
        assert!(!Rc::ptr_eq(&a.interactivity().base_style, &b_style));
        assert_eq!(b_style.size.width, Some(px(12.).into()));
        assert_eq!(a.interactivity().base_style.size.width, Some(px(13.).into()));
    }
}
