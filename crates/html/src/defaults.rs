//! Умолчания CSS/HTML и выбранные движком резервные значения.

/// UA-значение `font-size: medium` для разрешения длины без шрифта.
/// CSS2 §15.7 задаёт `medium` относительно таблицы UA, а не обязательные 16 px.
/// https://www.w3.org/TR/CSS2/fonts.html#font-size-props
pub(crate) const MEDIUM_FONT_SIZE_PX: f32 = 16.0;

/// Резерв `line-height: normal`, когда метрики шрифта недоступны.
/// CSS2 §10.8.1 рекомендует 1.0–1.2; обычный путь использует метрики шрифта.
/// https://www.w3.org/TR/CSS2/visudet.html#propdef-line-height
pub(crate) const FALLBACK_NORMAL_LINE_HEIGHT: f32 = 1.2;

/// Ширина replaced без собственных размеров: CSS2 §10.3.2.
/// https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-width
pub(crate) const DEFAULT_REPLACED_WIDTH_PX: f32 = 300.0;

/// Высота replaced без собственных размеров: CSS2 §10.6.2.
/// Вместе с §10.3.2 задаёт резерв 300 × 150 CSS px.
/// https://www.w3.org/TR/CSS2/visudet.html#inline-replaced-height
pub(crate) const DEFAULT_REPLACED_HEIGHT_PX: f32 = 150.0;

/// Начальный bitmap canvas: HTML §4.12.5, такие же 300 × 150 CSS px.
/// https://html.spec.whatwg.org/multipage/canvas.html#the-canvas-element
pub(crate) const DEFAULT_CANVAS_SIZE_PX: (f32, f32) = (300.0, 150.0);

/// Прямоугольник replaced для алгоритмов default object size (CSS Images §4.3.1).
/// https://www.w3.org/TR/css-images-3/#default-sizing
pub(crate) const DEFAULT_REPLACED_SIZE_PX: (f32, f32) =
    (DEFAULT_REPLACED_WIDTH_PX, DEFAULT_REPLACED_HEIGHT_PX);
