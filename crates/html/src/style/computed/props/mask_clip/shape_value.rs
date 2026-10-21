//! Проверка вида и границ функции формы до записи геометрии.

pub(in crate::style::computed::props) fn shape_valid(v: &str) -> bool {
    let boxes = |w: &str| {
        matches!(
            w,
            "margin-box"
                | "border-box"
                | "padding-box"
                | "content-box"
                | "fill-box"
                | "stroke-box"
                | "view-box"
        )
    };
    let Some(open) = v.find('(') else {
        return boxes(v) || v == "none";
    };
    let Some(end) = crate::style::css::block_end(v, open, '(') else {
        return false;
    };
    matches!(
        v[..open].trim(),
        "circle" | "ellipse" | "polygon" | "inset" | "rect" | "xywh" | "path" | "shape" | "url"
    ) && (v[end + 1..].trim().is_empty() || boxes(v[end + 1..].trim()))
}

/// CSS 2.1 §11.1.2: четыре auto/length; проценты и смешанные разделители запрещены.
pub(super) fn clip_rect(
    v: &str,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Option<[Option<crate::style::values::value::Len>; 4]>> {
    use crate::style::values::value::Len;
    if v == "auto" {
        return Some(None);
    }
    let body = v.strip_prefix("rect(")?.strip_suffix(')')?;
    let words = if body.contains(',') {
        body.split(',').map(str::trim).collect::<Vec<_>>()
    } else {
        body.split_whitespace().collect::<Vec<_>>()
    };
    if words.len() != 4 {
        return None;
    }
    let mut sides = [None; 4];
    for (slot, word) in sides.iter_mut().zip(words) {
        *slot = if word == "auto" {
            None
        } else {
            Some(Len::parse(word, vcx).filter(|l| {
                !matches!(
                    l,
                    Len::Auto | Len::Pct(_) | Len::MinContent | Len::MaxContent | Len::FitContent
                )
            })?)
        };
    }
    Some(Some(sides))
}
