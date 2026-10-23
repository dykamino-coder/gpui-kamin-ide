//! Каскад: начальные значения словами для `initial`/`unset`/`revert` у свойств
//! ВНЕ реестра (`style::css::property`). Наследуемых среди них нет: все
//! наследуемые свойства — длинные свойства реестра, их `unset` и `initial`
//! решает реестр (`apply_wide`).

/// Начальное значение свойства словами — для `initial`/`unset`/`revert`.
///
/// Значения взяты из спецификаций; свойства, начальное значение которых у нас
/// и так «поле не задано», сюда не входят — им сброс не нужен.
pub(super) fn initial_value(key: &str) -> Option<&'static str> {
    Some(match key {
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
        "scrollbar-width" => "auto",
        "border-radius" => "0",
        "corner-shape" => "round",
        "border-shape" => "none",
        "vertical-align" => "baseline",
        _ => return None,
    })
}
