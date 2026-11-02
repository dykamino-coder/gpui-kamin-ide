//! CSS-операнды группы ui; значение за общим COW-указателем.
use crate::style::values::value::{Color, ColorOrAuto};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedUi {
    pub color: Option<Color>,
    /// `image-rendering` (css-images-3 §image-rendering), наследуется:
    /// `Some(true)` — `pixelated`/`crisp-edges`, растр масштабируется без
    /// сглаживания; `Some(false)` — `auto`/`smooth`/`high-quality`.
    pub image_pixelated: Option<bool>,
    /// Форма курсора: у GPUI набор совпадает с CSS почти буква в букву.
    pub cursor: Option<String>,
    /// `visibility: hidden` — место занимает, но не рисуется.
    pub hidden: Option<bool>,
    /// `image-orientation: none` (css-images-3 §5.4) — НЕ разворачивать растр
    /// по метке EXIF. Начальное значение свойства — `from-image`, поэтому
    /// хранится именно отказ, а не разрешение.
    pub image_orient_none: Option<bool>,
    pub pointer_events_none: Option<bool>,
    /// Вычисленное `color-scheme` (css-color-adjust-1 §2.1): `normal` или
    /// список схем через пробел (`light dark`, `only light`); `None` у
    /// каскадного стиля — не задано (наследуется).
    pub color_scheme: Option<String>,
    /// `caret-color` поля ввода, наследуемое: `None` — не задано (у слитого
    /// стиля — начальное `auto`).
    pub caret_color: Option<ColorOrAuto>,
    /// `accent-color` флажков и переключателей, наследуемое (css-ui-4
    /// §accent-color): `None` — не задано (у слитого стиля — `auto`).
    pub accent_color: Option<ColorOrAuto>,
    pub math_compact: Option<bool>,
    pub math_shift: Option<bool>,
    pub math_level: i32,
    pub math_operator_form: Option<u8>,
}
