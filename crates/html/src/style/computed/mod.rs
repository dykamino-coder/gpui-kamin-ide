//! Вычисленный стиль узла: что получилось после каскада, до применения к GPUI.
//!
//! Промежуточная структура нужна по двум причинам. Во-первых, её видно в
//! тестах без окна и рендера — а `gpui::Style` собрать в тесте нельзя.
//! Во-вторых, ровно она задаёт границу охвата: поле есть — свойство
//! поддержано, поля нет — свойство игнорируется осознанно, а не потеряно.

mod bidi_properties;
mod border_color;
pub(crate) mod font_family;
mod font_kerning;
pub(super) mod font_members;
mod font_shorthand;
pub(super) mod font_weight;
mod gradient_paint;
mod image_color;
mod radius_mask;
mod radius_parse;
mod text_indent;
mod white_space;
pub(crate) use image_color::parse as parse_image_color;
mod content_functions;
mod counters;
mod list_style;
mod list_style_string;
mod mask_shorthand;
mod mask_size;
pub(crate) mod orthogonal;
mod quotes;
mod size_range;
mod tab_size;
pub(crate) use content_functions::parse_content;
mod outline_style;
use crate::style::computed::outline_style::parse as outline_style_of;
pub(crate) use outline_style::DOUBLE as OUTLINE_DOUBLE;
pub(super) mod props;
mod queries;
mod resolve;

use crate::style::values::value::Len;
mod fields;
pub use crate::style::computed::fields::*;
pub(super) mod transform;
pub use crate::style::computed::transform::*;
mod filter;
pub use crate::style::computed::filter::*;
pub(super) mod gradient;
pub(crate) use crate::style::computed::gradient::*;
pub(super) mod grid_tracks;
pub use crate::style::computed::grid_tracks::*;
pub(crate) use crate::style::computed::props::border::*;
pub(super) mod types;
pub use crate::style::computed::types::*;
pub(super) mod parse_util;
pub use crate::style::computed::parse_util::*;

/// Четыре стороны: `top right bottom left`, как в CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides {
    pub top: Option<Len>,
    pub right: Option<Len>,
    pub bottom: Option<Len>,
    pub left: Option<Len>,
}

impl Sides {
    /// Раскрытие сокращённой записи: 1 значение — все стороны, 2 — верт/гориз,
    /// 3 — верх/гориз/низ, 4 — по часовой.
    pub(crate) fn shorthand(raw: &str) -> Sides {
        // Разрез — по пробелам ВНЕ скобок: `calc(10px + 1%) 0 0 0` — четыре
        // значения, а не шесть обрывков (`calc-margin-block-1`). Смесь с долей
        // доживает индексом (`parse_mixed`) — раскладка складывает её сама.
        let v: Vec<Option<Len>> = split_outside_parens(raw)
            .iter()
            .map(|t| Len::parse_mixed(t))
            .collect();
        match v.len() {
            1 => Sides {
                top: v[0],
                right: v[0],
                bottom: v[0],
                left: v[0],
            },
            2 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[0],
                left: v[1],
            },
            3 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[1],
            },
            4 => Sides {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[3],
            },
            _ => Sides::default(),
        }
    }
}

/// Четыре угла скругления.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: Option<Len>,
    pub tr: Option<Len>,
    pub br: Option<Len>,
    pub bl: Option<Len>,
}

/// Разряды `inherit_bits`: ненаследуемые свойства, у которых слово `inherit`
/// обязано скопировать вычисленное значение родителя (§6.2.1).
pub(super) mod ainh {
    pub(crate) const ALIGN_ITEMS: u8 = 1 << 0;
    pub(crate) const JUSTIFY_ITEMS: u8 = 1 << 1;
    pub(crate) const ALIGN_CONTENT: u8 = 1 << 2;
    pub(crate) const JUSTIFY_CONTENT: u8 = 1 << 3;
    pub(crate) const JUSTIFY_SELF: u8 = 1 << 4;
}

pub(crate) mod inh {
    pub(crate) const BG_REPEAT: u32 = 1 << 0;
    pub(crate) const Z_INDEX: u32 = 1 << 1;
    pub(crate) const OUTLINE_W: u32 = 1 << 2;
    pub(crate) const DISPLAY: u32 = 1 << 3;
    pub(crate) const BG_IMAGE: u32 = 1 << 4;
    pub(crate) const BG_POS: u32 = 1 << 5;
    pub(crate) const CLIP: u32 = 1 << 6;
    pub(crate) const BG_ORIGIN: u32 = 1 << 7;
    pub(crate) const BG_CLIP: u32 = 1 << 8;
    pub(crate) const BG_SIZE: u32 = 1 << 9;
    pub(crate) const TRANSFORM: u32 = 1 << 10;
    pub(crate) const TRANSFORM_ORIGIN: u32 = 1 << 11;
    pub(crate) const OUTLINE_C: u32 = 1 << 12;
    pub(crate) const OUTLINE_S: u32 = 1 << 13;
    pub(crate) const OUTLINE_O: u32 = 1 << 14;
    /// `overflow-clip-margin: inherit` — коробка отсчёта и поле родителя.
    pub(crate) const CLIP_MARGIN: u32 = 1 << 15;
    /// `column-rule-color: inherit` — скалярный цвет и список линеек родителя.
    pub(crate) const COLUMN_RULE_C: u32 = 1 << 16;
    /// `row-rule-color: inherit`.
    pub(crate) const ROW_RULE_C: u32 = 1 << 17;
}

/// Разряды `will_change` (css-will-change-1 §2.1): чего ждать от коробки,
/// которая свойство только ОБЕЩАЕТ. «If any non-initial value of a property
/// would create a stacking context on the element, specifying that property
/// in will-change must create a stacking context on the element» — и то же
/// дословно про содержащий блок для `absolute` и для `fixed`.
pub(crate) mod wc {
    /// Содержащий блок для `position: absolute`.
    pub(crate) const CB_ABS: u8 = 1 << 0;
    /// Содержащий блок для `position: fixed`.
    pub(crate) const CB_FIXED: u8 = 1 << 1;
    /// Контекст наложения.
    pub(crate) const STACK: u8 = 1 << 2;
    /// `z-index`: контекст только там, где `z-index` действует
    /// (позиционированная коробка, элемент flex/grid) — решает
    /// `inline::inherit`, где известен вид родителя.
    pub(crate) const STACK_Z: u8 = 1 << 3;
    /// Обещано свойство семьи `transform` или `contain`: к строчной
    /// НЕатомарной коробке они не применяются (css-transforms-1
    /// «transformable element»), поэтому три разряда выше ставит `dom::walk`,
    /// когда вид коробки уже известен (`will-change-transform-inline`).
    pub(crate) const BOX: u8 = 1 << 4;
}

#[cfg(test)]
mod border_image_tests;
#[cfg(test)]
mod gradient_tests;
#[cfg(test)]
mod tests;
