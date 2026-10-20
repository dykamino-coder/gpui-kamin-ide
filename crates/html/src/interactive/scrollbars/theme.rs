//! Author scrollbar colors and independent host capture inputs.

#[cfg(test)]
mod tests {
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
        assert!(super::super::uses_modern(&style));
        assert_eq!(style.scroll.thickness(), 15.0);
        style.apply_one("scrollbar-width", "thin", vcx);
        assert_eq!(style.scroll.thickness(), 10.0);
    }
}

#[cfg(test)]
mod host_capture {
    #[test]
    fn deferred_context_keeps_its_host_capture_mode() {
        let mut host = crate::layout::cx::LayoutCx::default();
        assert!(!host.freeze_scrollbars);
        host.freeze_scrollbars = true;
        let deferred = host.clone();
        host.freeze_scrollbars = false;
        assert!(deferred.freeze_scrollbars);
        assert!(!host.freeze_scrollbars);
    }
}
