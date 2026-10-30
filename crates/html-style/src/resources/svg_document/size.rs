//! Размер SVG-документа по атрибутам, viewBox и вычисленным CSS-ограничениям.

use crate::defaults::DEFAULT_REPLACED_SIZE_PX;
use crate::dom::Element;
use crate::dom::resource_facts::SvgFacts;

/// Размер рисунка в логических пикселях: из атрибутов, иначе из `viewBox`.
pub fn size_of(e: &Element) -> (f32, f32) {
    // Обособление размера: рисунок меряется как пустой, величину задаёт
    // `contain-intrinsic-size` (css-contain-2 §size containment) — ни
    // атрибуты, ни `viewBox` не смотрим.
    // Атрибутные `width`/`height` — природный размер замещаемого, и зум его
    // домножает («It also multiplies the natural size of all replaced
    // elements», css-viewport-1 §zoom; `zoom/svg-stroke-width`). CSS-размеры
    // ниже уже домножены проходом `zoom::resolve`.
    let z = e.style.used().zoom_eff.unwrap_or(1.0);
    let facts = SvgFacts::of(e);
    // Стилевые размеры СТАРШЕ атрибутов (CSS поверх разметки).
    let css = |l: Option<crate::style::values::value::Len>| match l {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => None,
    };
    let given_w = css(e.style.used().width).or_else(|| facts.width.map(|v| v * z));
    let given_h = css(e.style.used().height).or_else(|| facts.height.map(|v| v * z));
    // Обособление размера снимает у рисунка ПРИРОДНЫЕ стороны и природное
    // соотношение (`viewBox`), но не написанные автором: «All CSS properties
    // of the size containment box are taken into account as they would be
    // when performing layout normally» (css-contain-2 Overview.bs:681-683).
    // Ось без названного размера берёт `contain-intrinsic-*`, иначе ноль.
    // Раньше выбрасывались ОБЕ стороны, и `<svg width="100" viewBox="0 0 50 50">`
    // под `contain: size` выходил нулевой ширины.
    if e.style.used().contains_width() || e.style.used().contains_height() {
        let axis = |contained: bool, given: Option<f32>, ci: Option<f32>| {
            if contained {
                given.or(ci).unwrap_or(0.0)
            } else {
                given.unwrap_or(0.0)
            }
        };
        return (
            axis(
                e.style.used().contains_width(),
                given_w,
                e.style.used().contain_intrinsic.0,
            ),
            axis(
                e.style.used().contains_height(),
                given_h,
                e.style.used().contain_intrinsic.1,
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
            clamp(w, e.style.used().min_width, e.style.used().max_width),
            clamp(h, e.style.used().min_height, e.style.used().max_height),
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
        .used()
        .aspect_ratio_auto
        .filter(|_| facts.view_box.is_none());
    if let Some(r) = e
        .style
        .used()
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
