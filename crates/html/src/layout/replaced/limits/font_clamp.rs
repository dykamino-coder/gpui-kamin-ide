//! Шрифт и ключевые слова размеров замещаемых элементов: наследуемый шрифт атома, пределы CSS2 и canvas.

use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

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
    // `image-rendering` тоже наследуется (css-images-3 §image-rendering).
    if copy.style.image_pixelated.is_none() {
        copy.style.image_pixelated = inherited.image_pixelated;
    }
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
