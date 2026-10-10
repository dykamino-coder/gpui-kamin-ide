//! Картинка с авто-размерами: естественный размер, пределы и соотношение.

use crate::dom::Element;
use crate::layout::block::containing::CB_WIDTH;
use crate::layout::replaced::limits::css2_replaced_limits;
use crate::style::values::value::Len;
use gpui::{Styled, StyledImage, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn auto_sized_image(
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
) -> gpui::Img {
    let both_auto = matches!(e.style.width, None | Some(Len::Auto))
        && matches!(e.style.height, None | Some(Len::Auto));
    if both_auto
        && let Some(ready) = crate::paint::background::source(&crate::paint::background::key(
            local.unwrap_or(src),
            &e.style,
        ))
    {
        // Авто-размер замещаемого считается ЗДЕСЬ, а не отдаётся
        // загрузчику картинок: тот работает асинхронно, и в первом
        // кадре коробка выходила нулевой высоты (box-sizing-007 —
        // страница вовсе без квадратов). Недостающая сторона
        // достраивается соотношением, резерв — 300×150
        // (CSS 2.1 §10.3.2/§10.6.2, css-images-3 §default-sizing).
        let side = ready.intrinsic();
        let (w0, h0) = match (side.w, side.h, side.ratio) {
            (Some(w), Some(h), _) => (w, h),
            (Some(w), None, Some(r)) => (w, w / r),
            (None, Some(h), Some(r)) => (h * r, h),
            (Some(w), None, None) => (w, 150.0),
            (None, Some(h), None) => (300.0, h),
            (None, None, Some(r)) => {
                // §10.3.2, последний пункт: при обеих `auto` и одном
                // лишь соотношении ширина берётся из содержащего
                // блока, а не из резерва. Резерв остаётся, когда
                // ширину взять неоткуда (`ratio-2.svg` в
                // `visudet/replaced-elements-*`: мы рисовали 300×150,
                // эталон — 200×100 по `div { width: 200px }`).
                let w = CB_WIDTH
                    .get()
                    .filter(|v| *v > 0.0)
                    .unwrap_or_else(|| (150.0 * r).min(300.0));
                (w, w / r)
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: считать ДОЛЮ собственного
            // размера рисунка (`<svg width="100%">` и SVG вовсе без
            // атрибутов — у него подразумевается `100%`) и разрешать
            // её от содержащего блока. Написано было и в `Intrinsic`
            // (поля `w_pct`/`h_pct` в `background::svg_size`), и
            // здесь. Срез из 943 пар (replaced/normal-flow/svg/
            // background-size/image): 880 → 874. `replaced-intrinsic-
            // 002` пошла 6.10 → 1.41, но вся семья `replaced-
            // elements-*` рухнула (0.01 → 24.17 и родня).
            // Причина в РАЗНИЦЕ ДВУХ СЛУЧАЕВ: у `<img>` доля значит
            // «своего размера нет» и работает умолчальный размер
            // 300×150 (css-images-3 §5.2), а у `<object>` SVG — это
            // вложенный ДОКУМЕНТ, и его `100%` считается от коробки
            // объекта, то есть от содержащего блока. Возвращать
            // вместе с этим различением.
            (None, None, None) => (300.0, 150.0),
        };
        if w0 > 0.0 && h0 > 0.0 {
            // §10.4: пределы — по таблице (`css2_replaced_limits`).
            let min_w = clamp(e.style.min_width, sub_w);
            let min_h = clamp(e.style.min_height, sub_h);
            // Без СОБСТВЕННОГО соотношения стороны независимы: потолок
            // высоты режет только высоту, и ширина остаётся своей
            // (§10.4, таблица «no intrinsic ratio»). Прежде общий
            // множитель ужимал и её — картинка без соотношения под
            // `max-height: 20px` выходила у́же в пятнадцать раз
            // (`replaced-elements-max-height-20`, снимки `no-ratio` и
            // `height-25-no-ratio`).
            let без_соотношения = side.ratio.is_none() && !(side.w.is_some() && side.h.is_some());
            let (rw, rh) = if без_соотношения {
                (limit(w0, min_w, max_w), limit(h0, min_h, max_h))
            } else {
                css2_replaced_limits(w0, h0, min_w, max_w, min_h, max_h)
            };
            image = vectorize(image, rw, rh)
                .w(px(rw))
                .h(px(rh))
                .object_fit(gpui::ObjectFit::Fill);
        }
    } else if (max_w.is_some() || max_h.is_some())
        && let Some(ready) = crate::paint::background::source(&crate::paint::background::key(
            local.unwrap_or(src),
            &e.style,
        ))
    {
        let side = ready.intrinsic();
        if let (Some(w0), Some(h0)) = (side.w, side.h)
            && w0 > 0.0
            && h0 > 0.0
        {
            let mut scale = 1.0f32;
            if let Some(m) = max_w {
                scale = scale.min(m / w0);
            }
            if let Some(m) = max_h {
                scale = scale.min(m / h0);
            }
            if scale < 1.0 {
                image = vectorize(image, w0 * scale, h0 * scale)
                    .w(px(w0 * scale))
                    .h(px(h0 * scale))
                    .object_fit(gpui::ObjectFit::Fill);
            }
        }
    }
    image
}
