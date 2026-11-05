//! KaminIDE patch: equal node styles of one layout tree share one allocation.
//!
//! A taffy `Style` is about 1 KiB and every node of a frame tree used to own a copy, although
//! the boxes of a page repeat a few styles (76 800 identical cells keep 76 800 copies). Layout
//! only reads node styles, so a node may point at an equal style of an earlier node. Like a
//! style sharing cache (Stylo), the candidates are the most recently created distinct styles:
//! siblings and cousins are requested next to each other. A hit needs full equality, so the
//! shared style is the same value the node would have owned.
//!
//! The conversion itself is shared too. A `Div` whose layout style is its base refinement alone
//! (no state styles applied) converts the same refinement allocation (`refinement_share`) to the
//! same taffy style for a given rem size and scale factor, so the converted style is kept per
//! recent refinement and a request with the same refinement skips `to_taffy` (Blink and Stylo
//! keep the computed style object itself shared between elements in the same way). `calc()`
//! handles in a converted style belong to this tree's registry, which lives as long as the
//! candidates (`clear`).

use crate::{Pixels, StyleRefinement};
use std::rc::Rc;
use taffy::{SharedStyle, Style};

/// Distinct recent styles kept as sharing candidates; as many as the refinement candidates
/// (`refinement_share`), so a converted refinement stays until its refinement is replaced.
pub(super) const CANDIDATES: usize = 32;

#[derive(Default)]
pub(super) struct StyleShare {
    recent: [Option<SharedStyle>; CANDIDATES],
    /// Slot of the next new candidate; the newest candidate sits just before it.
    next: usize,
    converted: [Option<Converted>; CANDIDATES],
    /// Slot of the next converted refinement.
    next_converted: usize,
}

/// The taffy style a refinement converted to at one rem size and scale factor.
struct Converted {
    /// Held so the allocation, and with it the key, cannot be reused by another refinement.
    refinement: Rc<StyleRefinement>,
    rem_size: Pixels,
    scale_factor: f32,
    style: SharedStyle,
}

impl StyleShare {
    /// The shared allocation of a style equal to `style`, or a new one that later equal
    /// styles share.
    pub(super) fn share(&mut self, style: Style) -> SharedStyle {
        for back in 1..=CANDIDATES {
            let slot = (self.next + CANDIDATES - back) % CANDIDATES;
            match &self.recent[slot] {
                Some(candidate) if **candidate == style => return candidate.clone(),
                Some(_) => {}
                None => break,
            }
        }
        let shared = SharedStyle::new(style);
        self.recent[self.next] = Some(shared.clone());
        self.next = (self.next + 1) % CANDIDATES;
        shared
    }

    /// The shared taffy style of a node whose style is `refinement` applied to the default
    /// style alone: the conversion `convert` runs only for a refinement allocation not
    /// converted recently at this rem size and scale factor.
    pub(super) fn share_refinement(
        &mut self,
        refinement: &Rc<StyleRefinement>,
        rem_size: Pixels,
        scale_factor: f32,
        convert: impl FnOnce() -> Style,
    ) -> SharedStyle {
        let hit = self.converted.iter().flatten().find(|c| {
            Rc::ptr_eq(&c.refinement, refinement)
                && c.rem_size == rem_size
                && c.scale_factor.to_bits() == scale_factor.to_bits()
        });
        if let Some(hit) = hit {
            return hit.style.clone();
        }
        let style = self.share(convert());
        self.converted[self.next_converted] = Some(Converted {
            refinement: refinement.clone(),
            rem_size,
            scale_factor,
            style: style.clone(),
        });
        self.next_converted = (self.next_converted + 1) % CANDIDATES;
        style
    }

    /// Forget the candidates together with the tree they were made for.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::{Dimension, Display, Size};

    fn sized(width: f32) -> Style {
        Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::auto(),
            },
            ..Style::default()
        }
    }

    #[test]
    fn equal_styles_share_one_allocation() {
        let mut share = StyleShare::default();
        let first = share.share(sized(8.0));
        let other = share.share(Style {
            display: Display::Grid,
            ..Style::default()
        });
        let again = share.share(sized(8.0));
        assert!(SharedStyle::ptr_eq(&first, &again));
        assert!(!SharedStyle::ptr_eq(&first, &other));
        assert_eq!(*again, sized(8.0));
    }

    #[test]
    fn different_styles_keep_their_own_values() {
        let mut share = StyleShare::default();
        let narrow = share.share(sized(8.0));
        let wide = share.share(sized(9.0));
        assert_eq!(*narrow, sized(8.0));
        assert_eq!(*wide, sized(9.0));
    }

    #[test]
    fn old_candidates_are_replaced_and_clear_forgets_all() {
        let mut share = StyleShare::default();
        let first = share.share(sized(0.0));
        for width in 1..=CANDIDATES {
            share.share(sized(width as f32));
        }
        // The first style left the candidates: an equal one gets a new allocation.
        let after = share.share(sized(0.0));
        assert!(!SharedStyle::ptr_eq(&first, &after));
        assert_eq!(*after, *first);
        let kept = share.share(sized(0.0));
        assert!(SharedStyle::ptr_eq(&after, &kept));
        share.clear();
        let cleared = share.share(sized(0.0));
        assert!(!SharedStyle::ptr_eq(&kept, &cleared));
    }

    #[test]
    fn a_refinement_is_converted_once_per_rem_size_and_scale() {
        let mut share = StyleShare::default();
        let refinement = Rc::new(StyleRefinement::default());
        let mut conversions = 0;
        let mut convert = |width| {
            conversions += 1;
            sized(width)
        };
        let first = share.share_refinement(&refinement, Pixels(16.0), 1.0, || convert(1.0));
        let again = share.share_refinement(&refinement, Pixels(16.0), 1.0, || convert(2.0));
        assert!(SharedStyle::ptr_eq(&first, &again));
        let scaled = share.share_refinement(&refinement, Pixels(16.0), 2.0, || convert(3.0));
        let rem = share.share_refinement(&refinement, Pixels(20.0), 1.0, || convert(4.0));
        assert_eq!(*scaled, sized(3.0));
        assert_eq!(*rem, sized(4.0));
        assert_eq!(conversions, 3);
    }

    #[test]
    fn a_row_of_24_refinements_is_converted_once() {
        // Rows of 24 paint variants (selectors-3200) repeat: the second row converts nothing.
        let mut share = StyleShare::default();
        let row: Vec<_> = (0..24).map(|_| Rc::new(StyleRefinement::default())).collect();
        let mut conversions = 0;
        for _ in 0..2 {
            for refinement in &row {
                share.share_refinement(refinement, Pixels(16.0), 1.0, || {
                    conversions += 1;
                    sized(1.0)
                });
            }
        }
        assert_eq!(conversions, 24);
    }

    #[test]
    fn an_equal_refinement_in_another_allocation_is_converted_again() {
        let mut share = StyleShare::default();
        let a = Rc::new(StyleRefinement::default());
        let b = Rc::new(StyleRefinement::default());
        share.share_refinement(&a, Pixels(16.0), 1.0, || sized(1.0));
        let other = share.share_refinement(&b, Pixels(16.0), 1.0, || sized(2.0));
        assert_eq!(*other, sized(2.0));
        share.clear();
        let cleared = share.share_refinement(&a, Pixels(16.0), 1.0, || sized(5.0));
        assert_eq!(*cleared, sized(5.0));
    }
}
