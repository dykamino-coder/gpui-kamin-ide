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

use crate::{Refineable, Style, StyleRefinement};
use std::cell::RefCell;
use std::rc::Rc;

/// Distinct recent refinements kept as sharing candidates. Boxes that differ only in paint
/// (selectors-3200: 24 cells per row, each with its own border colour) repeat with a period
/// longer than a handful of candidates; with eight, every cell missed and was refined, converted
/// and laid out from its own copy. Stylo's style sharing cache keeps 32 candidates for the same
/// reason. A miss scans every candidate, but equality stops at the first differing field.
const CANDIDATES: usize = 32;

thread_local! {
    static RECENT: RefCell<RefinementShare> = RefCell::default();
    static EMPTY: Rc<StyleRefinement> = Rc::default();
}

/// The empty refinement every new element of this thread starts from. Shared, so a write
/// copies it; an element whose style is replaced whole (a reused conversion) never allocates.
pub(crate) fn empty() -> Rc<StyleRefinement> {
    EMPTY.with(Rc::clone)
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
                Some(candidate) if likely_equal(candidate, style) && **candidate == **style => {
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

/// Fields that most often tell two box styles apart, compared before the whole refinement:
/// with 32 candidates most comparisons are misses, and the derived equality walks every equal
/// leading field (display, overflow, insets, sizing keywords, ...) first. The grid placement
/// comes first: it is the only difference between the cells of a table (table-360: 6 000
/// distinct refinements per frame, all equal up to `grid_location`). Only a shortcut for
/// unequal values: a hit is still decided by full equality.
fn likely_equal(a: &StyleRefinement, b: &StyleRefinement) -> bool {
    a.grid_location == b.grid_location
        && a.background == b.background
        && a.border_color == b.border_color
        && a.size == b.size
        && a.padding == b.padding
        && a.border_widths == b.border_widths
        && a.margin == b.margin
}

thread_local! {
    static COMPUTED: RefCell<ComputedShare> = RefCell::default();
}

/// The computed style of a div whose style is `refinement` applied to the default style alone
/// (no state styles): computed once per recent refinement allocation and shared by every
/// request, prepaint and paint of every div holding that allocation. As in Stylo and Blink,
/// boxes with equal styles read one computed style object instead of each building a copy.
pub(crate) fn computed(refinement: &Rc<StyleRefinement>) -> Rc<Style> {
    COMPUTED.with(|share| share.borrow_mut().computed(refinement))
}

#[derive(Default)]
struct ComputedShare {
    /// The refinement is held so its address cannot be reused by another refinement; writes to
    /// a shared refinement copy it (`Rc::make_mut`), so a held one never changes.
    recent: [Option<(Rc<StyleRefinement>, Rc<Style>)>; CANDIDATES],
    next: usize,
}

impl ComputedShare {
    fn computed(&mut self, refinement: &Rc<StyleRefinement>) -> Rc<Style> {
        for back in 1..=CANDIDATES {
            let slot = (self.next + CANDIDATES - back) % CANDIDATES;
            match &self.recent[slot] {
                Some((key, style)) if Rc::ptr_eq(key, refinement) => return style.clone(),
                Some(_) => {}
                None => break,
            }
        }
        let mut style = Style::default();
        style.refine(refinement);
        let style = Rc::new(style);
        self.recent[self.next] = Some((refinement.clone(), style.clone()));
        self.next = (self.next + 1) % CANDIDATES;
        style
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
    fn the_shortcut_never_rejects_equal_refinements() {
        let a = sized(8.0);
        assert!(likely_equal(&a, &sized(8.0)));
        assert!(!likely_equal(&a, &sized(9.0)));
        // Cells that differ only in grid placement stay distinct.
        let mut share = RefinementShare::default();
        let cell = |row: i16| {
            let mut style = StyleRefinement::default();
            style.grid_location = Some(crate::GridLocation {
                row: crate::GridPlacement::Line(row)..crate::GridPlacement::Auto,
                ..Default::default()
            });
            Rc::new(style)
        };
        let mut first = cell(1);
        share.share(&mut first);
        let mut second = cell(2);
        share.share(&mut second);
        assert!(!Rc::ptr_eq(&first, &second));
        let mut again = cell(1);
        share.share(&mut again);
        assert!(Rc::ptr_eq(&first, &again));
    }

    #[test]
    fn a_row_of_paint_variants_stays_shared_and_computed_once() {
        // selectors-3200: rows of 24 cells, each cell its own refinement; the next row
        // repeats them. Every cell of the second row shares the first row's allocation
        // and its computed style.
        let mut share = RefinementShare::default();
        let mut computed = ComputedShare::default();
        let row = |share: &mut RefinementShare| -> Vec<Rc<StyleRefinement>> {
            (0..24)
                .map(|cell| {
                    let mut style = sized(cell as f32);
                    share.share(&mut style);
                    style
                })
                .collect()
        };
        let first = row(&mut share);
        let first_computed: Vec<_> = first.iter().map(|s| computed.computed(s)).collect();
        let second = row(&mut share);
        for (cell, style) in second.iter().enumerate() {
            assert!(Rc::ptr_eq(style, &first[cell]));
            assert!(Rc::ptr_eq(&computed.computed(style), &first_computed[cell]));
        }
    }

    #[test]
    fn a_refinement_is_computed_once_and_equal_to_refining_the_default() {
        let mut share = ComputedShare::default();
        let refinement = sized(8.0);
        let first = share.computed(&refinement);
        let again = share.computed(&refinement);
        assert!(Rc::ptr_eq(&first, &again));
        let mut expected = Style::default();
        expected.refine(&refinement);
        // Style has no PartialEq; its debug form lists every field.
        assert_eq!(format!("{:?}", *first), format!("{expected:?}"));
        // An equal refinement in another allocation is computed on its own.
        let other = share.computed(&sized(8.0));
        assert!(!Rc::ptr_eq(&first, &other));
        assert_eq!(format!("{:?}", *other), format!("{expected:?}"));
    }
    #[test]
    fn new_divs_share_the_empty_style_until_written() {
        use crate::{InteractiveElement, Styled, div};
        let mut a = div();
        let mut b = div();
        let empty = b.interactivity().base_style.clone();
        assert!(Rc::ptr_eq(&a.interactivity().base_style, &empty));
        assert_eq!(*empty, StyleRefinement::default());
        a.style().size.width = Some(px(5.).into());
        assert!(!Rc::ptr_eq(&a.interactivity().base_style, &empty));
        assert_eq!(*b.interactivity().base_style, StyleRefinement::default());
        assert_eq!(*super::empty(), StyleRefinement::default());
        assert_eq!(a.interactivity().base_style.size.width, Some(px(5.).into()));
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
