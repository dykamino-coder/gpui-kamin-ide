//! Software override: ID specificity beats class-based infinite !important rules.

pub(super) fn block(software: bool) -> &'static str {
    if software {
        r#"<style id="__kaminReducedMotion">*:not(#__kaminNoMotion),*:not(#__kaminNoMotion)::before,*:not(#__kaminNoMotion)::after{animation-iteration-count:1!important}</style>"#
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn override_has_id_specificity_for_element_and_both_pseudo_elements() {
        let block = super::block(true);
        for selector in [
            "*:not(#__kaminNoMotion)",
            "*:not(#__kaminNoMotion)::before",
            "*:not(#__kaminNoMotion)::after",
        ] {
            assert!(block.contains(selector));
        }
        assert!(block.contains("animation-iteration-count:1!important"));
        assert!(!block.contains("animation:none"));
        assert_eq!(super::block(false), "");
    }
}
