//! Style types for Flexbox layout
use super::{
    AlignContent, AlignItems, AlignSelf, CoreStyle, Dimension, JustifyContent, LengthPercentage,
    Style,
};
use crate::geometry::Size;

/// The set of styles required for a Flexbox container
pub trait FlexboxContainerStyle: CoreStyle {
    /// KaminIDE patch: the container is a table box (its used width is floored
    /// by its min-content width, CSS 2.1 §17.5.2).
    #[inline(always)]
    fn is_table_container(&self) -> bool {
        false
    }
    /// Which direction does the main axis flow in?
    #[inline(always)]
    fn flex_direction(&self) -> FlexDirection {
        Style::<Self::CustomIdent>::DEFAULT.flex_direction
    }
    /// Should elements wrap, or stay in a single line?
    #[inline(always)]
    fn flex_wrap(&self) -> FlexWrap {
        Style::<Self::CustomIdent>::DEFAULT.flex_wrap
    }
    /// KaminIDE patch: `flex-wrap: balance` (css-flexbox-2 §5.2) — 0 =
    /// обычный перенос; N ≥ 1 = балансировка строк с минимумом N строк
    /// (`flex-line-count`). Ортогонально режиму `flex_wrap`, как в Blink.
    #[inline(always)]
    fn flex_balance_lines(&self) -> u16 {
        Style::<Self::CustomIdent>::DEFAULT.flex_balance_lines
    }
    /// KaminIDE patch: поперечная ось ОДНОСТРОЧНОГО контейнера идёт
    /// от физического конца (cross-start справа): гибкая колонка при
    /// `direction: rtl` (css-flexbox-1 §2 «cross-start … inline-start»).
    /// Выравнивание читает это как `wrap-reverse`, перенос строк — нет.
    #[inline(always)]
    fn flex_cross_reverse(&self) -> bool {
        Style::<Self::CustomIdent>::DEFAULT.flex_cross_reverse
    }
    /// KaminIDE patch: анонимный ряд строки — доли высоты детей решаются от
    /// высоты родителя ряда, а не от самого ряда (CSS 2.1 §10.1).
    #[inline(always)]
    fn percent_basis_from_parent(&self) -> bool {
        Style::<Self::CustomIdent>::DEFAULT.percent_basis_from_parent
    }
    /// The minimum number of flex lines requested for a multi-line container. When items are
    /// balanced ([`FlexWrap::Balance`] or [`FlexWrap::BalanceReverse`]) they are balanced into
    /// at least this many lines. For any multi-line container, definite cross-axis available
    /// space for measuring items is divided between this many lines.
    #[cfg(feature = "flexbox_balance")]
    #[inline(always)]
    fn flex_line_count(&self) -> u16 {
        Style::<Self::CustomIdent>::DEFAULT.flex_line_count
    }

    /// How large should the gaps between items in a grid or flex container be?
    #[inline(always)]
    fn gap(&self) -> Size<LengthPercentage> {
        Style::<Self::CustomIdent>::DEFAULT.gap
    }

    // Alignment properties

    /// How should content contained within this item be aligned in the cross/block axis
    #[inline(always)]
    fn align_content(&self) -> AlignContent {
        Style::<Self::CustomIdent>::DEFAULT.align_content
    }
    /// How this node's children aligned in the cross/block axis?
    #[inline(always)]
    fn align_items(&self) -> AlignItems {
        Style::<Self::CustomIdent>::DEFAULT.align_items
    }
    /// How this node's children should be aligned in the inline axis
    #[inline(always)]
    fn justify_content(&self) -> JustifyContent {
        Style::<Self::CustomIdent>::DEFAULT.justify_content
    }
}

/// The set of styles required for a Flexbox item (child of a Flexbox container)
pub trait FlexboxItemStyle: CoreStyle {
    /// KaminIDE patch: элемент — анонимный ряд строки
    /// (`percent_basis_from_parent`): родителю надо отдать ему свою высоту
    /// как базу долей и в проходе основы.
    #[inline(always)]
    fn is_line_row(&self) -> bool {
        false
    }
    /// Sets the initial main axis size of the item
    #[inline(always)]
    fn flex_basis(&self) -> Dimension {
        Style::<Self::CustomIdent>::DEFAULT.flex_basis
    }
    /// The relative rate at which this item grows when it is expanding to fill space
    #[inline(always)]
    fn flex_grow(&self) -> f32 {
        Style::<Self::CustomIdent>::DEFAULT.flex_grow
    }
    /// The relative rate at which this item shrinks when it is contracting to fit into space
    #[inline(always)]
    fn flex_shrink(&self) -> f32 {
        Style::<Self::CustomIdent>::DEFAULT.flex_shrink
    }

    /// How this node should be aligned in the cross/block axis
    /// Falls back to the parents [`AlignItems`] if not set
    #[inline(always)]
    fn align_self(&self) -> Option<AlignSelf> {
        Style::<Self::CustomIdent>::DEFAULT.align_self
    }
    /// KaminIDE patch: элемент — наружная коробка таблицы; по главной оси он
    /// не ужимается ниже min-content своего содержимого (css-tables-3 §3.9).
    #[inline(always)]
    fn is_table_item(&self) -> bool {
        false
    }
    /// KaminIDE patch: `max-width`/`max-height: min-content | max-content`
    /// (css-sizing-3 §3.2), measured by the flex container.
    #[inline(always)]
    fn max_size_keywords(&self) -> crate::geometry::Size<Option<crate::style::AvailableSpace>> {
        crate::geometry::Size {
            width: None,
            height: None,
        }
    }
}

use crate::geometry::AbsoluteAxis;

/// Controls whether flex items are forced onto one line or can wrap onto multiple lines.
///
/// Defaults to [`FlexWrap::NoWrap`]
///
/// [Specification](https://www.w3.org/TR/css-flexbox-1/#flex-wrap-property)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FlexWrap {
    /// Items will not wrap and stay on a single line
    #[default]
    NoWrap,
    /// Items will wrap according to this item's [`FlexDirection`]
    Wrap,
    /// Items will wrap in the opposite direction to this item's [`FlexDirection`]
    WrapReverse,
    /// Items will wrap according to this item's [`FlexDirection`], and be balanced across
    /// lines such that the largest line is as small as possible
    ///
    /// [Specification](https://drafts.csswg.org/css-flexbox-2/#balance-values)
    #[cfg(feature = "flexbox_balance")]
    Balance,
    /// Items will wrap in the opposite direction to this item's [`FlexDirection`], and be
    /// balanced across lines such that the largest line is as small as possible
    ///
    /// [Specification](https://drafts.csswg.org/css-flexbox-2/#balance-values)
    #[cfg(feature = "flexbox_balance")]
    BalanceReverse,
}

impl FlexWrap {
    /// Is this a wrapping mode (any value other than [`FlexWrap::NoWrap`])?
    #[inline]
    pub(crate) fn is_multi_line(self) -> bool {
        self != Self::NoWrap
    }

    /// Do lines stack in the opposite direction to the cross axis?
    #[inline]
    pub(crate) fn is_reverse(self) -> bool {
        match self {
            Self::WrapReverse => true,
            #[cfg(feature = "flexbox_balance")]
            Self::BalanceReverse => true,
            _ => false,
        }
    }

    /// Are items balanced across lines?
    #[cfg(feature = "flexbox_balance")]
    #[inline]
    pub(crate) fn is_balance(self) -> bool {
        matches!(self, Self::Balance | Self::BalanceReverse)
    }
}

#[cfg(all(feature = "parse", not(feature = "flexbox_balance")))]
crate::util::parse::impl_parse_for_keyword_enum!(FlexWrap,
    "nowrap" => NoWrap,
    "wrap" => Wrap,
    "wrap-reverse" => WrapReverse,
);

#[cfg(all(feature = "parse", feature = "flexbox_balance"))]
impl crate::util::parse::FromCss for FlexWrap {
    fn from_css<'i>(
        input: &mut crate::util::parse::Parser<'i, '_>,
    ) -> crate::util::parse::CssParseResult<'i, Self> {
        let mut wrap: Option<Self> = None;
        let mut balance = false;
        let mut is_first = true;
        loop {
            let ident = if is_first {
                input.expect_ident()?.clone()
            } else {
                match input.try_parse(|input| input.expect_ident().cloned()) {
                    Ok(ident) => ident,
                    Err(_) => break,
                }
            };
            let is_valid = cssparser::match_ignore_ascii_case! { &ident,
                "nowrap" => {
                    if is_first {
                        return Ok(Self::NoWrap);
                    }
                    false
                },
                "wrap" => wrap.replace(Self::Wrap).is_none(),
                "wrap-reverse" => wrap.replace(Self::WrapReverse).is_none(),
                "balance" => !core::mem::replace(&mut balance, true),
                _ => false,
            };
            if !is_valid {
                return Err(
                    input.new_unexpected_token_error(crate::util::parse::Token::Ident(ident))
                );
            }
            is_first = false;
        }
        Ok(match (wrap, balance) {
            (Some(Self::WrapReverse), true) => Self::BalanceReverse,
            (_, true) => Self::Balance,
            // At least one keyword is always parsed, so `wrap` must be `Some` if `balance` is false
            (wrap, false) => wrap.unwrap_or(Self::NoWrap),
        })
    }
}
#[cfg(all(feature = "parse", feature = "flexbox_balance"))]
crate::util::parse::from_str_from_css!(FlexWrap);

/// The direction of the flexbox layout main axis.
///
/// There are always two perpendicular layout axes: main (or primary) and cross (or secondary).
/// Adding items will cause them to be positioned adjacent to each other along the main axis.
/// By varying this value throughout your tree, you can create complex axis-aligned layouts.
///
/// Items are always aligned relative to the cross axis, and justified relative to the main axis.
///
/// The default behavior is [`FlexDirection::Row`].
///
/// [Specification](https://www.w3.org/TR/css-flexbox-1/#flex-direction-property)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FlexDirection {
    /// Defines +x as the main axis
    ///
    /// Items will be added from left to right in a row.
    #[default]
    Row,
    /// Defines +y as the main axis
    ///
    /// Items will be added from top to bottom in a column.
    Column,
    /// Defines -x as the main axis
    ///
    /// Items will be added from right to left in a row.
    RowReverse,
    /// Defines -y as the main axis
    ///
    /// Items will be added from bottom to top in a column.
    ColumnReverse,
}

#[cfg(feature = "parse")]
crate::util::parse::impl_parse_for_keyword_enum!(FlexDirection,
    "row" => Row,
    "column" => Column,
    "row-reverse" => RowReverse,
    "column-reverse" => ColumnReverse,
);

impl FlexDirection {
    #[inline]
    /// Is the direction [`FlexDirection::Row`] or [`FlexDirection::RowReverse`]?
    pub(crate) const fn is_row(self) -> bool {
        matches!(self, Self::Row | Self::RowReverse)
    }

    #[inline]
    /// Is the direction [`FlexDirection::Column`] or [`FlexDirection::ColumnReverse`]?
    pub(crate) const fn is_column(self) -> bool {
        matches!(self, Self::Column | Self::ColumnReverse)
    }

    #[inline]
    /// Is the direction [`FlexDirection::RowReverse`] or [`FlexDirection::ColumnReverse`]?
    pub(crate) const fn is_reverse(self) -> bool {
        matches!(self, Self::RowReverse | Self::ColumnReverse)
    }

    #[inline]
    /// The `AbsoluteAxis` that corresponds to the main axis
    pub(crate) const fn main_axis(self) -> AbsoluteAxis {
        match self {
            Self::Row | Self::RowReverse => AbsoluteAxis::Horizontal,
            Self::Column | Self::ColumnReverse => AbsoluteAxis::Vertical,
        }
    }

    #[inline]
    /// The `AbsoluteAxis` that corresponds to the cross axis
    pub(crate) const fn cross_axis(self) -> AbsoluteAxis {
        match self {
            Self::Row | Self::RowReverse => AbsoluteAxis::Vertical,
            Self::Column | Self::ColumnReverse => AbsoluteAxis::Horizontal,
        }
    }
}

#[cfg(test)]
mod tests {
    mod test_flex_direction {
        use crate::style::*;

        #[test]
        fn flex_direction_is_row() {
            assert!(FlexDirection::Row.is_row());
            assert!(FlexDirection::RowReverse.is_row());
            assert!(!FlexDirection::Column.is_row());
            assert!(!FlexDirection::ColumnReverse.is_row());
        }

        #[test]
        fn flex_direction_is_column() {
            assert!(!FlexDirection::Row.is_column());
            assert!(!FlexDirection::RowReverse.is_column());
            assert!(FlexDirection::Column.is_column());
            assert!(FlexDirection::ColumnReverse.is_column());
        }

        #[test]
        fn flex_direction_is_reverse() {
            assert!(!FlexDirection::Row.is_reverse());
            assert!(FlexDirection::RowReverse.is_reverse());
            assert!(!FlexDirection::Column.is_reverse());
            assert!(FlexDirection::ColumnReverse.is_reverse());
        }
    }
}
