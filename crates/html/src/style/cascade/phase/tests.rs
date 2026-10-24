//! Фазы вычисления: список фаз, порядок применения и входы `ComputeCx`.

use super::Phase;
use crate::document::state::DocState;
use crate::style::cascade::vars::CascadeCx;
use crate::style::computed::Computed;
use crate::style::css::{Media, Origin, Rule, Selector, VarMap, parse_decls, parse_decls_in};
use crate::style::values::value::Len;

fn rule(css: &str, specificity_selector: &str) -> Rule {
    Rule {
        sel: Selector::parse(specificity_selector).unwrap(),
        decls: parse_decls(css),
        order: 0,
        origin: Origin::Author,
        layer: vec![u32::MAX],
        link: crate::style::css::LinkMatch::Any,
    }
}

const ROOT: CascadeCx<'static> = CascadeCx {
    attrs: &[],
    html_attrs: false,
    sibling: None,
    root: true,
};

#[test]
fn dependencies_come_before_their_consumers() {
    for early in [
        "writing-mode",
        "direction",
        "text-orientation",
        "zoom",
        "color-scheme",
    ] {
        assert_eq!(Phase::of(early), Phase::CascadeAffecting, "{early}");
    }
    for font in [
        "font-size",
        "font-width",
        "font-variant-caps",
        "font-family",
        "line-height",
        "font-size-adjust",
        "color",
        "math-depth",
    ] {
        assert_eq!(Phase::of(font), Phase::Font, "{font}");
    }
    for rest in [
        "margin-top",
        "border-block-start-radius",
        "width",
        "unicode-bidi",
    ] {
        assert_eq!(Phase::of(rest), Phase::Rest, "{rest}");
    }
    assert!(Phase::CascadeAffecting < Phase::Font && Phase::Font < Phase::Rest);
}

/// `direction` из более сильного блока (`style=""`) применяется ДО
/// логического радиуса из правила: сторона сопоставляется по итогу фазы
/// письма, а не по порядку блоков.
#[test]
fn logical_side_radius_maps_by_the_writing_phase_output() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let r = rule("border-inline-start-radius: 10px", "p");
    let inline = parse_decls_in("direction: rtl", vcx);
    let c = Computed::resolve(&[&r], &inline, vcx);
    assert_eq!(c.radius.tr, Some(Len::Px(10.0)));
    assert_eq!(c.radius.br, Some(Len::Px(10.0)));
    assert_eq!(c.radius.tl, None);
}

/// `rem` на корне: в собственном `font-size` — от начального кегля (а не
/// от метрик прошлого обхода), в остальных свойствах — от вычисленного
/// кегля корня (css-values-4 §6.1.4).
#[test]
fn root_rem_uses_the_root_font_phase() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let none = parse_decls_in("", vcx);
    let r = rule("font-size: 2rem; margin-left: 1rem", "html");
    for _ in 0..2 {
        let c = Computed::resolve_in(&[&r], &none, &VarMap::new(), ROOT, vcx);
        assert_eq!(c.font_size, Some(Len::Px(32.0)));
        assert_eq!(c.margin.left, Some(Len::Px(32.0)));
    }
    // `calc()` с единицами шрифта: родитель корня — начальные 16px.
    let r = rule("font-size: calc(1rem + 1em)", "html");
    Computed::resolve_in(&[&r], &none, &VarMap::new(), ROOT, vcx);
    let child = rule("width: 1rem", "div");
    let c = Computed::resolve(&[&child], &none, vcx);
    assert_eq!(c.width, Some(Len::Px(32.0)));
}

/// `color-scheme` решается раньше цветов того же элемента: `light-dark()` в
/// `color` берёт схему из более слабого блока (css-color-adjust-1 §2,
/// css-color-5 §light-dark), не из порядка блоков; без своего значения —
/// схема, с которой пришёл контекст родителя.
#[test]
fn color_scheme_decides_light_dark_of_the_same_element() {
    let doc = DocState::new(Media {
        dark: true,
        ..Media::default()
    });
    let vcx = doc.values();
    let r = rule("color-scheme: dark", "p");
    let inline = parse_decls_in("color: light-dark(rgb(255, 0, 0), rgb(0, 128, 0))", vcx);
    let c = Computed::resolve(&[&r], &inline, vcx);
    assert_eq!(c.color_scheme.as_deref(), Some("dark"));
    assert_eq!(c.color.map(|c| (c.r, c.g > 0.0)), Some((0.0, true)));
    let light = rule("color-scheme: only light", "p");
    let c = Computed::resolve(&[&light], &inline, vcx);
    assert_eq!(c.color_scheme.as_deref(), Some("only light"));
    assert_eq!(c.color.map(|c| c.r), Some(1.0));
    let none = parse_decls_in("color-scheme: only only light", vcx);
    assert_eq!(Computed::resolve(&[], &none, vcx).color_scheme, None);
}
