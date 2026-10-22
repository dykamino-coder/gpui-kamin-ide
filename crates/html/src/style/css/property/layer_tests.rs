//! Раскрытие сокращений слоёв: `background`, `background-position`, `mask`,
//! `mask-border` (css-backgrounds-4 §background, §3.6; css-masking-1 §7.9,
//! §8.7).

use super::*;
use crate::document::state::DocState;
use crate::style::css::Media;

fn doc() -> std::rc::Rc<DocState> {
    DocState::new(Media::default())
}

fn texts(s: ShorthandId, v: &str, vcx: ValueCx<'_>) -> Option<Vec<(LonghandId, String)>> {
    Some(
        s.expand(v, Some(vcx))?
            .into_iter()
            .map(|(id, _, t)| (id, t))
            .collect(),
    )
}

fn get(list: &[(LonghandId, String)], id: LonghandId) -> &str {
    &list.iter().find(|(i, _)| *i == id).unwrap().1
}

#[test]
fn background_layers_fill_every_longhand_and_color_comes_from_the_last() {
    let doc = doc();
    let vcx = doc.values();
    let l = texts(
        ShorthandId::Background,
        "url(a) right 10px top / 5px no-repeat content-box, url(b) fixed green",
        vcx,
    )
    .unwrap();
    assert_eq!(get(&l, LonghandId::BackgroundImage), "url(a), url(b)");
    assert_eq!(get(&l, LonghandId::BackgroundPositionX), "right 10px, 0%");
    assert_eq!(get(&l, LonghandId::BackgroundPositionY), "top, 0%");
    assert_eq!(get(&l, LonghandId::BackgroundSize), "5px, auto");
    assert_eq!(get(&l, LonghandId::BackgroundRepeat), "no-repeat, repeat");
    assert_eq!(get(&l, LonghandId::BackgroundAttachment), "scroll, fixed");
    assert_eq!(
        get(&l, LonghandId::BackgroundOrigin),
        "content-box, padding-box"
    );
    assert_eq!(
        get(&l, LonghandId::BackgroundClip),
        "content-box, border-box"
    );
    assert_eq!(get(&l, LonghandId::BackgroundColor), "green");
}

#[test]
fn background_rejects_misplaced_or_repeated_parts() {
    let doc = doc();
    let vcx = doc.values();
    let bg = |v: &str| ShorthandId::Background.expand(v, Some(vcx));
    assert!(bg("red, url(a)").is_none(), "color only in the final layer");
    assert!(bg("url(a) url(b)").is_none());
    assert!(bg("left repeat top").is_none(), "position is one component");
    assert!(bg("/ 10px").is_none());
    assert!(bg("url(a),").is_none());
    assert!(bg("red green").is_none());
    assert!(bg("padding-box border-box content-box").is_none());
    assert!(bg("bogus").is_none());
}

#[test]
fn background_boxes_set_origin_and_clip() {
    let doc = doc();
    let vcx = doc.values();
    let one = texts(ShorthandId::Background, "padding-box", vcx).unwrap();
    assert_eq!(get(&one, LonghandId::BackgroundOrigin), "padding-box");
    assert_eq!(get(&one, LonghandId::BackgroundClip), "padding-box");
    let two = texts(ShorthandId::Background, "border-box content-box", vcx).unwrap();
    assert_eq!(get(&two, LonghandId::BackgroundOrigin), "border-box");
    assert_eq!(get(&two, LonghandId::BackgroundClip), "content-box");
    let text = texts(ShorthandId::Background, "text red", vcx).unwrap();
    assert_eq!(get(&text, LonghandId::BackgroundOrigin), "border-box");
    assert_eq!(get(&text, LonghandId::BackgroundClip), "text");
}

#[test]
fn background_with_a_substitution_is_pending_on_every_longhand() {
    let doc = doc();
    let vcx = doc.values();
    let out = ShorthandId::Background
        .expand("var(--c) url(a)", Some(vcx))
        .unwrap();
    assert_eq!(out.len(), 9);
    assert!(
        out.iter()
            .all(|(_, s, _)| *s == Specified::Pending(Some(ShorthandId::Background)))
    );
    let s = ShorthandId::Background;
    assert_eq!(
        s.component_of(LonghandId::BackgroundColor, "green url(a)", vcx)
            .as_deref(),
        Some("green")
    );
    assert_eq!(
        s.component_of(LonghandId::BackgroundColor, "green green", vcx),
        None
    );
}

#[test]
fn background_position_splits_into_axes() {
    let doc = doc();
    let vcx = doc.values();
    let axes = |v: &str| {
        texts(ShorthandId::BackgroundPosition, v, vcx).map(|l| (l[0].1.clone(), l[1].1.clone()))
    };
    let pair = |x: &str, y: &str| Some((x.to_string(), y.to_string()));
    assert_eq!(axes("top"), pair("center", "top"));
    assert_eq!(axes("10px"), pair("10px", "center"));
    assert_eq!(axes("bottom left"), pair("left", "bottom"));
    assert_eq!(axes("10px 20%"), pair("10px", "20%"));
    assert_eq!(axes("center bottom 5px"), pair("center", "bottom 5px"));
    assert_eq!(axes("bottom 5px right"), pair("right", "bottom 5px"));
    assert_eq!(
        axes("left 1px top 2px, 3px"),
        pair("left 1px, 3px", "top 2px, center")
    );
    assert_eq!(axes("top 10px"), None);
    assert_eq!(axes("left right"), None);
    assert_eq!(axes("center 10px top 5px"), None);
}

#[test]
fn mask_expands_layers_and_resets_the_mask_border() {
    let doc = doc();
    let vcx = doc.values();
    let l = texts(
        ShorthandId::Mask,
        "url(m.svg) center / contain no-repeat content-box no-clip luminance subtract, none",
        vcx,
    )
    .unwrap();
    assert_eq!(get(&l, LonghandId::MaskImage), "url(m.svg), none");
    assert_eq!(get(&l, LonghandId::MaskPosition), "center center, 0% 0%");
    assert_eq!(get(&l, LonghandId::MaskSize), "contain, auto");
    assert_eq!(get(&l, LonghandId::MaskRepeat), "no-repeat, repeat");
    assert_eq!(get(&l, LonghandId::MaskOrigin), "content-box, border-box");
    assert_eq!(get(&l, LonghandId::MaskClip), "no-clip, border-box");
    assert_eq!(get(&l, LonghandId::MaskMode), "luminance, match-source");
    assert_eq!(get(&l, LonghandId::MaskComposite), "subtract, add");
    assert_eq!(get(&l, LonghandId::MaskBorderSlice), "0");
    assert!(
        ShorthandId::Mask
            .expand("url(a) url(b)", Some(vcx))
            .is_none()
    );
    assert!(
        ShorthandId::Mask
            .expand("left 10px top", Some(vcx))
            .is_none()
    );
}

#[test]
fn mask_border_parts_follow_the_slash_grammar() {
    let doc = doc();
    let vcx = doc.values();
    let l = texts(
        ShorthandId::MaskBorder,
        "url(b.png) 30 fill / 10px / 2px round luminance",
        vcx,
    )
    .unwrap();
    assert_eq!(get(&l, LonghandId::MaskBorderSource), "url(b.png)");
    assert_eq!(get(&l, LonghandId::MaskBorderSlice), "30 fill");
    assert_eq!(get(&l, LonghandId::MaskBorderWidth), "10px");
    assert_eq!(get(&l, LonghandId::MaskBorderOutset), "2px");
    assert_eq!(get(&l, LonghandId::MaskBorderRepeat), "round");
    assert_eq!(get(&l, LonghandId::MaskBorderMode), "luminance");
    assert!(
        ShorthandId::MaskBorder
            .expand("url(a) / 2px", Some(vcx))
            .is_none()
    );
}

#[test]
fn layer_longhands_validate_every_layer() {
    let doc = doc();
    let vcx = doc.values();
    let ok = |id: LonghandId, v: &str| id.accepts(v, vcx);
    assert!(ok(LonghandId::BackgroundRepeat, "repeat-x, space round"));
    assert!(!ok(LonghandId::BackgroundRepeat, "repeat-x round"));
    assert!(!ok(LonghandId::BackgroundSize, "-1px"));
    assert!(ok(LonghandId::BackgroundSize, "auto 10%, cover"));
    assert!(!ok(LonghandId::BackgroundClip, "border-box,"));
    assert!(ok(LonghandId::BackgroundPositionX, "right 10px, center"));
    assert!(!ok(LonghandId::BackgroundPositionX, "top"));
    assert!(ok(LonghandId::MaskClip, "no-clip, view-box"));
    assert!(!ok(LonghandId::MaskOrigin, "no-clip"));
    assert!(!ok(LonghandId::BackgroundColor, "url(a)"));
}
