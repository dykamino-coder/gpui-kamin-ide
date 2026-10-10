//! Типы оформления текста: линии, положение подчёркивания, стиль/толщина/смещение и их разбор.

use super::*;

/// Линии украшения (css-text-decor-3 §2.1 `text-decoration-line`).
pub const DECOR_UNDER: u8 = 1;

pub const DECOR_OVER: u8 = 2;

pub const DECOR_THROUGH: u8 = 4;

/// `text-underline-position` (css-text-decor-4 §5.2).
pub const UPOS_UNDER: u8 = 1;

pub const UPOS_LEFT: u8 = 2;

pub const UPOS_RIGHT: u8 = 4;

pub const UPOS_FROM_FONT: u8 = 8;

/// `text-decoration-style` (css-text-decor-3 §2.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DecorStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Длина украшения: толщина, смещение подчёркивания, отступ концов.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DecorLen {
    #[default]
    Auto,
    FromFont,
    /// Абсолютная длина в точках CSS.
    Px(f32),
    /// Доля (1.0 = 100%): у толщины и смещения — от кегля, у отступа — от
    /// ширины украшаемого прогона.
    Pct(f32),
    /// Как задано (единицы шрифта решаются при наследовании).
    Raw(Len),
    /// `calc(доля + точки)` отступа концов: доля от ширины прогона.
    Mix(f32, f32),
}

/// Шрифт украшающей коробки: от него берутся метрики линий
/// (css-text-decor-3 §2.1 «decorating box»).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecorFont {
    pub family: Option<String>,
    pub monospace: Option<bool>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub stretch: Option<f32>,
    pub size: f32,
}

/// Украшение, наложенное украшающей коробкой (Blink `AppliedTextDecoration`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decor {
    pub lines: u8,
    pub style: DecorStyle,
    pub color: Color,
    /// Толщина: `Auto`, `FromFont` или `Px`.
    pub thickness: DecorLen,
    /// `text-underline-offset`: `Auto` или `Px`.
    pub offset: DecorLen,
    pub position: u8,
    /// `None` — `auto`; иначе (начало, конец): `Px` или `Pct`.
    pub inset: Option<[DecorLen; 2]>,
    pub clone: bool,
    pub font: DecorFont,
    /// Язык украшающей коробки — японский, корейский или монгольский: в вертикальном
    /// письме подчёркивание по умолчанию справа (Blink
    /// `ResolveUnderlinePosition`, css-text-decor-3 §default-stylesheet).
    pub over_lang: bool,
}

pub(in crate::style::computed) fn parse_decor_style(t: &str) -> Option<DecorStyle> {
    Some(match t {
        "solid" => DecorStyle::Solid,
        "double" => DecorStyle::Double,
        "dotted" => DecorStyle::Dotted,
        "dashed" => DecorStyle::Dashed,
        "wavy" => DecorStyle::Wavy,
        _ => return None,
    })
}

/// `<length-percentage>` украшения (толщина, смещение, отступ концов).
pub(in crate::style::computed) fn parse_decor_length(t: &str) -> Option<DecorLen> {
    if t == "auto" || t == "normal" {
        return None;
    }
    let l = Len::parse_spacing(t)?;
    Some(match l {
        Len::Px(v) => DecorLen::Px(v),
        Len::Pct(k) => DecorLen::Pct(k),
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => return None,
        other => DecorLen::Raw(other),
    })
}

/// `text-decoration-thickness`: `auto | from-font | <length-percentage> |
/// <line-width>` (css-text-decor-4 §2.4).
pub(in crate::style::computed) fn parse_decor_thickness(t: &str) -> Option<DecorLen> {
    Some(match t {
        "auto" => DecorLen::Auto,
        "from-font" => DecorLen::FromFont,
        "thin" => DecorLen::Px(1.0),
        "medium" => DecorLen::Px(3.0),
        "thick" => DecorLen::Px(5.0),
        _ => parse_decor_length(t)?,
    })
}
