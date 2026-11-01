//! CSS-операнды группы background; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::{Color, StyleColor};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetBackground {
    /// USED background color (paint-time): derived from the computed
    /// `background_color` only by `Computed::derive_background` (on the own
    /// style for absolute values, at the merge for `currentcolor`-dependent
    /// and inherited values). Used-value stages rewrite it afterwards: canvas
    /// propagation (`document::canvas_background`), flat-stack compositing
    /// (`flat_background`), `filter` painting and `::first-line`/hover/animation
    /// styles. Never inherited, never read as a computed value.
    pub background: Option<Color>,
    /// Фон ЗАЯВЛЕН автором (`background` или `background-color`), пусть даже
    /// прозрачным. Умолчание UA у полей ввода ставится только тогда, когда
    /// автор не сказал ничего: сокращение `background: linear-gradient(...)`
    /// сбрасывает цвет в прозрачный (§2.1), и подставлять поверх него
    /// служебную заливку нельзя.
    pub bg_explicit: bool,
    /// COMPUTED `background-color` (css-backgrounds-3 §3.2, css-color-4
    /// §6.4): `None` — initial `transparent`. `currentcolor` and functions of
    /// it stay unresolved; `inherit` copies this value, never the parent's
    /// used `background`.
    pub background_color: Option<StyleColor>,
    pub gradient: Option<Gradient>,
    /// Сырая запись градиента фона: источник для слоя-картинки там, где
    /// градиент рисуется плиткой (фон ряда таблицы).
    pub gradient_raw: Option<ImageSource>,
    /// Authored currentColor expression, independent of background tiling.
    pub gradient_current: Option<Gradient>,
    /// CSS Compositing 1: each image blends with the isolated background stack.
    pub background_blend: Option<BlendMode>,
    /// `background-attachment: fixed` — плитка считается от области
    /// просмотра, а не от коробки.
    pub bg_fixed: Option<bool>,
    /// `background-clip`: до какого края красится фон. `None` — до внешнего
    /// края рамки, как по умолчанию в CSS.
    pub bg_clip: Option<BgClip>,
    /// `background-origin`: от какого края коробки отсчитывается фоновая
    /// картинка. `None` — от внутреннего края рамки, как по умолчанию в CSS.
    pub bg_origin: Option<BgClip>,
    /// Collapsed table borders occupy padding in layout, but remain border
    /// outsets for background positioning and clipping: top/right/bottom/left.
    pub bg_border_insets: Option<[f32; 4]>,
    /// `background-image: url(...)` — ссылка на картинку-заливку.
    pub bg_image: Option<ImageSource>,
    pub bg_size: BgSize,
    pub bg_pos: BgPos,
    /// Percentage comparisons wait for the possibly negative image free space.
    pub bg_position_math: [Option<super::background_position::PositionExpression>; 2],
    /// Сырые СПИСКИ фоновых свойств со слоями через запятую: (свойство,
    /// запись). Поля `bg_*` несут верхний слой; все слои строит `bg_layers`.
    pub bg_lists: BgLists,
    pub bg_repeat: Option<BgRepeat>,
}
