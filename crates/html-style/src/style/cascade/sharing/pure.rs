//! Context-free declaration values: the style-sharing cache may reuse a cascade
//! only when every value is a function of its text and the environment key.

/// Простые цветовые функции и градиенты (css-images-3 §3) чистые: `currentcolor`
/// стопов и длины в `em` решает `compute` по элементу после общего каскада.
/// Вложенные функции проверяются тем же правилом; неизвестная — обычный путь.
pub(super) fn pure_value(value: &str) -> bool {
    if value.contains('\\') {
        return false;
    }
    value.match_indices('(').all(|(at, _)| {
        let name = value[..at]
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .next()
            .unwrap_or("");
        let colors = [
            "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
        ];
        colors.iter().any(|pure| name.eq_ignore_ascii_case(pure)) || gradient_name(name)
    })
}

/// `linear-`, `radial-`, `conic-gradient` and their `repeating-` forms (ASCII name).
fn gradient_name(name: &str) -> bool {
    let Some(kind) = name
        .len()
        .checked_sub(9)
        .filter(|&at| name[at..].eq_ignore_ascii_case("-gradient"))
    else {
        return false;
    };
    let kind = &name[..kind];
    let kind = match kind.get(..10) {
        Some(head) if head.eq_ignore_ascii_case("repeating-") => &kind[10..],
        _ => kind,
    };
    ["linear", "radial", "conic"]
        .iter()
        .any(|k| kind.eq_ignore_ascii_case(k))
}
