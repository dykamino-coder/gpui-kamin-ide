//! Коробка элемента: базовый стиль div, обрезка и покраска фона за вычетом области.

mod clipping;
use crate::style::computed::BoxStyle;
use clipping::shows_inside;

mod clamp;
use crate::dom::Element;
use crate::dom::whitespace::is_blank;
use crate::layout::block::margin_height;
use crate::layout::positioned::absolute_overflow;
use crate::layout::writing_mode::native_intrinsic;
use crate::paint::apply::{apply_for_decorations, apply_hover};
use crate::paint::decorations::decorations;
use gpui::{ParentElement, Styled, div};

/// Базовый стиль элемента плюс слой наведения и дорисовка того, чего в
/// `gpui::Style` нет: обводки, размытия подложки, разноцветных сторон рамки.
/// `cx` — контекст раскладки места, где коробка строится (`LayoutCx`).
pub(crate) fn styled_div(e: &Element, cx: &crate::layout::cx::LayoutCx) -> gpui::Div {
    styled_div_with(e, &BoxStyle::shared(e.style.used()), cx)
}

pub(crate) fn styled_div_with(
    e: &Element,
    style: &BoxStyle,
    cx: &crate::layout::cx::LayoutCx,
) -> gpui::Div {
    let vcx = cx.vcx();
    // Скрытая коробка с видимым потомком не прячется целиком: раскладка та
    // же, гаснет только СВОЯ краска — иначе ранний возврат из отрисовки
    // уносит и потомка (`visufx/visibility-005`).
    let bare;
    // Скрытая коробка с `filter: url()`: исходная картинка прозрачна, но
    // примитивы без источника (`feFlood`) свой выход дают — фильтр остаётся
    // (`visibility-hidden-element-with-filter-001`: зелёная заливка видна).
    let c = if style.hidden == Some(true)
        && (shows_inside(&e.children) || style.filter_ref.is_some())
    {
        bare = style.restyled(style.paint_off());
        &bare
    } else {
        style
    };
    // Коробка под слоем `filter: url(#id)` (`paint::effects::filter::FilterLayer`, только
    // пустая — `dom::filter_ref_only_empty`) несёт свой фон ВНУТРИ SVG:
    // результат фильтра ЗАМЕНЯЕТ картинку элемента, вне области фильтра —
    // прозрачно (filter-effects-1 §filter region). Фон самой коробки под
    // слоем проступал там, куда фильтр краски не кладёт
    // (`empty-element-with-filter`, `filter-region-calc-001`).
    // Под преобразованием (своим или предка) фон остаётся: растр слоя
    // считается по масштабу окна, а не преобразования, и под `scale(10)`
    // его край расплывается на десяток точек — чёткий фон коробки под ним
    // держит край (`filter-scale-001`, `filter-scaling-001`).
    let unfilled;
    let paint = if c.background_paint.paints()
        && c.background.is_some()
        && !c.transform_ancestor
        && c.transform.is_none()
        && c.filter_ref
            .as_deref()
            .is_some_and(|id| vcx.doc.masks.markup(&format!("filter:{id}")).is_some())
        && !e.children.iter().any(|n| !is_blank(n))
    {
        unfilled = {
            let mut unfilled = c.clone();
            unfilled.background = None;
            unfilled
        };
        &unfilled
    } else {
        c
    };
    let auto_height = margin_height::used_style(e, paint);
    let mut d = apply_for_decorations(div(), auto_height.as_ref().unwrap_or(paint), vcx);
    if native_intrinsic::eligible(e) {
        d.style().sizing_keywords = Some(crate::paint::apply::intrinsic_size::keywords(c));
    }
    d = crate::interactive::scroll_target::attach(d, e, c, cx);
    if let Some(probe) = crate::interactive::scroll_extent::probe(
        cx.scroll.frame.as_ref(),
        e.node_id,
        c,
        cx.scroll.sticky.as_ref(),
    ) {
        d = d.child(probe);
    }
    // Проба якоря (css-anchor-position-1 §anchor-name) и содержащего блока
    // (§position-area): канвас во всю коробку пишет её рамку в реестр кадра
    // на подготовке — позже по дереву её прочтёт `anchor::AnchorPlace`
    // позиционированной коробки. Ставится здесь, потому что через
    // `styled_div_with` проходят и блоки, и атомы строки (`inline-block` из
    // `anchor-position-005`), и держатели. `visibility: hidden` якоря —
    // из `style`: `c` может быть `paint_off()` без этого флага
    // (§position-visibility: anchor-visible, «anchor box is invisible»).
    if let Some(probe) =
        crate::layout::positioned::anchor::probe_for(e, c, style.hidden == Some(true), vcx)
    {
        d = d.child(probe);
    }
    if let Some(probe) = absolute_overflow::probe(c, &cx.position.state, vcx, cx.contracts.build) {
        d = d.child(probe);
    }
    // `pointer-events: none` — элемент не реагирует на курсор, значит и слой
    // наведения к нему не применяется.
    if c.pointer_events_none != Some(true)
        && let Some(h) = &e.hover
    {
        d = apply_hover(d, h.cascaded(), c);
    }
    let mut d = clamp::apply(d, e, c, cx);
    let empty = !e.children.iter().any(|n| !is_blank(n));
    // Корень документа — сам корень подложки (filter-effects-2
    // §BackdropRoot): под ним фильтровать нечего, а холст с фоном корня лежит
    // СОСЕДОМ коробки и попал бы в копию кадра (`backdrop-filter-root-element`).
    let rootless;
    let c = if e.tag == "html" && c.backdrop_color.is_some() {
        rootless = {
            let mut rootless = c.clone();
            rootless.backdrop_color = None;
            rootless
        };
        &rootless
    } else {
        c
    };
    for extra in decorations(c, empty, vcx, Some(cx)) {
        d = d.child(extra);
    }
    super::container_units::observe(d, e, style)
}
