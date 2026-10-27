//! Размер рисунка: атрибуты, `viewBox`, замещаемый размер по умолчанию, вписывание в пределы.

use crate::defaults::DEFAULT_REPLACED_SIZE_PX;
use crate::dom::Element;
use crate::dom::resource_facts::SvgFacts;
use crate::style::values::value::px_strict;

pub use crate::resources::svg_document::size::size_of;

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
    if e.style.used().contains_width()
        || e.style.used().contains_height()
        || !auto(e.style.used().width)
        || !auto(e.style.used().height)
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
        .used()
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(vw / vh);
    let side = |l: Option<Len>| px(l).unwrap_or(0.0);
    let b = e.style.used().borders();
    let pb_x = side(e.style.used().padding.left)
        + side(e.style.used().padding.right)
        + side(b.left)
        + side(b.right);
    let pb_y = side(e.style.used().padding.top)
        + side(e.style.used().padding.bottom)
        + side(b.top)
        + side(b.bottom);
    // Внешний размер = содержимое + паддинг + рамка + поля (css-sizing-3
    // §5.1 «outer size»); `size_of` отдаёт размер СОДЕРЖИМОГО.
    let outer_x = side(e.style.used().margin.left) + side(e.style.used().margin.right) + pb_x;
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
    let (sub_x, sub_y) = if e.style.used().border_box == Some(true) {
        (pb_x, pb_y)
    } else {
        (0.0, 0.0)
    };
    if let Some(m) = px(e.style.used().max_width).map(|m| (m - sub_x).max(0.0))
        && w > m
    {
        w = m;
        h = w / ratio;
    }
    if let Some(m) = px(e.style.used().max_height).map(|m| (m - sub_y).max(0.0))
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
