//! Resolve kerning independently of variant declarations and inheritance.

use super::Computed;

impl Computed {
    pub(super) fn set_font_kerning(&mut self, value: &str) {
        let value = match value.trim() {
            "none" => Some(0),
            "normal" => Some(1),
            "auto" | "initial" => Some(2),
            "inherit" | "unset" => None,
            _ => return,
        };
        self.font_kerning = value;
    }

    pub(super) fn add_kerning_feature(&self, features: &mut Vec<(String, u32)>) {
        if let Some(value @ 0..=1) = self.font_kerning {
            features.push(("kern".into(), value as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::cascade::inherit::inherit;
    use crate::style::css::parse_decls;

    fn computed(css: &str) -> Computed {
        let mut style = Computed::default();
        style.apply_decls_with_vars(&parse_decls(css), &Default::default());
        style
    }

    fn kern(style: &Computed) -> Option<u32> {
        style
            .used_features()
            .into_iter()
            .find(|(tag, _)| tag == "kern")
            .map(|(_, n)| n)
    }

    #[test]
    fn variant_on_child_preserves_inherited_kerning() {
        let parent = computed("font-kerning: none");
        let child = computed("font-variant-ligatures: no-common-ligatures");
        let inherited = inherit(&parent, &child);
        assert_eq!(kern(&inherited), Some(0));
        assert_eq!(kern(&inherited.text_only()), Some(0));
    }

    #[test]
    fn child_auto_and_initial_cancel_parent_none() {
        let parent = computed("font-kerning: none");
        for keyword in ["auto", "initial"] {
            let child = computed(&format!("font-kerning: {keyword}"));
            assert_eq!(kern(&inherit(&parent, &child)), None);
        }
        for keyword in ["inherit", "unset"] {
            let child = computed(&format!("font-kerning: {keyword}"));
            assert_eq!(kern(&inherit(&parent, &child)), Some(0));
        }
    }

    #[test]
    fn low_level_setting_wins_in_either_declaration_order() {
        for css in [
            "font-feature-settings: 'kern' 1; font-kerning: none",
            "font-kerning: none; font-feature-settings: 'kern' 1",
        ] {
            assert_eq!(kern(&computed(css)), Some(1));
        }
    }

    #[test]
    fn invalid_value_preserves_previous_declaration() {
        let mut style = computed("font-kerning: normal");
        style.set_font_kerning("invalid");
        assert_eq!(kern(&style), Some(1));
        assert!(!style.font_features.iter().any(|(tag, _)| tag == "kern"));
    }

    #[test]
    fn font_shorthand_resets_kerning_and_inherit_restores_parent_value() {
        let parent = computed("font-kerning: none");
        let reset = computed("font-kerning: none; font: 16px serif");
        assert_eq!(kern(&inherit(&parent, &reset)), None);
        let inherited = computed("font: 16px serif; font: inherit");
        assert_eq!(kern(&inherit(&parent, &inherited)), Some(0));
    }

    #[test]
    fn later_kerning_longhand_and_important_declarations_win() {
        assert_eq!(
            kern(&computed("font: 16px serif; font-kerning: none")),
            Some(0)
        );
        assert_eq!(
            kern(&computed("font-kerning: none !important; font: 16px serif")),
            Some(0)
        );
        assert_eq!(
            kern(&computed("font: 16px serif !important; font-kerning: none")),
            None
        );
    }
}
