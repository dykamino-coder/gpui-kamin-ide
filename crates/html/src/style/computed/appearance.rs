//! `appearance` / `-webkit-appearance` (css-ui-4 §7.2, css-forms-1 §appearance).
//!
//! Only the effective widget look matters to rendering: `none` turns a form
//! control into a primitive box painted purely from CSS, `base` selects the
//! interoperable base look, everything else (`auto` and the compat keywords)
//! keeps the native look. Blink folds the compat keywords the same way
//! (`StyleAdjuster::AdjustStyleForHTMLElement`, `LayoutTheme::AdjustAppearance`).

/// Computed `appearance`. The initial value `auto` is the field's `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// `auto` and the `<compat-auto>`/`<compat-special>` keywords.
    Auto,
    /// `none`: no native widget painting, plain CSS box.
    None,
    /// `base` (css-forms-1): the base appearance stylesheet look.
    Base,
}

impl Appearance {
    /// Parse one value; `None` for an invalid declaration (kept unapplied,
    /// CSS 2.1 §4.1.8).
    pub fn parse(v: &str) -> Option<Self> {
        Some(match v.trim().to_ascii_lowercase().as_str() {
            "none" => Appearance::None,
            "base" => Appearance::Base,
            "auto" | "searchfield" | "textarea" | "checkbox" | "radio" | "menulist" | "listbox"
            | "meter" | "progress-bar" | "button" | "textfield" | "menulist-button" => {
                Appearance::Auto
            }
            _ => return None,
        })
    }
}

impl crate::style::computed::Computed {
    /// `appearance: none` — the control is rendered as a primitive CSS box.
    pub fn appearance_none(&self) -> bool {
        self.appearance == Some(Appearance::None)
    }
}

#[cfg(test)]
mod tests {
    use super::Appearance;
    use crate::style::computed::Computed;

    #[test]
    fn keywords_fold_to_auto_none_or_base() {
        assert_eq!(Appearance::parse("none"), Some(Appearance::None));
        assert_eq!(Appearance::parse(" NONE "), Some(Appearance::None));
        assert_eq!(Appearance::parse("base"), Some(Appearance::Base));
        for k in [
            "auto",
            "checkbox",
            "menulist-button",
            "textfield",
            "progress-bar",
        ] {
            assert_eq!(Appearance::parse(k), Some(Appearance::Auto), "{k}");
        }
        assert_eq!(Appearance::parse("bogus"), None);
        assert_eq!(Appearance::parse("none auto"), None);
    }

    #[test]
    fn prefixed_alias_and_invalid_values() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("-webkit-appearance", "none", vcx);
        assert!(c.appearance_none());
        c.apply_one("appearance", "junk", vcx);
        assert!(c.appearance_none(), "invalid value keeps the previous one");
        c.apply_one("appearance", "auto", vcx);
        assert_eq!(c.appearance, Some(Appearance::Auto));
        assert!(!c.appearance_none());
    }
}
