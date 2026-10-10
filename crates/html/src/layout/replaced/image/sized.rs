//! Размеры картинки по заданным ширине/высоте и соотношению сторон.

use super::auto_sized_image;
use crate::dom::Element;
use crate::style::values::value::Len;
use gpui::{Styled, StyledImage, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn sized_image(
    src: &str,
    e: &Element,
    local: Option<&str>,
    mut image: gpui::Img,
    vectorize: impl Fn(gpui::Img, f32, f32) -> gpui::Img,
    sub_w: f32,
    sub_h: f32,
    clamp: impl Fn(Option<Len>, f32) -> Option<f32>,
    max_w: Option<f32>,
    max_h: Option<f32>,
    limit: impl Fn(f32, Option<f32>, Option<f32>) -> f32,
    узкая: &mut Option<(f32, f32)>,
    ratio_of: impl Fn() -> Option<f32>,
    transfer: impl Fn(f32, f32, bool) -> f32,
) -> gpui::Img {
    if let (Some(Len::Px(w)), Some(Len::Px(h))) = (e.style.width, e.style.height) {
        image = vectorize(image, (w - sub_w).max(1.0), (h - sub_h).max(1.0));
        // Элемент ГИБКОГО контейнера: коробку задаёт раскладка (рост,
        // сжатие — css-flexbox-1 §9.7), и картинка заполняет её
        // (`object-fit: fill`, css-images-3 §5.5), а не держит
        // объявленную ширину. Прежде коробка `flex: 5` росла до 122.5,
        // а картинка оставалась 10 точек (`flexbox-basic-img-horiz-001`,
        // `-vert-001`).
        image = if e.style.flex_item {
            image.size_full()
        } else {
            image.w(px(w)).h(px(h))
        };
    } else if let (Some(Len::Px(w)), None | Some(Len::Auto)) = (e.style.width, e.style.height) {
        // Заданная ширина + auto-высота: высота из соотношения (§10.6.2),
        // без соотношения — своя, резерв 150.
        // §10.7: пределы зажимают и НАЗВАННУЮ сторону тоже. Коробку
        // `apply` уже зажал, а картинка шла как написана и вылезала за неё.
        let cw = limit((w - sub_w).max(0.0), clamp(e.style.min_width, sub_w), max_w);
        let ch = match ratio_of() {
            Some(r) if r > 0.0 => transfer(cw, r, true),
            _ => crate::paint::background::source(&crate::paint::background::key(
                local.unwrap_or(src),
                &e.style,
            ))
            .and_then(|s| s.intrinsic().h)
            .unwrap_or(150.0),
        };
        // §10.4: пределы держат ВЫВЕДЕННУЮ сторону тоже — заданная
        // остаётся как написана, а высота из соотношения обязана влезть
        // в свой потолок и пол. Замерено отдельно: 0 и 0 — правка по
        // спеке, счёт на ней не держится.
        let ch_free = ch;
        let ch = limit(ch, clamp(e.style.min_height, sub_h), max_h);
        // §10.4: когда потолок или пол ИЗМЕНИЛИ выведенную сторону,
        // заданная пересчитывается по соотношению — коробка остаётся
        // пропорциональной, а не растягивается. `width: 200px` при
        // `max-height: 50px` у картинки 100×50 — это 100×50, а не
        // 200×50.
        let cw = match ratio_of() {
            Some(r) if r > 0.0 && (ch - ch_free).abs() > 0.01 => {
                let w = limit(
                    transfer(ch, r, false),
                    clamp(e.style.min_width, sub_w),
                    max_w,
                );
                // Коробка ужимается ТОЛЬКО когда предел и правда изменил
                // выведенную сторону: иначе высота у неё остаётся `auto`,
                // и подстановка ломала замещённые без пределов вовсе
                // (`replaced-intrinsic-004`).
                *узкая = Some((w, ch));
                w
            }
            _ => cw,
        };
        image = vectorize(image, cw.max(1.0), ch.max(1.0))
            .w(px(cw))
            .h(px(ch))
            .object_fit(gpui::ObjectFit::Fill);
    } else if let (None | Some(Len::Auto), Some(Len::Px(h))) = (e.style.width, e.style.height) {
        // Зеркально: заданная высота + auto-ширина (§10.3.2).
        let ch = limit(
            (h - sub_h).max(0.0),
            clamp(e.style.min_height, sub_h),
            max_h,
        );
        let cw = match ratio_of() {
            Some(r) if r > 0.0 => transfer(ch, r, false),
            _ => crate::paint::background::source(&crate::paint::background::key(
                local.unwrap_or(src),
                &e.style,
            ))
            .and_then(|s| s.intrinsic().w)
            .unwrap_or(300.0),
        };
        let cw_free = cw;
        let cw = limit(cw, clamp(e.style.min_width, sub_w), max_w);
        // Зеркально §10.4: изменённая ширина тянет за собой высоту.
        let ch = match ratio_of() {
            Some(r) if r > 0.0 && (cw - cw_free).abs() > 0.01 => {
                let h = limit(
                    transfer(cw, r, true),
                    clamp(e.style.min_height, sub_h),
                    max_h,
                );
                *узкая = Some((cw, h));
                h
            }
            _ => ch,
        };
        image = vectorize(image, cw.max(1.0), ch.max(1.0))
            .w(px(cw))
            .h(px(ch))
            .object_fit(gpui::ObjectFit::Fill);
    } else if let (Some(Len::Pct(_)), None | Some(Len::Auto)) = (e.style.width, e.style.height) {
        // Доля ширины при auto-высоте: ширину даёт содержащий блок, высоту
        // — собственное соотношение сторон (§10.3.2, §10.6.2). Раньше доля
        // не попадала ни в одну ветку, и замещаемый рисовался СВОИМ
        // пикселем (`absolute-replaced-width-006`: 15×15 вместо 96×96).
        // Долю по этой оси УЖЕ поставил хозяин (`styled_div(e)` несёт
        // стиль элемента целиком), поэтому картинке остаётся заполнить
        // его: `relative(kw)` внутри давал долю ОТ ДОЛИ — `width: 50%`
        // выходило четвертью содержащего блока.
        image = image.w(gpui::relative(1.0));
        if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
            image.style().aspect_ratio = Some(r);
        }
    } else if let (None | Some(Len::Auto), Some(Len::Pct(_))) = (e.style.width, e.style.height) {
        // Зеркально: доля высоты при auto-ширине.
        image = image.h(gpui::relative(1.0));
        if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
            image.style().aspect_ratio = Some(r);
        }
    } else if !matches!(e.style.width, None | Some(Len::Auto))
        && !matches!(e.style.height, None | Some(Len::Auto))
    {
        // Обе стороны заданы, но не обе в точках: каждая ось — своим
        // значением. `width:100%; height:100%` растягивается на коробку
        // (sizing-percentages-replaced-orthogonal-001), а смешанная
        // запись `width:50%; height:15px` раньше падала в size_full и
        // ТЕРЯЛА пиксельную сторону (inline-replaced-width-011..015).
        image = match (e.style.width, e.style.height) {
            (Some(Len::Pct(_)), Some(Len::Px(h))) => image.w(gpui::relative(1.0)).h(px(h)),
            (Some(Len::Px(w)), Some(Len::Pct(_))) => image.w(px(w)).h(gpui::relative(1.0)),
            _ => image.size_full(),
        };
    } else if !matches!(e.style.width, Some(Len::Px(_)))
        && !matches!(e.style.height, Some(Len::Px(_)))
    {
        image = auto_sized_image(
            src, e, local, image, vectorize, sub_w, sub_h, clamp, max_w, max_h, limit,
        );
    }
    image
}
