//! Приведение атрибута к типу CSS Values 5.

use super::*;

/// Значение атрибута под типом `attr()`; `None` — не разбирается этим типом.
pub(super) fn attr_cast(value: &str, ty: &str, vcx: ValueCx<'_>) -> Option<String> {
    let v = value.trim();
    let ty: String = ty
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    // `<length>`: голое число длиной не бывает, кроме нуля (css-values-4
    // §6.1); `Len::parse` принимает его точками — отсекаем здесь.
    let length = |v: &str| match Len::parse(v, vcx) {
        Some(Len::Px(n)) if v.parse::<f32>().is_ok() => (n == 0.0).then(|| v.to_string()),
        Some(
            Len::Pct(_)
            | Len::Auto
            | Len::MinContent
            | Len::MaxContent
            | Len::FitContent
            | Len::Anchor(_),
        )
        | None => None,
        Some(_) => Some(v.to_string()),
    };
    let percentage = |v: &str| {
        v.strip_suffix('%')
            .and_then(|n| n.trim().parse::<f32>().ok())
            .map(|_| v.to_string())
    };
    match ty.as_str() {
        // Токены; attr-taint проверяется после выбора значения или запаса.
        "type(*)" => Some(v.to_string()),
        "type(<length>)" => length(v),
        "type(<percentage>)" => percentage(v),
        "type(<length-percentage>)" => length(v).or_else(|| percentage(v)),
        "type(<number>)" | "number" => v.parse::<f32>().ok().map(|_| v.to_string()),
        "type(<integer>)" => v.parse::<i64>().ok().map(|_| v.to_string()),
        "type(<color>)" => Color::parse(v, vcx).map(|_| v.to_string()),
        // `attr(x px)` — число из атрибута с единицей из записи.
        unit if !unit.is_empty()
            && (unit == "%" || unit.chars().all(|c| c.is_ascii_alphabetic())) =>
        {
            v.parse::<f32>().ok().map(|_| format!("{v}{unit}"))
        }
        _ => None,
    }
}
