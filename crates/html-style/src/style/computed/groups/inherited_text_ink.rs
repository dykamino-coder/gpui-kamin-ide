//! CSS-операнды группы text_ink; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::StyleColor;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedTextInk {
    /// `text-shadow`: смещение, размытие и цвет — ПЕРВАЯ (верхняя) тень списка.
    pub text_shadow: Option<Shadow>,
    /// Остальные тени `text-shadow` в порядке записи: свойство — СПИСОК, и
    /// «the first shadow is on top» (css-text-decor-3 Overview.bs:881-882).
    /// Первая живёт в `text_shadow`: на неё смотрят зум и наследование.
    /// Прежде хвост отбрасывался — `-1em 0em orange, 1em 0em blue` у эталона
    /// `box-shadow-multiple-001-ref` рисовал одну оранжевую.
    pub text_shadow_rest: Vec<Shadow>,
    /// Знак акцента (`text-emphasis-style`, css-text-decor-3 §5): рисуется
    /// над каждым знаком базы, как надстрочная аннотация руби.
    pub text_emphasis: Option<String>,
    /// Omitted emphasis shape resolves in the element's typographic mode.
    pub emphasis_auto_shape: bool,
    /// Explicit position, including language-sensitive `auto`; None inherits.
    pub emphasis_position_auto: Option<bool>,
    /// Акцент СНИЗУ (`text-emphasis-position: under`).
    pub emphasis_under: bool,
    /// Акцент СЛЕВА в вертикальном письме (`text-emphasis-position: … left`).
    pub emphasis_left: bool,
    /// `text-emphasis-color` (css-text-decor-3 §5.2), наследуемое: `None` —
    /// не задано (у слитого стиля — начальное `currentcolor`).
    pub emphasis_color: Option<StyleColor>,
    /// `text-underline-offset` (наследуется).
    pub underline_offset: Option<DecorLen>,
    /// `text-underline-position` (наследуется), биты `UPOS_*`.
    pub underline_pos: Option<u8>,
    /// `text-decoration-skip-ink` (наследуется): 0 `none`, 1 `auto`, 2 `all`.
    pub skip_ink: Option<u8>,
    /// `text-decoration-skip-spaces` (наследуется): 1 `start`, 2 `end`,
    /// 4 `all`, 0 `none`; пусто — начальное `start end`.
    pub skip_spaces: Option<u8>,
}
