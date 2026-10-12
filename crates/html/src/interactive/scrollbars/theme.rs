//! Static captures freeze the modern theme at rest; live HTML tracks the pointer.
use std::cell::Cell;
thread_local! { static CAPTURE: Cell<bool> = const { Cell::new(false) }; }

/// Freeze scrollbar interaction for static captures on this UI thread.
/// This changes neither appearance nor CSS sizing or authored scrollbar colors.
#[doc(hidden)]
pub fn set_scrollbar_capture_mode(capture: bool) {
    // Reference captures have a fixed at-rest appearance, independent of the
    // desktop pointer.
    CAPTURE.with(|value| value.set(capture));
}
pub(super) fn interactive() -> bool {
    !CAPTURE.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    #[test]
    fn author_colors_keep_their_appearance_and_capture_override_is_local() {
        let mut style = Computed::default();
        assert!(super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "red blue");
        assert!(!super::super::uses_modern(&style));
        style.apply_one("scrollbar-color", "auto");
        set_scrollbar_capture_mode(true);
        assert!(!interactive());
        set_scrollbar_capture_mode(false);
        assert!(interactive());
        assert!(super::super::uses_modern(&style));
        assert_eq!(style.scroll.thickness(), 15.0);
        style.apply_one("scrollbar-width", "thin");
        assert_eq!(style.scroll.thickness(), 10.0);
    }
}
