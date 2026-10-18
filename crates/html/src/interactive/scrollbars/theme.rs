//! Static captures freeze the modern theme at rest; live HTML tracks the pointer.
#[cfg(any(test, feature = "wpt-harness"))]
use std::cell::Cell;
#[cfg(any(test, feature = "wpt-harness"))]
thread_local! { static CAPTURE: Cell<bool> = const { Cell::new(false) }; }

/// Freeze scrollbar interaction for static captures on this UI thread.
/// This changes neither appearance nor CSS sizing or authored scrollbar colors.
#[cfg(any(test, feature = "wpt-harness"))]
pub fn set_scrollbar_capture_mode(capture: bool) {
    // Reference captures have a fixed at-rest appearance, independent of the
    // desktop pointer.
    CAPTURE.with(|value| value.set(capture));
}
pub(super) fn interactive() -> bool {
    #[cfg(any(test, feature = "wpt-harness"))]
    {
        !CAPTURE.with(Cell::get)
    }
    #[cfg(not(any(test, feature = "wpt-harness")))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn author_colors_keep_their_appearance_and_capture_override_is_local() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        assert!(super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "red blue", vcx);
        assert!(!super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "auto", vcx);
        set_scrollbar_capture_mode(true);
        assert!(!interactive());
        set_scrollbar_capture_mode(false);
        assert!(interactive());
        assert!(super::super::uses_modern(&style));
        assert_eq!(style.scroll.thickness(), 15.0);
        style.apply_one("scrollbar-width", "thin", vcx);
        assert_eq!(style.scroll.thickness(), 10.0);
    }
}
