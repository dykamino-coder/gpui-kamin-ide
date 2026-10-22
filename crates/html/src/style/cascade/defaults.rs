//! Каскад: таблицы свойств для общих ключевых слов. Какие свойства наследуются
//! по умолчанию (столбец «Inherited» спецификаций — для `unset`, который у них
//! значит `inherit`) и начальные значения словами для `initial`/`unset`/`revert`.

/// Наследуется ли свойство по умолчанию (столбец «Inherited» таблиц
/// свойств CSS). Нужен `unset`: у наследуемого он значит `inherit`.
pub(super) fn inherited_property(key: &str) -> bool {
    matches!(
        key,
        "scrollbar-color"
            | "color"
            | "font"
            | "font-family"
            | "font-size"
            | "font-style"
            | "math-style"
            | "font-variant"
            | "font-weight"
            | "font-stretch"
            | "letter-spacing"
            | "word-spacing"
            | "line-height"
            | "text-align"
            | "text-align-all"
            | "image-rendering"
            | "text-align-last"
            | "text-indent"
            | "text-transform"
            | "visibility"
            | "white-space"
            | "list-style"
            | "list-style-type"
            | "list-style-position"
            | "list-style-image"
            | "direction"
            | "writing-mode"
            | "quotes"
            | "cursor"
            | "tab-size"
            | "word-break"
            | "overflow-wrap"
            | "word-wrap"
            | "hyphens"
            | "hyphenate-character"
            | "hyphenate-limit-chars"
            | "hyphenate-limit-lines"
            | "text-orientation"
            | "border-collapse"
            | "border-spacing"
            | "caption-side"
            | "empty-cells"
    )
}

/// Начальное значение свойства словами — для `initial`/`unset`/`revert`.
///
/// Значения взяты из спецификаций; свойства, начальное значение которых у нас
/// и так «поле не задано», сюда не входят — им сброс не нужен.
pub(super) fn initial_value(key: &str) -> Option<&'static str> {
    Some(match key {
        // css-position-3 §insets: начальное значение вставок — `auto`
        // (`left: unset` в `@position-try` снимает вставку базы).
        "top" | "right" | "bottom" | "left" | "inset" | "inset-block" | "inset-inline"
        | "inset-block-start" | "inset-block-end" | "inset-inline-start" | "inset-inline-end" => {
            "auto"
        }
        "scroll-snap-type" | "scroll-snap-align" | "path-length" => "none",
        "scroll-padding"
        | "scroll-padding-top"
        | "scroll-padding-right"
        | "scroll-padding-bottom"
        | "scroll-padding-left" => "auto",
        "scroll-margin"
        | "scroll-margin-top"
        | "scroll-margin-right"
        | "scroll-margin-bottom"
        | "scroll-margin-left" => "0",
        "scrollbar-color" | "scrollbar-width" => "auto",
        "border" => "0 none",
        "border-radius" => "0",
        "corner-shape" => "round",
        "border-shape" => "none",
        "color" => "black",
        "cursor" => "auto",
        // css-backgrounds-3 §4.3: the initial border width is `medium`.
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            "medium"
        }
        "hyphens" => "manual",
        "text-shadow" => "none",
        "margin" => "0",
        // css-box-4 §4.1: a padding side starts at 0 — an explicit value, so the
        // author keyword also beats a form control's default padding.
        "padding" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => "0",
        "direction" => "ltr",
        "font-family" => "serif",
        "font-size" => "medium",
        "font-style" => "normal",
        // MathML Core §4.2: initial must stop inheriting compact style.
        "math-style" => "normal",
        "font-variant" => "normal",
        "font-weight" => "normal",
        "letter-spacing" => "normal",
        "line-break" => "auto",
        "line-height" => "normal",
        "list-style-position" => "outside",
        "list-style-type" => "disc",
        "quotes" => "auto",
        "overflow-wrap" | "word-wrap" => "normal",
        "tab-size" => "8",
        "text-align" | "text-align-all" => "start",
        "image-rendering" => "auto",
        "hyphenate-limit-lines" => "no-limit",
        "text-justify" | "hyphenate-character" | "hyphenate-limit-chars" => "auto",
        "text-combine-upright" => "none",
        "text-indent" => "0",
        "text-orientation" => "mixed",
        "text-transform" => "none",
        "vertical-align" => "baseline",
        "visibility" => "visible",
        "white-space" => "normal",
        "word-break" => "normal",
        "word-spacing" => "normal",
        "writing-mode" => "horizontal-tb",
        _ => return None,
    })
}
