//! Наследуемые свойства и начальные значения.
// owner: B

/// Начальное значение свойства словами — для `initial`/`unset`/`revert`.
///
/// Значения взяты из спецификаций; свойства, начальное значение которых у нас
/// и так «поле не задано», сюда не входят — им сброс не нужен.
/// Наследуется ли свойство по умолчанию (столбец «Inherited» таблиц
/// свойств CSS). Нужен `unset`: у наследуемого он значит `inherit`.
pub(crate) fn inherited_property(key: &str) -> bool {
    matches!(
        key,
        "color"
            | "font"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "font-stretch"
            | "letter-spacing"
            | "word-spacing"
            | "line-height"
            | "text-align"
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
            | "text-orientation"
            | "border-collapse"
            | "border-spacing"
            | "caption-side"
            | "empty-cells"
    )
}

pub(crate) fn initial_value(key: &str) -> Option<&'static str> {
    Some(match key {
        "border" => "0 none",
        "border-radius" => "0",
        "corner-shape" => "round",
        "border-shape" => "none",
        "color" => "black",
        "margin" => "0",
        "padding" => "0",
        "direction" => "ltr",
        "font-family" => "serif",
        "font-size" => "medium",
        "font-style" => "normal",
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
        "text-align" => "start",
        "text-justify" => "auto",
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
