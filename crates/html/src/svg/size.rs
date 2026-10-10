//! Размер рисунка: атрибуты, `viewBox`, замещаемый размер по умолчанию, вписывание в пределы.

use crate::dom::Element;

/// Размер рисунка в логических пикселях: из атрибутов, иначе из `viewBox`.
pub fn size_of(e: &Element) -> (f32, f32) {
    // Обособление размера: рисунок меряется как пустой, величину задаёт
    // `contain-intrinsic-size` (css-contain-2 §size containment) — ни
    // атрибуты, ни `viewBox` не смотрим.
    // Атрибутные `width`/`height` — природный размер замещаемого, и зум его
    // домножает («It also multiplies the natural size of all replaced
    // elements», css-viewport-1 §zoom; `zoom/svg-stroke-width`). CSS-размеры
    // ниже уже домножены проходом `zoom::resolve`.
    let z = e.style.zoom_eff.unwrap_or(1.0);
    let num = |name: &str| -> Option<f32> {
        e.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .map(|v| v * z)
    };
    // Стилевые размеры СТАРШЕ атрибутов (CSS поверх разметки).
    let css = |l: Option<crate::style::values::value::Len>| match l {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let given_w = css(e.style.width).or_else(|| num("width"));
    let given_h = css(e.style.height).or_else(|| num("height"));
    // Обособление размера снимает у рисунка ПРИРОДНЫЕ стороны и природное
    // соотношение (`viewBox`), но не написанные автором: «All CSS properties
    // of the size containment box are taken into account as they would be
    // when performing layout normally» (css-contain-2 Overview.bs:681-683).
    // Ось без названного размера берёт `contain-intrinsic-*`, иначе ноль.
    // Раньше выбрасывались ОБЕ стороны, и `<svg width="100" viewBox="0 0 50 50">`
    // под `contain: size` выходил нулевой ширины.
    if e.style.contains_width() || e.style.contains_height() {
        let axis = |contained: bool, given: Option<f32>, ci: Option<f32>| {
            if contained {
                given.or(ci).unwrap_or(0.0)
            } else {
                given.unwrap_or(0.0)
            }
        };
        return (
            axis(
                e.style.contains_width(),
                given_w,
                e.style.contain_intrinsic.0,
            ),
            axis(
                e.style.contains_height(),
                given_h,
                e.style.contain_intrinsic.1,
            ),
        );
    }
    if let (Some(w), Some(h)) = (given_w, given_h) {
        return (w, h);
    }
    // Заявленное `aspect-ratio` СИЛЬНЕЕ соотношения из `viewBox`
    // (css-sizing-4 §4: «the preferred aspect ratio … overrides any natural
    // aspect ratio»): рисунок переформатируется, как и картинка.
    if let Some(r) = e.style.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0) {
        match (given_w, given_h) {
            (Some(w), None) => return (w, w / r),
            (None, Some(h)) => return (h * r, h),
            _ => {}
        }
    }
    // `viewBox` задаёт СООТНОШЕНИЕ сторон, а не собственный размер: заданная
    // сторона тянет за собой вторую (CSS Images 3 §5 default sizing).
    let ratio = view_box_ratio(e);
    match (given_w, given_h, ratio) {
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        // Ни одной стороны: рисунок сам себе размера не даёт. По CSS 2.1
        // §10.3.2 замещаемый без собственных сторон занимает 300x150, а с
        // соотношением — наибольший такой прямоугольник, что в 300x150
        // влезает. Прежде отдавали viewBox как СОБСТВЕННЫЙ размер и 120x120
        // без него — рисунок выходил своего масштаба, а не блочного.
        (None, None, Some((vw, vh))) => {
            let k = (DEFAULT_REPLACED.0 / vw).min(DEFAULT_REPLACED.1 / vh);
            (vw * k, vh * k)
        }
        (Some(w), None, None) => (w, DEFAULT_REPLACED.1),
        (None, Some(h), None) => (DEFAULT_REPLACED.0, h),
        _ => DEFAULT_REPLACED,
    }
}

/// Размер замещаемого элемента, у которого нет собственного (CSS 2.1
/// §10.3.2: «the used value of 'width' becomes 300px … 'height' becomes
/// 150px»).
const DEFAULT_REPLACED: (f32, f32) = (300.0, 150.0);

/// Соотношение сторон из `viewBox`: (ширина, высота) окна просмотра.
pub(super) fn view_box_ratio(e: &Element) -> Option<(f32, f32)> {
    e.attr("viewBox").and_then(|vb| {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (p.len() == 4 && p[2] > 0.0 && p[3] > 0.0).then(|| (p[2], p[3]))
    })
}

/// Рисунок БЕЗ собственного размера, но С соотношением (`<svg viewBox>` без
/// `width`/`height`) занимает stretch-fit ширину содержащего блока, а высоту
/// берёт из соотношения. CSS 2.2 §10.3.2, последний пункт: «if the containing
/// block's width does not itself depend on the replaced element's width, then
/// the used value of 'width' is calculated from the constraint equation used
/// for block-level, non-replaced elements in normal flow»; css-sizing-3 §5.1
/// «stretch-fit size: the size a box would take if its outer size filled the
/// available space»; Blink `ComputeReplacedSizeInternal` (length_utils.cc):
/// без natural size главная сторона — `Length::Stretch()` / `StretchFit()`
/// от available-size, когда тот definite. Резерв 300×150 остаётся только
/// когда ширину взять неоткуда (`cb_width == None`).
///
/// Пределы держат соотношение (CSS 2 §10.4): сперва `max-width`, затем
/// `max-height`; нарушенный предел переносится во вторую ось. До правки
/// `size_of` вписывал рисунок в 300×150 и пределов не читал:
/// `svg-root-as-flex-item-003` рисовал 150×150 вместо 100×100,
/// `flex-aspect-ratio-img-row-015` не видел `max-height: 100px`.
///
/// Возвращает копию с обеими сторонами в точках — их и возьмёт `size_of`.
pub fn stretch_fit(e: &Element, cb_width: Option<f32>) -> Element {
    use crate::style::values::value::Len;
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let attr_num = |name: &str| -> bool {
        e.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .is_some()
    };
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    // Обособление размера и любая заданная сторона (стилем или атрибутом)
    // — прежний путь `size_of`, здесь ничего не меняется.
    if e.style.contains_width()
        || e.style.contains_height()
        || !auto(e.style.width)
        || !auto(e.style.height)
        || attr_num("width")
        || attr_num("height")
    {
        return e.clone();
    }
    let Some((vw, vh)) = view_box_ratio(e) else {
        return e.clone();
    };
    // Заявленное `aspect-ratio` старше соотношения из `viewBox`
    // (css-sizing-4 §4) — та же иерархия, что в `size_of`.
    let ratio = e
        .style
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(vw / vh);
    let side = |l: Option<Len>| px(l).unwrap_or(0.0);
    let b = e.style.borders();
    let pb_x =
        side(e.style.padding.left) + side(e.style.padding.right) + side(b.left) + side(b.right);
    let pb_y =
        side(e.style.padding.top) + side(e.style.padding.bottom) + side(b.top) + side(b.bottom);
    // Внешний размер = содержимое + паддинг + рамка + поля (css-sizing-3
    // §5.1 «outer size»); `size_of` отдаёт размер СОДЕРЖИМОГО.
    let outer_x = side(e.style.margin.left) + side(e.style.margin.right) + pb_x;
    let (mut w, mut h) = match cb_width {
        Some(cb) if cb > 0.0 => {
            let w = (cb - outer_x).max(0.0);
            (w, w / ratio)
        }
        _ => {
            let k = (DEFAULT_REPLACED.0 / vw).min(DEFAULT_REPLACED.1 / vh);
            (vw * k, vh * k)
        }
    };
    // При `box-sizing: border-box` предел включает паддинг и рамку —
    // содержимому остаётся остальное (как `sub_w`/`sub_h` в `image_with`).
    let (sub_x, sub_y) = if e.style.border_box == Some(true) {
        (pb_x, pb_y)
    } else {
        (0.0, 0.0)
    };
    if let Some(m) = px(e.style.max_width).map(|m| (m - sub_x).max(0.0))
        && w > m
    {
        w = m;
        h = w / ratio;
    }
    if let Some(m) = px(e.style.max_height).map(|m| (m - sub_y).max(0.0))
        && h > m
    {
        h = m;
        w = h * ratio;
    }
    let mut copy = e.clone();
    copy.style.width = Some(Len::Px(w));
    copy.style.height = Some(Len::Px(h));
    copy
}
