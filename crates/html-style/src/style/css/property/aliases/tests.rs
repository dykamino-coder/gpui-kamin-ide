//! Compat Standard aliases share a cascade slot and retain legacy value parsing.

use super::*;

#[test]
fn board_appearance_compat_aliases_share_names_values_and_source_order() {
    use crate::style::css::parse_decls;
    // Compatibility Standard §3.4.1–2, plus appearance (CSS UI) and columns.
    let suffixes = [
        "align-items",
        "align-content",
        "align-self",
        "animation-name",
        "animation-duration",
        "animation-timing-function",
        "animation-iteration-count",
        "animation-direction",
        "animation-play-state",
        "animation-delay",
        "animation-fill-mode",
        "animation",
        "backface-visibility",
        "background-clip",
        "background-origin",
        "background-size",
        "border-bottom-left-radius",
        "border-bottom-right-radius",
        "border-top-left-radius",
        "border-top-right-radius",
        "border-radius",
        "box-shadow",
        "box-sizing",
        "flex",
        "flex-basis",
        "flex-direction",
        "flex-flow",
        "flex-grow",
        "flex-shrink",
        "flex-wrap",
        "filter",
        "justify-content",
        "mask",
        "mask-clip",
        "mask-composite",
        "mask-image",
        "mask-origin",
        "mask-position",
        "mask-repeat",
        "mask-size",
        "order",
        "perspective",
        "perspective-origin",
        "transform-origin",
        "transform-style",
        "transform",
        "transition-delay",
        "transition-duration",
        "transition-property",
        "transition-timing-function",
        "transition",
        "text-size-adjust",
        "appearance",
        "column-count",
        "column-gap",
    ];
    let aliases = suffixes
        .iter()
        .map(|suffix| (format!("-webkit-{suffix}"), *suffix))
        .chain([
            ("-webkit-mask-box-image".to_owned(), "mask-border"),
            (
                "-webkit-mask-box-image-outset".to_owned(),
                "mask-border-outset",
            ),
            (
                "-webkit-mask-box-image-repeat".to_owned(),
                "mask-border-repeat",
            ),
            (
                "-webkit-mask-box-image-slice".to_owned(),
                "mask-border-slice",
            ),
            (
                "-webkit-mask-box-image-source".to_owned(),
                "mask-border-source",
            ),
            (
                "-webkit-mask-box-image-width".to_owned(),
                "mask-border-width",
            ),
        ]);
    for (alias, canonical) in aliases {
        assert_eq!(legacy_name_alias(&alias), Some(canonical), "{alias}");
        for (first, second) in [("inherit", "initial"), ("initial", "inherit")] {
            for prefixed_first in [true, false] {
                let (a, b) = if prefixed_first {
                    (alias.as_str(), canonical)
                } else {
                    (canonical, alias.as_str())
                };
                let actual = parse_decls(&format!("{a}:{first};{b}:{second}!important"));
                let expected = parse_decls(&format!(
                    "{canonical}:{first};{canonical}:{second}!important"
                ));
                let entries = |block: &crate::style::css::DeclBlock| {
                    block
                        .iter()
                        .map(|d| (d.name.clone(), d.value.clone(), d.important, d.pos))
                        .collect::<Vec<_>>()
                };
                assert!(!actual.is_empty(), "{alias}");
                assert_eq!(entries(&actual), entries(&expected), "{alias}");
            }
        }
    }
}
#[test]
fn prefixed_transform_family_shares_the_standard_slot() {
    assert_eq!(
        legacy_name_alias("-webkit-perspective"),
        Some("perspective")
    );
    assert_eq!(legacy_name_alias("-webkit-transform"), Some("transform"));
    assert_eq!(legacy_name_alias("-webkit-box-shadow"), Some("box-shadow"));
    assert_eq!(legacy_name_alias("-webkit-mask-image"), Some("mask-image"));
    assert_eq!(
        legacy_name_alias("-webkit-mask-composite"),
        Some("mask-composite")
    );
    assert_eq!(legacy_name_alias("perspective"), None);
}

#[test]
fn legacy_values_follow_the_old_grammar() {
    assert_eq!(
        legacy_alias_value("-webkit-border-radius", "4px 8px"),
        "4px / 8px"
    );
    assert_eq!(
        legacy_alias_value("-webkit-border-radius", "1px 2px 3px"),
        "1px 2px 3px"
    );
    assert_eq!(
        legacy_alias_value("-webkit-border-radius", "1px / 2px"),
        "1px / 2px"
    );
    assert_eq!(
        legacy_alias_value("-webkit-background-size", "2em, cover"),
        "2em 2em, cover"
    );
    assert_eq!(legacy_alias_value("border-radius", "4px 8px"), "4px 8px");
}
