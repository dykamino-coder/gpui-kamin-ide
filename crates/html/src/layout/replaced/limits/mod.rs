//! Пределы размеров замещаемых элементов.
// owner: A

use crate::dom::Element;
use crate::render::replaced_tag;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod font_clamp;
pub(crate) use font_clamp::atom_base_font;
pub(crate) use font_clamp::canvas_limit_keywords;
pub(super) use font_clamp::css2_replaced_limits;
pub(crate) use font_clamp::with_inherited_font;

/// Ключевое слово содержимого в `min-width`/`max-width` при `width` в точках.
///
/// css-sizing-3 §4.1 даёт `min-content`/`max-content`/`fit-content` и
/// минимуму, и максимуму, а used size — зажим предпочтительного размера
/// пределами (§5.1; CSS 2.1 §10.4): `width: W; min-width: kw` = max(W, kw)
/// = `width: kw; min-width: W`, `width: W; max-width: kw` = min(W, kw) =
/// `width: kw; max-width: W`. Ключевое слово переезжает в `width`, где его
/// знает обёртка-сетка (`content_sized`), а точки — в предел, который
/// раскладке отдаёт `apply` (`min-content-min-width-000`,
/// `shrink-to-fit-sizing-max-width-min-content`, `fit-content-{min,max}-
/// inline-size`, `block-size-with-min-or-max-content-6/7`).
///
/// Второй предел обязан быть пуст: с ним порядок зажима (минимум сильнее
/// максимума) одной перестановкой не выражается. Таблица считает пределы
/// сама (`min_fix`), замещаемый — в `image_with`; их не трогаем.
pub(crate) fn content_limit_swapped(e: &Element) -> Option<Element> {
    let kw = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    let unset = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let c = &e.style;
    let Some(w @ Len::Px(_)) = c.width else {
        return None;
    };
    if replaced_tag(e)
        || e.tag == "table"
        || matches!(c.display, Some(Display::Table) | Some(Display::InlineTable))
    {
        return None;
    }
    let mut copy = e.clone();
    if kw(c.min_width) && unset(c.max_width) {
        copy.style.width = c.min_width;
        copy.style.min_width = Some(w);
        // Довод `fit-content(N)` едет вместе с ключевым словом.
        copy.style.fit_arg[0] = c.fit_arg[1];
    } else if kw(c.max_width) && unset(c.min_width) {
        // `max-width: fit-content(<доля>)`: во вкладе доля циклична, и предел
        // ведёт себя как `fit-content(none)` = `max-content` (комментарий
        // `fit-content-length-percentage-013/016`: «the result max-width is
        // equal to max-content size»). При определённом блоке это ближе к
        // `max-content`, чем к истине (`-010`: 124 вместо 100, как и без
        // довода), но обёртка-сетка определённость базы не различает.
        copy.style.width = match (c.max_width, c.fit_arg[2]) {
            (Some(Len::FitContent), Some(Len::Pct(_))) => Some(Len::MaxContent),
            _ => c.max_width,
        };
        copy.style.max_width = Some(w);
        copy.style.fit_arg[0] = c.fit_arg[2].filter(|a| !matches!(a, Len::Pct(_)));
    } else {
        return None;
    }
    copy.style.intrinsic_wrapper_required = true;
    Some(copy)
}

/// Потолок блочного размера для `line-clamp: auto` (css-overflow-4
/// §line-clamp, «auto clamp point»): размер, который коробка получила бы при
/// БЕСКОНЕЧНОМ автоматическом размере — заданная `height`, ограниченная
/// `max-height`, а без `height` — сам `max-height`; `min-height` поднимает
/// результат (`line-clamp-auto-005`: `height: 4.5lh`; `-014`: максимум из
/// `min-height` и `max-height`). Бесконечный размер — точки нет.
pub(crate) fn auto_clamp_limit(m: &Computed) -> Option<f32> {
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let max_h = px(m.max_height);
    let base = match px(m.height) {
        Some(h) => Some(max_h.map_or(h, |mx| h.min(mx))),
        None => max_h,
    }?;
    Some(px(m.min_height).map_or(base, |mn| base.max(mn)))
}

/// Флаг «responsive embedded sizing» документа (css-sizing-4
/// §iframe-frame-sizing): истина, если `<meta name=responsive-embedded-sizing>`
/// встретился при разборе РАНЬШЕ, чем открылся `<body>` (явно или неявно —
/// любым тегом тела или непробельным текстом).
pub(super) fn responsive_embedded_sizing(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    let b = lower.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if lower[i..].starts_with("<!--") {
            match lower[i + 4..].find("-->") {
                Some(k) => i += 4 + k + 3,
                None => return false,
            }
            continue;
        }
        if b[i] != b'<' {
            if !b[i].is_ascii_whitespace() && b[i] != 0xef && b[i] != 0xbb && b[i] != 0xbf {
                return false;
            }
            i += 1;
            continue;
        }
        let Some(end) = lower[i..].find('>').map(|k| i + k) else {
            return false;
        };
        let tag = &lower[i + 1..end];
        i = end + 1;
        if tag.starts_with('!') || tag.starts_with('?') || tag.starts_with('/') {
            continue;
        }
        let name: String = tag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        match name.as_str() {
            "html" | "head" | "link" | "base" | "basefont" | "bgsound" => {}
            "meta" => {
                let compact: String = tag.chars().filter(|c| *c != '"' && *c != '\'').collect();
                if compact.contains("name=responsive-embedded-sizing") {
                    return true;
                }
            }
            "style" | "script" | "title" | "noscript" | "template" => {
                let close = format!("</{name}");
                match lower[i..].find(&close) {
                    Some(k) => i += k,
                    None => return false,
                }
            }
            _ => return false,
        }
    }
    false
}
