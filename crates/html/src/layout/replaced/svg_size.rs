//! Размер рисунка: атрибуты, `viewBox`, замещаемый размер по умолчанию, вписывание в пределы.

use crate::defaults::DEFAULT_REPLACED_SIZE_PX;
use crate::dom::Element;
use crate::dom::resource_facts::SvgFacts;
use crate::style::values::value::px_strict;

/// Размер рисунка в логических пикселях: из атрибутов, иначе из `viewBox`.
pub fn size_of(e: &Element) -> (f32, f32) {
    // Обособление размера: рисунок меряется как пустой, величину задаёт
    // `contain-intrinsic-size` (css-contain-2 §size containment) — ни
    // атрибуты, ни `viewBox` не смотрим.
    // Атрибутные `width`/`height` — природный размер замещаемого, и зум его
    // домножает («It also multiplies the natural size of all replaced
    // elements», css-viewport-1 §zoom; `zoom/svg-stroke-width`). CSS-размеры
    // ниже уже домножены проходом `zoom::resolve`.
    let z = e.style.computed().zoom_eff.unwrap_or(1.0);
    let facts = SvgFacts::of(e);
    // Стилевые размеры СТАРШЕ атрибутов (CSS поверх разметки).
    let css = |l: Option<crate::style::values::value::Len>| match l {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let given_w = css(e.style.computed().width).or_else(|| facts.width.map(|v| v * z));
    let given_h = css(e.style.computed().height).or_else(|| facts.height.map(|v| v * z));
    // Обособление размера снимает у рисунка ПРИРОДНЫЕ стороны и природное
    // соотношение (`viewBox`), но не написанные автором: «All CSS properties
    // of the size containment box are taken into account as they would be
    // when performing layout normally» (css-contain-2 Overview.bs:681-683).
    // Ось без названного размера берёт `contain-intrinsic-*`, иначе ноль.
    // Раньше выбрасывались ОБЕ стороны, и `<svg width="100" viewBox="0 0 50 50">`
    // под `contain: size` выходил нулевой ширины.
    if e.style.computed().contains_width() || e.style.computed().contains_height() {
        let axis = |contained: bool, given: Option<f32>, ci: Option<f32>| {
            if contained {
                given.or(ci).unwrap_or(0.0)
            } else {
                given.unwrap_or(0.0)
            }
        };
        return (
            axis(
                e.style.computed().contains_width(),
                given_w,
                e.style.computed().contain_intrinsic.0,
            ),
            axis(
                e.style.computed().contains_height(),
                given_h,
                e.style.computed().contain_intrinsic.1,
            ),
        );
    }
    if let (Some(w), Some(h)) = (given_w, given_h) {
        // Both sides given: min/max clamp each axis on its own (CSS 2.1
        // §10.4, §10.7); intrinsic keywords there are already lengths
        // (`dom::element_style::svg_size`, `svg-no-ar-*-min-content`).
        let clamp = |v: f32, lo, hi| {
            v.min(css(hi).unwrap_or(f32::INFINITY))
                .max(css(lo).unwrap_or(0.0))
        };
        return (
            clamp(
                w,
                e.style.computed().min_width,
                e.style.computed().max_width,
            ),
            clamp(
                h,
                e.style.computed().min_height,
                e.style.computed().max_height,
            ),
        );
    }
    // Заявленное `aspect-ratio` СИЛЬНЕЕ соотношения из `viewBox`
    // (css-sizing-4 §4: «the preferred aspect ratio … overrides any natural
    // aspect ratio»): рисунок переформатируется, как и картинка.
    // `auto <ratio>` (css-sizing-4 §5.1) берёт запасное соотношение, только
    // когда природного нет: ни `viewBox`, ни пары атрибутов
    // (`aspect-ratio/replaced-element-016`).
    let fallback = e
        .style
        .computed()
        .aspect_ratio_auto
        .filter(|_| facts.view_box.is_none());
    if let Some(r) = e
        .style
        .computed()
        .aspect_ratio
        .or(fallback)
        .filter(|r| r.is_finite() && *r > 0.0)
    {
        match (given_w, given_h) {
            (Some(w), None) => return (w, w / r),
            (None, Some(h)) => return (h * r, h),
            _ => {}
        }
    }
    // `viewBox` задаёт СООТНОШЕНИЕ сторон, а не собственный размер: заданная
    // сторона тянет за собой вторую (CSS Images 3 §5 default sizing).
    let ratio = facts.view_box;
    match (given_w, given_h, ratio) {
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        // Ни одной стороны: рисунок сам себе размера не даёт. По CSS 2.1
        // §10.3.2 замещаемый без собственных сторон занимает 300x150, а с
        // соотношением — наибольший такой прямоугольник, что в 300x150
        // влезает. Прежде отдавали viewBox как СОБСТВЕННЫЙ размер и 120x120
        // без него — рисунок выходил своего масштаба, а не блочного.
        (None, None, Some((vw, vh))) => {
            let k = (DEFAULT_REPLACED_SIZE_PX.0 / vw).min(DEFAULT_REPLACED_SIZE_PX.1 / vh);
            (vw * k, vh * k)
        }
        (Some(w), None, None) => (w, DEFAULT_REPLACED_SIZE_PX.1),
        (None, Some(h), None) => (DEFAULT_REPLACED_SIZE_PX.0, h),
        _ => DEFAULT_REPLACED_SIZE_PX,
    }
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
    let px = px_strict;
    let facts = SvgFacts::of(e);
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    // Обособление размера и любая заданная сторона (стилем или атрибутом)
    // — прежний путь `size_of`, здесь ничего не меняется.
    if e.style.computed().contains_width()
        || e.style.computed().contains_height()
        || !auto(e.style.computed().width)
        || !auto(e.style.computed().height)
        || facts.width.is_some()
        || facts.height.is_some()
    {
        return e.clone();
    }
    let Some((vw, vh)) = facts.view_box else {
        return e.clone();
    };
    // Заявленное `aspect-ratio` старше соотношения из `viewBox`
    // (css-sizing-4 §4) — та же иерархия, что в `size_of`.
    let ratio = e
        .style
        .computed()
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(vw / vh);
    let side = |l: Option<Len>| px(l).unwrap_or(0.0);
    let b = e.style.computed().borders();
    let pb_x = side(e.style.computed().padding.left)
        + side(e.style.computed().padding.right)
        + side(b.left)
        + side(b.right);
    let pb_y = side(e.style.computed().padding.top)
        + side(e.style.computed().padding.bottom)
        + side(b.top)
        + side(b.bottom);
    // Внешний размер = содержимое + паддинг + рамка + поля (css-sizing-3
    // §5.1 «outer size»); `size_of` отдаёт размер СОДЕРЖИМОГО.
    let outer_x =
        side(e.style.computed().margin.left) + side(e.style.computed().margin.right) + pb_x;
    let (mut w, mut h) = match cb_width {
        Some(cb) if cb > 0.0 => {
            let w = (cb - outer_x).max(0.0);
            (w, w / ratio)
        }
        _ => {
            let k = (DEFAULT_REPLACED_SIZE_PX.0 / vw).min(DEFAULT_REPLACED_SIZE_PX.1 / vh);
            (vw * k, vh * k)
        }
    };
    // При `box-sizing: border-box` предел включает паддинг и рамку —
    // содержимому остаётся остальное (как `sub_w`/`sub_h` в `image_with`).
    let (sub_x, sub_y) = if e.style.computed().border_box == Some(true) {
        (pb_x, pb_y)
    } else {
        (0.0, 0.0)
    };
    if let Some(m) = px(e.style.computed().max_width).map(|m| (m - sub_x).max(0.0))
        && w > m
    {
        w = m;
        h = w / ratio;
    }
    if let Some(m) = px(e.style.computed().max_height).map(|m| (m - sub_y).max(0.0))
        && h > m
    {
        h = m;
        w = h * ratio;
    }
    let mut copy = e.clone();
    copy.style.used_mut().width = Some(Len::Px(w));
    copy.style.used_mut().height = Some(Len::Px(h));
    copy
}
