//! Вычисленный стиль: `font-kerning` отдельным полем, независимым от
//! `font-variant-*` и наследования; `none`/`normal` дают признак `kern` в
//! используемых шрифтовых фичах, `auto` оставляет решение шрифтовой системе.

use super::Computed;

impl Computed {
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

    fn computed(css: &str, vcx: crate::document::state::ValueCx<'_>) -> Computed {
        let mut style = Computed::default();
        style.apply_decls_with_vars(&parse_decls(css), &Default::default(), vcx);
        style
    }

    fn kern(style: &Computed, vcx: crate::document::state::ValueCx<'_>) -> Option<u32> {
        style
            .used_features(vcx)
            .into_iter()
            .find(|(tag, _)| tag == "kern")
            .map(|(_, n)| n)
    }

    #[test]
    fn variant_on_child_preserves_inherited_kerning() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = computed("font-kerning: none", vcx);
        let child = computed("font-variant-ligatures: no-common-ligatures", vcx);
        let inherited = inherit(&parent, &child, vcx);
        assert_eq!(kern(&inherited, vcx), Some(0));
        assert_eq!(kern(&inherited.text_only(), vcx), Some(0));
    }

    #[test]
    fn child_auto_and_initial_cancel_parent_none() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = computed("font-kerning: none", vcx);
        for keyword in ["auto", "initial"] {
            let child = computed(&format!("font-kerning: {keyword}"), vcx);
            assert_eq!(kern(&inherit(&parent, &child, vcx), vcx), None);
        }
        for keyword in ["inherit", "unset"] {
            let child = computed(&format!("font-kerning: {keyword}"), vcx);
            assert_eq!(kern(&inherit(&parent, &child, vcx), vcx), Some(0));
        }
    }

    #[test]
    fn low_level_setting_wins_in_either_declaration_order() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for css in [
            "font-feature-settings: 'kern' 1; font-kerning: none",
            "font-kerning: none; font-feature-settings: 'kern' 1",
        ] {
            assert_eq!(kern(&computed(css, vcx), vcx), Some(1));
        }
    }

    #[test]
    fn invalid_value_preserves_previous_declaration() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = computed("font-kerning: normal", vcx);
        style.apply_one("font-kerning", "invalid", vcx);
        assert_eq!(kern(&style, vcx), Some(1));
        assert!(!style.font_features.iter().any(|(tag, _)| tag == "kern"));
    }

    #[test]
    fn font_shorthand_resets_kerning_and_inherit_restores_parent_value() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let parent = computed("font-kerning: none", vcx);
        let reset = computed("font-kerning: none; font: 16px serif", vcx);
        assert_eq!(kern(&inherit(&parent, &reset, vcx), vcx), None);
        let inherited = computed("font: 16px serif; font: inherit", vcx);
        assert_eq!(kern(&inherit(&parent, &inherited, vcx), vcx), Some(0));
    }

    #[test]
    fn later_kerning_longhand_and_important_declarations_win() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(
            kern(&computed("font: 16px serif; font-kerning: none", vcx), vcx),
            Some(0)
        );
        assert_eq!(
            kern(
                &computed("font-kerning: none !important; font: 16px serif", vcx),
                vcx
            ),
            Some(0)
        );
        assert_eq!(
            kern(
                &computed("font: 16px serif !important; font-kerning: none", vcx),
                vcx
            ),
            None
        );
    }
}
