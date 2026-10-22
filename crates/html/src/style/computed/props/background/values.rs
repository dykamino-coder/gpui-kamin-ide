//! Грамматики длинных свойств фона и маски (`LonghandId::accepts`; разбор
//! и применение берут ОДНИ функции): слой картинки, `<repeat-style>`,
//! `<bg-size>`, привязка, коробки, оси положения (css-backgrounds-3/4 §3),
//! части слоя маски (css-masking-1 §7) и `mask-border-*` (§8). Значение
//! свойства слоёв — список через запятую, годный, только когда годен каждый
//! слой.

use crate::document::state::ValueCx;
use crate::style::computed::{
    BgClip, BgRepeat, BgSize, Computed, ImageSource, ParseResult, Tiling,
};
use crate::style::css::LonghandId::{self, *};
use crate::style::values::value::{Len, StyleColor};

/// Годится ли значение длинному свойству фона или маски.
pub(crate) fn accepts(id: LonghandId, value: &str, vcx: ValueCx<'_>) -> bool {
    match id {
        BackgroundColor => StyleColor::parse_value(value, vcx).is_some(),
        MaskBorderSource | MaskBorderSlice | MaskBorderWidth | MaskBorderOutset
        | MaskBorderRepeat => {
            let mut parser = Computed {
                declaration_parse: ParseResult::Invalid,
                ..Computed::default()
            };
            parser.set_mask_border(id, value, vcx);
            parser.declaration_parse == ParseResult::Accepted
        }
        MaskBorderMode => matches!(value.trim(), "alpha" | "luminance"),
        _ => crate::style::css::layer_list(value)
            .is_some_and(|layers| layers.iter().all(|l| layer_ok(id, l, vcx))),
    }
}

fn layer_ok(id: LonghandId, layer: &str, vcx: ValueCx<'_>) -> bool {
    let l = layer.trim();
    match id {
        BackgroundImage | MaskImage => image_layer(l, vcx).is_some(),
        BackgroundPositionX => axis_ok(l, 0, vcx),
        BackgroundPositionY => axis_ok(l, 1, vcx),
        BackgroundSize | MaskSize => bg_size(l, vcx).is_some(),
        BackgroundRepeat | MaskRepeat => repeat_style(l).is_some(),
        BackgroundAttachment => attachment(l).is_some(),
        BackgroundOrigin => origin_box(l).is_some(),
        BackgroundClip => clip_box(l).is_some(),
        MaskMode => matches!(l, "alpha" | "luminance" | "match-source"),
        MaskPosition => position_ok(l, vcx),
        MaskOrigin => coord_box(l).is_some(),
        MaskClip => l == "no-clip" || coord_box(l).is_some(),
        MaskComposite => matches!(l, "add" | "subtract" | "intersect" | "exclude"),
        _ => false,
    }
}

/// Слой картинки: `none` — `Some(None)`.
pub(crate) fn image_layer(v: &str, vcx: ValueCx<'_>) -> Option<Option<ImageSource>> {
    if v.trim().eq_ignore_ascii_case("none") {
        return Some(None);
    }
    ImageSource::parse(v, vcx).map(Some)
}

/// `<repeat-style>` (css-backgrounds-3 §3.4): одно слово задаёт обе оси,
/// два — каждой свою.
pub(crate) fn repeat_style(v: &str) -> Option<BgRepeat> {
    let word = |w: &str| match w {
        "no-repeat" => Some(Tiling::None),
        "space" => Some(Tiling::Space),
        "round" => Some(Tiling::Round),
        "repeat" => Some(Tiling::Repeat),
        _ => None,
    };
    let words: Vec<String> = v.split_whitespace().map(str::to_ascii_lowercase).collect();
    match words.as_slice() {
        [x] if x == "repeat-x" => Some(BgRepeat::RepeatX),
        [y] if y == "repeat-y" => Some(BgRepeat::RepeatY),
        [x] => word(x).map(|t| BgRepeat::Axes(t, t)),
        [x, y] => Some(BgRepeat::Axes(word(x)?, word(y)?)),
        _ => None,
    }
}

/// `<bg-size>` (css-backgrounds-3 §3.9): `cover | contain | [<length-
/// percentage [0,∞]> | auto]{1,2}`; отрицательная длина негодна.
pub(crate) fn bg_size(v: &str, vcx: ValueCx<'_>) -> Option<BgSize> {
    match v.trim().to_ascii_lowercase().as_str() {
        "cover" => return Some(BgSize::Cover),
        "contain" => return Some(BgSize::Contain),
        _ => {}
    }
    let words = crate::style::computed::split_outside_parens(v);
    if !(1..=2).contains(&words.len()) {
        return None;
    }
    let one = |w: &str| -> Option<Option<Len>> {
        if w.eq_ignore_ascii_case("auto") {
            return Some(None);
        }
        match Len::parse_mixed(w, vcx)? {
            Len::Px(n) | Len::Pct(n) if n < 0.0 => None,
            l => Some(Some(l)),
        }
    };
    let w = one(&words[0])?;
    let h = match words.get(1) {
        Some(word) => one(word)?,
        None => None,
    };
    Some(match (w, h) {
        (None, None) => BgSize::Auto,
        (w, h) => BgSize::Fixed(w, h),
    })
}

/// `<attachment>` (css-backgrounds-3 §3.5): `fixed` — `true`.
pub(crate) fn attachment(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "fixed" => Some(true),
        "scroll" | "local" => Some(false),
        _ => None,
    }
}

/// `<visual-box>` для `background-origin` (§3.8): `padding-box`, начальное
/// значение, — `Some(None)`.
pub(crate) fn origin_box(v: &str) -> Option<Option<BgClip>> {
    match v.trim().to_ascii_lowercase().as_str() {
        "border-box" => Some(Some(BgClip::BorderBox)),
        "padding-box" => Some(None),
        "content-box" => Some(Some(BgClip::ContentBox)),
        _ => None,
    }
}

/// `<bg-clip>` (css-backgrounds-4 §3.7): `border-box`, начальное значение,
/// — `Some(None)`.
pub(crate) fn clip_box(v: &str) -> Option<Option<BgClip>> {
    match v.trim().to_ascii_lowercase().as_str() {
        "border-box" => Some(None),
        "padding-box" => Some(Some(BgClip::PaddingBox)),
        "content-box" => Some(Some(BgClip::ContentBox)),
        "text" => Some(Some(BgClip::Text)),
        "border-area" => Some(Some(BgClip::BorderArea)),
        _ => None,
    }
}

/// Ось положения слоя (css-backgrounds-4 §3.6): `center | [ <край> ]?
/// <length-percentage>? `, край своей оси.
pub(crate) fn axis_ok(v: &str, axis: usize, vcx: ValueCx<'_>) -> bool {
    let words = crate::style::computed::split_outside_parens(v);
    let edge = |w: &str| match axis {
        0 => matches!(w, "left" | "right"),
        _ => matches!(w, "top" | "bottom"),
    };
    let length = |w: &str| Computed::background_position_length(w, vcx);
    match words.as_slice() {
        [w] => w == "center" || edge(w) || length(w),
        [e, l] => edge(e) && length(l),
        _ => false,
    }
}

/// `<position>` слоя маски (css-values-4 §9.1: одно, два или четыре слова).
fn position_ok(v: &str, vcx: ValueCx<'_>) -> bool {
    let words = crate::style::css::layer_tokens(v);
    let w = crate::style::css::LayerWords { vcx: Some(vcx) };
    words.iter().all(|t| w.position(t))
        && crate::style::css::split_position(&words, false).is_some()
}

/// `<coord-box>` маски (css-masking-1 §7.4): код коробки `mask_origin`.
pub(crate) fn coord_box(v: &str) -> Option<u8> {
    Some(match v.trim() {
        "border-box" => 0,
        "padding-box" => 2,
        "content-box" => 3,
        "fill-box" => 4,
        "stroke-box" => 5,
        "view-box" => 6,
        _ => return None,
    })
}
