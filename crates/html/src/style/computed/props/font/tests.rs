//! Длинные свойства шрифта реестра: разбор, сброс сокращением `font`,
//! наследование групп `font-variant-*`.

use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::css::parse_decls;
use crate::style::values::value::Len;

fn doc() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

fn style(css: &str, vcx: crate::document::state::ValueCx<'_>) -> Computed {
    let mut c = Computed::default();
    c.apply_decls(&crate::style::css::parse_decls_in(css, vcx), vcx);
    c
}

#[test]
fn stretch_preserves_fractional_percentages_and_keywords() {
    let doc = doc();
    let vcx = doc.values();
    for (value, expected) in [
        ("extra-condensed", 62.5),
        ("semi-condensed", 87.5),
        ("semi-expanded", 112.5),
        ("87.5%", 87.5),
        ("0%", 0.),
        ("calc(50% + 25%)", 75.),
    ] {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(&format!("font-stretch: {value}")), vcx);
        assert_eq!(c.font_stretch, Some(expected), "{value}");
    }
}

#[test]
fn invalid_stretch_preserves_previous_declaration() {
    let doc = doc();
    let vcx = doc.values();
    for value in [
        "123",
        "0",
        "-5%",
        "100%%",
        "NaN%",
        "inf%",
        "10px",
        "stretch",
        "87.5% extra",
    ] {
        let c = style(&format!("font-stretch: 75%; font-stretch: {value}"), vcx);
        assert_eq!(c.font_stretch, Some(75.), "{value}");
    }
}

#[test]
fn font_shorthand_resets_every_longhand_it_names() {
    let doc = doc();
    let vcx = doc.values();
    let c = style(
        "font-style: italic; font-weight: bold; line-height: 3; font-kerning: none; \
         font-variant-numeric: tabular-nums; font-size-adjust: 0.5; \
         font-feature-settings: 'liga' 0; font: 13.3333px monospace",
        vcx,
    );
    assert_eq!(c.italic, Some(false));
    assert_eq!(c.font_weight, Some(400));
    assert_eq!(c.line_height, Some(Len::Auto));
    assert_eq!(c.font_kerning, Some(2));
    assert_eq!(c.font_size_adjust, Some((u8::MAX, 0.0)));
    assert_eq!(c.font_settings, Some(Vec::new()));
    assert!(c.font_features.is_empty());
    assert_eq!(c.font_variant_set, 0b11111);
    assert_eq!(c.font_size, Some(Len::Px(13.3333)));
    assert_eq!(c.monospace, Some(true));
}

#[test]
fn font_shorthand_order_does_not_matter_against_its_longhands() {
    let doc = doc();
    let vcx = doc.values();
    let c = style("line-height: 3; font: 20px/1 serif; font-weight: 900", vcx);
    assert_eq!(c.font_weight, Some(900));
    assert_eq!(c.line_height, Some(Len::Pct(1.0)));
    let c = style("font-weight: 900; font: 20px serif", vcx);
    assert_eq!(c.font_weight, Some(400));
}

#[test]
fn invalid_font_shorthand_is_dropped_whole() {
    let doc = doc();
    let vcx = doc.values();
    for value in [
        "bold highlighted 100% serif",
        "4em/-2em serif",
        "20px",
        "italic italic 20px serif",
        "small-caps 20px 1Ahem",
        "20px serif, inherit",
    ] {
        let c = style(&format!("font: 30px Ahem; font: {value}"), vcx);
        assert_eq!(c.font_size, Some(Len::Px(30.0)), "{value}");
    }
}

#[test]
fn font_shorthand_prefix_in_any_order() {
    let doc = doc();
    let vcx = doc.values();
    let c = style("font: 900 oblique 10deg small-caps condensed 0 Ahem", vcx);
    assert_eq!(c.font_weight, Some(900));
    assert_eq!(c.font_oblique_angle, Some(10.0));
    assert_eq!(c.font_stretch, Some(75.0));
    assert_eq!(c.font_size, Some(Len::Px(0.0)));
    assert!(c.font_features.iter().any(|(t, _)| t == "smcp"));
    let c = style("font: normal normal 2em / 1.5 \"A B\", serif", vcx);
    assert_eq!(c.font_size, Some(Len::Em(2.0)));
    assert_eq!(c.line_height, Some(Len::Pct(1.5)));
}

#[test]
fn variant_groups_inherit_separately_and_font_resets_them() {
    let doc = doc();
    let vcx = doc.values();
    let parent = style("font-variant: small-caps tabular-nums", vcx);
    let child = style("font-variant-caps: normal", vcx);
    let c = inherit(
        &crate::style::computed::BoxStyle::from(parent.clone()),
        &child,
        vcx,
    );
    assert!(c.font_features.iter().any(|(t, _)| t == "tnum"));
    assert!(!c.font_features.iter().any(|(t, _)| t == "smcp"));
    let child = style("font: 16px serif", vcx);
    let c = inherit(
        &crate::style::computed::BoxStyle::from(parent.clone()),
        &child,
        vcx,
    );
    assert!(c.font_features.is_empty(), "{:?}", c.font_features);
    let child = style("font-variant-numeric: inherit", vcx);
    let c = inherit(
        &crate::style::computed::BoxStyle::from(parent.clone()),
        &child,
        vcx,
    );
    assert!(c.font_features.iter().any(|(t, _)| t == "tnum"));
}

#[test]
fn system_font_sets_size_and_family() {
    let doc = doc();
    let vcx = doc.values();
    let c = style("font: menu", vcx);
    assert_eq!(c.font_size, Some(Len::Px(12.0)));
    assert_eq!(c.line_height, Some(Len::Auto));
}

#[test]
fn var_in_font_shorthand_is_invalid_at_computed_value_time_as_a_whole() {
    let doc = doc();
    let vcx = doc.values();
    let mut vars = crate::style::css::VarMap::default();
    vars.insert("--s".into(), "20px".into());
    let mut c = Computed::default();
    c.apply_decls_with_vars(
        &crate::style::css::parse_decls_in("font: var(--s) Ahem", vcx),
        &vars,
        vcx,
    );
    assert_eq!(c.font_size, Some(Len::Px(20.0)));
    let mut c = Computed::default();
    c.apply_decls_with_vars(
        &crate::style::css::parse_decls_in("font-weight: 900; font: var(--s)", vcx),
        &vars,
        vcx,
    );
    // Негодное после подстановки — `unset` у каждого длинного свойства.
    assert_eq!(c.font_weight, None);
}
