//! Каскад фона и маски после раскрытия сокращений при разборе: сокращение
//! сбрасывает свои длинные свойства, длинное свойство пишет только себя,
//! негодное сокращение не значит ничего, `var()` в сокращении негоден на
//! вычислении целиком (css-backgrounds-3 §3.10, css-variables-1 §3.1),
//! `inherit` сокращения наследует каждое длинное свойство.

use super::*;
use crate::style::computed::ImageSource;
use crate::style::css::parse_decls_in;
use crate::style::values::value::Color;

fn style(css: &str, vcx: ValueCx<'_>) -> Computed {
    let mut c = Computed::default();
    c.apply_decls(&parse_decls_in(css, vcx), vcx);
    c
}

#[test]
fn shorthand_resets_longhands_and_longhands_override_in_order() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let red = Color::parse("red", vcx);
    let reset = style("background-color: red; background: url(a) no-repeat", vcx);
    assert_eq!(reset.background.filter(|c| c.a > 0.0), None);
    assert_eq!(reset.bg_image, Some(ImageSource::Url("a".into())));
    let over = style("background: url(a) green; background-color: red", vcx);
    assert_eq!(over.background, red);
    assert_eq!(over.bg_image, Some(ImageSource::Url("a".into())));
    let invalid = style("background: red; background: url(a) bogus", vcx);
    assert_eq!(invalid.background, red);
    assert_eq!(invalid.bg_image, None);
}

#[test]
fn initial_layer_values_compute_as_absent() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let c = style("background: url(a)", vcx);
    assert_eq!(c.bg_repeat, None);
    assert_eq!(c.bg_pos.x, None);
    assert_eq!(c.bg_pos.y, None);
    assert_eq!(c.bg_fixed, None);
    assert_eq!(c.bg_origin, None);
    assert_eq!(c.bg_clip, None);
    assert!(c.bg_lists.is_empty());
}

#[test]
fn layer_lists_belong_to_their_longhand() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let c = style(
        "background: url(a) no-repeat, url(b) repeat-x; background-repeat: space",
        vcx,
    );
    let layers = c.bg_layers(vcx).unwrap();
    assert_eq!(layers.len(), 2);
    assert!(
        layers
            .iter()
            .all(|l| l.bg_repeat == Some(BgRepeat::Axes(Tiling::Space, Tiling::Space)))
    );
    assert_eq!(layers[1].bg_image, Some(ImageSource::Url("b".into())));
}

#[test]
fn invalid_substitution_in_the_shorthand_unsets_every_longhand() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut vars = crate::style::css::VarMap::new();
    vars.insert("--c".into(), "bogus".into());
    let mut c = Computed::default();
    c.apply_decls_with_vars(
        &parse_decls_in("background-color: red; background: var(--c) url(a)", vcx),
        &vars,
        vcx,
    );
    assert_eq!(c.background.filter(|c| c.a > 0.0), None);
    assert_eq!(c.bg_image, None);
    vars.insert("--c".into(), "green".into());
    let mut c = Computed::default();
    c.apply_decls_with_vars(
        &parse_decls_in("background: var(--c) url(a)", vcx),
        &vars,
        vcx,
    );
    assert_eq!(c.background, Color::parse("green", vcx));
    assert_eq!(c.bg_image, Some(ImageSource::Url("a".into())));
}

#[test]
fn inherit_shorthand_inherits_each_layer_longhand() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let parent = style(
        "background: url(a) right 4px bottom no-repeat fixed content-box, url(b) red",
        vcx,
    );
    let own = style("background: inherit", vcx);
    let c = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
    assert_eq!(c.bg_image, parent.bg_image);
    assert_eq!(c.bg_pos, parent.bg_pos);
    assert_eq!(c.bg_repeat, parent.bg_repeat);
    assert_eq!(c.bg_fixed, Some(true));
    assert_eq!(c.bg_origin, parent.bg_origin);
    for prop in crate::style::computed::BgListProp::ALL {
        assert_eq!(c.bg_lists.get(prop), parent.bg_lists.get(prop));
    }
    assert_eq!(c.background, Color::parse("red", vcx));
}

#[test]
fn mask_shorthand_writes_its_longhands() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let c = style(
        "mask: url(m.svg) no-repeat content-box luminance, url(n.svg)",
        vcx,
    );
    assert_eq!(c.mask_image.as_ref().map(Vec::len), Some(2));
    assert_eq!(c.mask_origin, Some(3));
    assert_eq!(c.mask_luminance, Some(true));
    assert_eq!(c.mask_repeat_list, Some(vec![(true, true), (false, false)]));
    assert_eq!(c.mask_border, None);
    let b = style("mask-border: url(b.png) 30 / 10px round", vcx);
    assert!(b.mask_border.is_some());
    assert!(!b.mask_border_luminance);
}
