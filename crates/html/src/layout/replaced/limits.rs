//! Пределы размеров замещаемых элементов.
// owner: A

use crate::dom::Element;
use crate::render::{RenderOpts, replaced_tag};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

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
pub(crate) fn responsive_embedded_sizing(html: &str) -> bool {
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

/// Кегль для разрешения долей на атоме: свой размер шрифта уже разрешён в
/// слитом стиле, иначе — базовый.
/// Кусок с УНАСЛЕДОВАННЫМ шрифтом для разрешения шрифтовых единиц.
///
/// `image_with` разрешает `em`/`ex`/`ch` по стилю САМОГО куска, а гарнитуры у
/// `<img>` своей нет — метрика бралась запасная (пол-кегля вместо настоящего
/// роста строчной), и `height: 0.25ex` при `font: 250px/1 Ahem` давало 31
/// точку вместо 50 (`units-003`: оранжевый квадрат не совпадал с навесными
/// прямоугольниками).
pub(crate) fn with_inherited_font(e: &Element, inherited: &Computed) -> Element {
    let mut copy = e.clone();
    if copy.style.font_family.is_none() {
        copy.style.font_family = inherited.font_family.clone();
    }
    if copy.style.monospace.is_none() {
        copy.style.monospace = inherited.monospace;
    }
    // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4, «Inherited: yes»),
    // а копия несёт только собственный стиль элемента. Замещаемая коробка
    // строится ИМЕННО ИЗ НЕЁ: и растр (`background::key(src, &e.style)`), и
    // собственный фон (`styled_div` -> `background::layer(&e.style)`) читают
    // стиль копии, поэтому без переноса `image-orientation: none`, заданный
    // на предке, до картинки не доезжает и EXIF-разворот применяется всё
    // равно (`image-orientation-none`, `-none-content-images`).
    // Ранний возврат снят намеренно: он экономил только клон, а перенос
    // обязан идти и у элемента со своим шрифтом.
    if copy.style.image_orient_none.is_none() {
        copy.style.image_orient_none = inherited.image_orient_none;
    }
    // `color` наследуется (CSS 2.1 §14.1), а цвет рамки по умолчанию —
    // `currentColor` (css-backgrounds-3 §4.1): без переноса рамка картинки
    // в белом абзаце рисовалась чёрной (`c44-ln-box-001/002/003`).
    if copy.style.color.is_none() {
        copy.style.color = inherited.color;
    }
    copy
}

pub(crate) fn atom_base_font(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    }
}

/// Пределы замещаемого с соотношением сторон — таблица CSS 2.1 §10.4
/// (`csswg-drafts/css2/Overview.bs:8985-9050`): при нарушении пределов по
/// ОДНОЙ оси вторая выводится через соотношение и зажимается своими
/// пределами, при нарушении по обеим в разные стороны (`w < min-width`,
/// `h > max-height`) соотношение сдаётся — обе стороны берут пределы.
/// Прежний общий множитель «сперва потолки, затем полы» держал соотношение
/// всегда и расходился с таблицей ровно в этих строках
/// (`box-sizing-replaced-001..003`). `max` берётся как max(min, max).
pub(crate) fn css2_replaced_limits(
    w: f32,
    h: f32,
    min_w: Option<f32>,
    max_w: Option<f32>,
    min_h: Option<f32>,
    max_h: Option<f32>,
) -> (f32, f32) {
    let min_w = min_w.unwrap_or(0.0);
    let min_h = min_h.unwrap_or(0.0);
    let max_w = max_w.unwrap_or(f32::INFINITY).max(min_w);
    let max_h = max_h.unwrap_or(f32::INFINITY).max(min_h);
    let (over_w, under_w) = (w > max_w, w < min_w);
    let (over_h, under_h) = (h > max_h, h < min_h);
    match (over_w, under_w, over_h, under_h) {
        (true, _, true, _) if max_w / w <= max_h / h => (max_w, min_h.max(max_w * h / w)),
        (true, _, true, _) => (min_w.max(max_h * w / h), max_h),
        (_, true, _, true) if min_w / w <= min_h / h => (max_w.min(min_h * w / h), min_h),
        (_, true, _, true) => (min_w, max_h.min(min_w * h / w)),
        (_, true, true, _) => (min_w, max_h),
        (true, _, _, true) => (max_w, min_h),
        (true, _, _, _) => (max_w, (max_w * h / w).max(min_h)),
        (_, true, _, _) => (min_w, (min_w * h / w).min(max_h)),
        (_, _, true, _) => ((max_h * w / h).max(min_w), max_h),
        (_, _, _, true) => ((min_h * w / h).min(max_w), min_h),
        _ => (w, h),
    }
}

/// Ключевое слово содержимого в ПРЕДЕЛЕ холста — в точки.
///
/// min-content и max-content замещаемого — его природный размер, а при
/// определённой второй оси и соотношении — размер, перенесённый через
/// соотношение (css-sizing-3 §5.1 «min-content … of a replaced element»;
/// css-sizing-4 §5.1 transferred size; Blink `ComputeReplacedSize`).
/// Раскладке такой предел не выразить (`apply` ставит вместо него долю
/// 100%), и `width: 1000px; height: 100px; max-width: min-content` у холста
/// 10×10 давал ширину во всю строку, а не 100
/// (`replaced-max-width-min-content`, `replaced-min-width-min-content`).
pub(crate) fn canvas_limit_keywords(e: &Element) -> Element {
    let kw = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    let c = &e.style;
    if !(kw(c.min_width) || kw(c.max_width) || kw(c.min_height) || kw(c.max_height)) {
        return e.clone();
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let ratio = c.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0);
    let nat_w = match (px(c.height), ratio) {
        (Some(h), Some(r)) => Some(h * r),
        _ => px(c.attr_width),
    };
    let nat_h = match (px(c.width), ratio) {
        (Some(w), Some(r)) => Some(w / r),
        _ => px(c.attr_height),
    };
    let mut copy = e.clone();
    let s = &mut copy.style;
    for (lim, nat) in [
        (&mut s.min_width, nat_w),
        (&mut s.max_width, nat_w),
        (&mut s.min_height, nat_h),
        (&mut s.max_height, nat_h),
    ] {
        if kw(*lim) {
            *lim = nat.map(Len::Px);
        }
    }
    copy
}
