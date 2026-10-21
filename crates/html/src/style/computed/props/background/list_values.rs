//! Проверка всех слоёв до изменения одиночного значения и сохранённых списков.

use super::*;

impl Computed {
    pub(crate) fn background_list_valid(prop: BgListProp, v: &str, vcx: ValueCx<'_>) -> bool {
        if matches!(
            v,
            "inherit" | "initial" | "unset" | "revert" | "revert-layer"
        ) {
            return true;
        }
        if prop == BgListProp::BlendMode {
            return background_blend_valid(v);
        }
        let layers = background_layers(v);
        layers.iter().enumerate().all(|(i, value)| {
            // Первый слой длинного свойства разбирает его типизированный обработчик.
            if i == 0 && prop != BgListProp::Shorthand {
                return true;
            }
            match prop {
                BgListProp::Image => image_valid(value, vcx),
                BgListProp::Size => size_valid(value, vcx),
                BgListProp::Position => layer_props::position_valid(value, vcx),
                BgListProp::Repeat => repeat_valid(value),
                BgListProp::Origin => {
                    matches!(*value, "border-box" | "padding-box" | "content-box")
                }
                BgListProp::Clip => matches!(
                    *value,
                    "border-box" | "padding-box" | "content-box" | "text" | "border-area"
                ),
                BgListProp::Attachment => matches!(*value, "scroll" | "fixed" | "local"),
                BgListProp::Shorthand => shorthand_valid(value, i + 1 == layers.len(), vcx),
                BgListProp::BlendMode => unreachable!(),
            }
        })
    }
}

fn image_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    if v == "none" {
        return true;
    }
    if let Some(inner) = v
        .strip_prefix("light-dark(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let args = crate::style::css::split_args(inner);
        return args.len() == 2 && args.iter().all(|a| image_valid(a.trim(), vcx));
    }
    ImageSource::parse(v, vcx).is_some()
}

fn size_valid(v: &str, vcx: ValueCx<'_>) -> bool {
    if matches!(v, "cover" | "contain") {
        return true;
    }
    let words = split_outside_parens(v);
    (1..=2).contains(&words.len())
        && words.iter().all(|w| {
            size_range::parse_nonnegative_size(w, vcx)
                .is_some_and(|l| !matches!(l, Len::MinContent | Len::MaxContent | Len::FitContent))
        })
}

fn repeat_valid(v: &str) -> bool {
    if matches!(v, "repeat-x" | "repeat-y") {
        return true;
    }
    let words: Vec<_> = v.split_whitespace().collect();
    (1..=2).contains(&words.len())
        && words
            .iter()
            .all(|w| matches!(*w, "repeat" | "no-repeat" | "space" | "round"))
}

fn shorthand_valid(v: &str, last: bool, vcx: ValueCx<'_>) -> bool {
    if image_valid(v, vcx) {
        return true;
    }
    let words = split_outside_parens(v);
    !words.is_empty()
        && words.iter().all(|word| {
            // Косая черта между положением и размером может быть без пробелов.
            let tokens: Vec<_> = if word.contains('(') {
                vec![word.as_str()]
            } else {
                word.split('/').collect()
            };
            tokens.iter().all(|w| {
                w.is_empty()
                    || matches!(
                        *w,
                        "none"
                            | "auto"
                            | "no-repeat"
                            | "repeat"
                            | "repeat-x"
                            | "repeat-y"
                            | "space"
                            | "round"
                            | "cover"
                            | "contain"
                            | "scroll"
                            | "fixed"
                            | "local"
                            | "border-box"
                            | "padding-box"
                            | "content-box"
                            | "text"
                            | "left"
                            | "right"
                            | "top"
                            | "bottom"
                            | "center"
                    )
                    || Len::parse_mixed(w, vcx).is_some()
                    || image_valid(w, vcx)
                    || (last
                        && (Color::parse(w, vcx).is_some()
                            || crate::style::values::color_space::ColorExpression::parse(w, vcx)
                                .is_some()))
            })
        })
}
