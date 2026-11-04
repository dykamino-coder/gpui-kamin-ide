//! KaminIDE patch: equal node styles of one layout tree share one allocation.
//!
//! A taffy `Style` is about 1 KiB and every node of a frame tree used to own a copy, although
//! the boxes of a page repeat a few styles (76 800 identical cells keep 76 800 copies). Layout
//! only reads node styles, so a node may point at an equal style of an earlier node. Like a
//! style sharing cache (Stylo), the candidates are the most recently created distinct styles:
//! siblings and cousins are requested next to each other. A hit needs full equality, so the
//! shared style is the same value the node would have owned.

use taffy::{SharedStyle, Style};

/// Distinct recent styles kept as sharing candidates.
const CANDIDATES: usize = 8;

#[derive(Default)]
pub(super) struct StyleShare {
    recent: [Option<SharedStyle>; CANDIDATES],
    /// Slot of the next new candidate; the newest candidate sits just before it.
    next: usize,
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
}
