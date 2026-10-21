//! Таблица длинных свойств реестра: имя, начальное значение (css-box-4,
//! css-position-3, css-sizing-3, css-logical-1), наследование, грамматика и
//! место в логической группе (css-logical-1 §4: логическое и физическое
//! свойство одной стороны делят вычисленное значение).

use crate::document::state::ValueCx;

/// Грамматика значения длинного свойства; проверка — тем же разбором, что
/// у применения (`Computed::accepts_longhand`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grammar {
    /// `<length-percentage> | auto | <anchor-size()>` (css-box-4 §3.1).
    Margin,
    /// `<length-percentage [0,∞]>` (css-box-4 §4.1).
    Padding,
    /// `auto | <length-percentage> | <anchor()> | <anchor-size()>`
    /// (css-position-3 §3.1, css-anchor-position-1 §4).
    Inset,
    /// Размеры коробки (css-sizing-3 §3.1-3.2, css-sizing-4, css-values-5
    /// `calc-size()`).
    Size,
}

/// Логическая группа (Stylo `logical_group`): физические и логические
/// свойства одной группы делят поля стиля.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxGroup {
    Margin,
    Padding,
    Inset,
    Size,
}

/// Место свойства в группе. Стороны: физические `top, right, bottom,
/// left`, логические `inline-start, inline-end, block-start, block-end`.
/// Размеры: физические `width, height, min-width, min-height, max-width,
/// max-height`, логические `inline, block, min-inline, min-block,
/// max-inline, max-block`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Physical(BoxGroup, usize),
    Logical(BoxGroup, usize),
}

macro_rules! longhands {
    ($($id:ident = $name:literal, $initial:literal, $inherited:literal, $grammar:ident, $slot:expr;)*) => {
        /// Длинное свойство реестра.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum LonghandId { $($id),* }

        impl LonghandId {
            /// Все длинные свойства в порядке таблицы.
            pub const ALL: &'static [LonghandId] = &[$(LonghandId::$id),*];
            const NAMES: &'static [&'static str] = &[$($name),*];
            const INITIAL: &'static [&'static str] = &[$($initial),*];
            const INHERITED: &'static [bool] = &[$($inherited),*];
            const GRAMMAR: &'static [Grammar] = &[$(Grammar::$grammar),*];
            const SLOT: &'static [Slot] = &[$($slot),*];

            /// Свойство реестра по имени (строчными, как его хранит блок).
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(LonghandId::$id),)*
                    _ => None,
                }
            }
        }
    };
}

use BoxGroup::{Inset as I, Margin as M, Padding as P, Size as S};
use Slot::{Logical as L, Physical as F};

longhands! {
    MarginTop = "margin-top", "0", false, Margin, F(M, 0);
    MarginRight = "margin-right", "0", false, Margin, F(M, 1);
    MarginBottom = "margin-bottom", "0", false, Margin, F(M, 2);
    MarginLeft = "margin-left", "0", false, Margin, F(M, 3);
    MarginInlineStart = "margin-inline-start", "0", false, Margin, L(M, 0);
    MarginInlineEnd = "margin-inline-end", "0", false, Margin, L(M, 1);
    MarginBlockStart = "margin-block-start", "0", false, Margin, L(M, 2);
    MarginBlockEnd = "margin-block-end", "0", false, Margin, L(M, 3);
    PaddingTop = "padding-top", "0", false, Padding, F(P, 0);
    PaddingRight = "padding-right", "0", false, Padding, F(P, 1);
    PaddingBottom = "padding-bottom", "0", false, Padding, F(P, 2);
    PaddingLeft = "padding-left", "0", false, Padding, F(P, 3);
    PaddingInlineStart = "padding-inline-start", "0", false, Padding, L(P, 0);
    PaddingInlineEnd = "padding-inline-end", "0", false, Padding, L(P, 1);
    PaddingBlockStart = "padding-block-start", "0", false, Padding, L(P, 2);
    PaddingBlockEnd = "padding-block-end", "0", false, Padding, L(P, 3);
    Top = "top", "auto", false, Inset, F(I, 0);
    Right = "right", "auto", false, Inset, F(I, 1);
    Bottom = "bottom", "auto", false, Inset, F(I, 2);
    Left = "left", "auto", false, Inset, F(I, 3);
    InsetInlineStart = "inset-inline-start", "auto", false, Inset, L(I, 0);
    InsetInlineEnd = "inset-inline-end", "auto", false, Inset, L(I, 1);
    InsetBlockStart = "inset-block-start", "auto", false, Inset, L(I, 2);
    InsetBlockEnd = "inset-block-end", "auto", false, Inset, L(I, 3);
    Width = "width", "auto", false, Size, F(S, 0);
    Height = "height", "auto", false, Size, F(S, 1);
    MinWidth = "min-width", "auto", false, Size, F(S, 2);
    MinHeight = "min-height", "auto", false, Size, F(S, 3);
    MaxWidth = "max-width", "none", false, Size, F(S, 4);
    MaxHeight = "max-height", "none", false, Size, F(S, 5);
    InlineSize = "inline-size", "auto", false, Size, L(S, 0);
    BlockSize = "block-size", "auto", false, Size, L(S, 1);
    MinInlineSize = "min-inline-size", "auto", false, Size, L(S, 2);
    MinBlockSize = "min-block-size", "auto", false, Size, L(S, 3);
    MaxInlineSize = "max-inline-size", "none", false, Size, L(S, 4);
    MaxBlockSize = "max-block-size", "none", false, Size, L(S, 5);
}

impl LonghandId {
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// Начальное значение (css-cascade-5 §7.3.1) — текстом той же
    /// грамматики, что и объявление.
    pub fn initial(self) -> &'static str {
        Self::INITIAL[self as usize]
    }

    /// Наследуется ли по умолчанию: решает `unset` (§7.3.3).
    pub fn inherited(self) -> bool {
        Self::INHERITED[self as usize]
    }

    pub fn grammar(self) -> Grammar {
        Self::GRAMMAR[self as usize]
    }

    pub fn slot(self) -> Slot {
        Self::SLOT[self as usize]
    }

    /// Годится ли значение (не общее слово и без подстановки) этому
    /// свойству: негодное объявление отбрасывается разбором (CSS Syntax
    /// §4.1.7 / css-cascade-5 §4) и в каскад не попадает.
    pub fn accepts(self, value: &str, vcx: ValueCx<'_>) -> bool {
        crate::style::computed::Computed::accepts_longhand(self, value, vcx)
    }
}
