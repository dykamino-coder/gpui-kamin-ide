//! Разбор значений SVG paint и cursor до изменения вычисленного стиля.

use super::*;

pub(super) fn paint_value(v: &str, vcx: ValueCx<'_>) -> Option<String> {
    (matches!(
        v,
        "none" | "context-fill" | "context-stroke" | "currentcolor"
    ) || Color::parse(v, vcx).is_some()
        || (v.starts_with("url(") && v.ends_with(')')))
    .then(|| v.to_string())
}

pub(super) fn cursor_value(v: &str) -> Option<String> {
    let words: Vec<_> = crate::style::css::split_args(v);
    let keyword = words.last()?.trim();
    if !matches!(
        keyword,
        "auto"
            | "default"
            | "none"
            | "context-menu"
            | "help"
            | "pointer"
            | "progress"
            | "wait"
            | "cell"
            | "crosshair"
            | "text"
            | "vertical-text"
            | "alias"
            | "copy"
            | "move"
            | "no-drop"
            | "not-allowed"
            | "grab"
            | "grabbing"
            | "all-scroll"
            | "col-resize"
            | "row-resize"
            | "n-resize"
            | "e-resize"
            | "s-resize"
            | "w-resize"
            | "ne-resize"
            | "nw-resize"
            | "se-resize"
            | "sw-resize"
            | "ew-resize"
            | "ns-resize"
            | "nesw-resize"
            | "nwse-resize"
            | "zoom-in"
            | "zoom-out"
    ) {
        return None;
    }
    if !words[..words.len() - 1]
        .iter()
        .all(|w| w.trim().starts_with("url(") && w.contains(')'))
    {
        return None;
    }
    Some(v.to_string())
}

impl Computed {
    pub(super) fn apply_paint_value(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) -> bool {
        // Возврат обработчика не зависит от валидности: неизвестное значение
        // известного свойства остаётся отклонённой декларацией.
        match key {
            "cursor" => {
                if self.css_wide_value {
                    self.accept_declaration();
                    self.cursor = None;
                } else if let Some(value) = cursor_value(v) {
                    self.accept_declaration();
                    self.cursor = Some(value);
                }
            }
            "fill" | "stroke" => {
                if let Some(value) = paint_value(v, vcx) {
                    self.accept_declaration();
                    if key == "fill" {
                        self.svg_fill = Some(value);
                    } else {
                        self.svg_stroke = Some(value);
                    }
                }
            }
            "stroke-width" => {
                if size_range::parse_nonnegative_size(v, vcx)
                    .filter(|l| {
                        !matches!(
                            l,
                            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
                        )
                    })
                    .is_none()
                    && v.parse::<f32>()
                        .ok()
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .is_none()
                {
                    return true;
                }
                self.accept_declaration();
                self.svg_stroke_width =
                    Some(match crate::style::values::value::calc_pct_px(v, vcx) {
                        Some((0.0, px)) => format!("{px}"),
                        _ => v.to_string(),
                    });
            }
            _ => return false,
        }
        true
    }
}

/// Проверка всех имён и числовых операндов списка счётчиков.
pub(super) fn counter_value(v: &str, reversed: bool) -> bool {
    if matches!(v, "none" | "inherit") {
        return true;
    }
    let words = split_outside_parens(v);
    let mut words = words.iter().peekable();
    let mut any = false;
    while let Some(word) = words.next() {
        let name = if reversed {
            word.strip_prefix("reversed(")
                .and_then(|w| w.strip_suffix(')'))
                .unwrap_or(word)
        } else {
            word
        };
        if !crate::style::css::selector_tokens::ident(name)
            || matches!(
                name,
                "none" | "default" | "initial" | "inherit" | "unset" | "revert" | "revert-layer"
            )
        {
            return false;
        }
        any = true;
        if let Some(next) = words.peek() {
            let digits = next.strip_prefix(['+', '-']).unwrap_or(next);
            if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit())
                || ["calc(", "min(", "max(", "clamp("]
                    .iter()
                    .any(|p| next.starts_with(p))
            {
                words.next();
            }
        }
    }
    any
}
