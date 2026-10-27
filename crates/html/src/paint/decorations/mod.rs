//! Украшения коробки: тени, рамки по форме, слои градиентов.

use crate::document::state::ValueCx;
use crate::style::computed::BoxStyle;
mod borders;
pub(super) use borders::box_borders;
pub(super) use borders::coloured_borders;

use crate::hooks::{IntoHooked, PaintOnly};
use crate::paint::decorations::backdrop::backdrop_matrix;
use crate::paint::decorations::border_shape::border_shape_layer;
use crate::paint::decorations::gradient_stripes::gradient_stripes;
use crate::paint::decorations::shadows::{inset_shadows, outer_shadows};
use gpui::AnyElement;

mod backdrop;
mod border_shape;
mod gradient_stripes;
mod outline;
pub(crate) mod outline_phase;
pub(crate) mod rect_minus;
mod shadows;
pub(crate) mod shaped_outset;
pub(crate) mod shaped_ring;
pub(crate) mod text_shadows;

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
pub(crate) fn decorations(
    c: &BoxStyle,
    empty: bool,
    vcx: ValueCx<'_>,
    cx: Option<&crate::layout::cx::LayoutCx>,
) -> Vec<AnyElement> {
    if crate::paint::effects::corner_clip::insets(c, vcx).is_some() {
        return Vec::new();
    }
    let mut out: Vec<AnyElement> = vec![];

    outer_shadows(c, &mut out, vcx);
    if let Some(layer) = crate::paint::background::single_corner::layer(c) {
        out.push(layer);
    }

    // `filter: url(#id)` на HTML-элементе (filter-effects-1 §filter
    // region): дешёвый путь для коробки без содержимого — SVG с `<rect>`
    // цвета фона и этим фильтром растрируется resvg и ложится слоем поверх
    // коробки; область — по умолчанию −10 %/120 % от border-box, поэтому
    // холст вдвое шире, а слой сдвинут на половину коробки. Фильтр над
    // готовым буфером группы — отдельная задача.
    // Только у коробки БЕЗ содержимого: над содержимым слой лёг бы поверх
    // детей (filter-region-transformed-composited-child-001).
    if empty
        && let Some(id) = c.filter_ref.as_deref()
        && let Some(def) = vcx.doc.masks.markup(&format!("filter:{id}"))
    {
        let fill = match c.background.filter(|_| c.background_paint.paints()) {
            Some(col) if col.a > 0.0 => format!(
                "rgba({},{},{},{})",
                (col.r * 255.0).round(),
                (col.g * 255.0).round(),
                (col.b * 255.0).round(),
                col.a
            ),
            _ => "none".to_string(),
        };
        out.push(
            PaintOnly(crate::paint::effects::filter::FilterLayer {
                def,
                id: id.to_string(),
                fill,
                values: vcx.keep(),
            })
            .hooked(),
        );
    }
    // Filter Effects 2 §backdrop-filter (Overview.bs:61-66): the filtered
    // backdrop is drawn first and the element's background, borders and
    // content on top of it; borders after it stay unfiltered
    // (`backdrop-filter-with-background-clip-padding-box`).
    backdrop_matrix(c, &mut out, vcx);
    if !(empty
        && !c.transform_ancestor
        && c.transform.is_none()
        && c.filter_ref
            .as_deref()
            .is_some_and(|id| vcx.doc.masks.markup(&format!("filter:{id}")).is_some()))
        && let Some(layer) = crate::paint::background::color_layer(c, vcx)
    {
        out.push(layer);
    }

    // Фоновая картинка идёт первой: она поверх цвета фона и под всем
    // остальным — тот же порядок, что в браузере.
    // Несколько слоёв (css-backgrounds-3 §2.1): плитки каждого слоя своей
    // механикой, снизу вверх — первый в списке рисуется последним, поверх.
    if c.background_paint.paints() {
        if let Some(layers) = c.bg_layers(vcx) {
            for l in layers.iter().rev() {
                if let Some(layer) = crate::paint::background::layer(l, vcx) {
                    out.push(layer);
                }
            }
        } else if let Some(layer) = crate::paint::background::layer(c, vcx) {
            out.push(layer);
        } else if c.gradient_as_tile() {
            // Градиент с размером/повтором/позицией — той же механикой плитки:
            // источник понимает записи `linear-gradient(...)`.
            let mut tiled = c.clone();
            tiled.bg_image = tiled.gradient_raw.clone();
            if let Some(layer) = crate::paint::background::layer(&tiled, vcx) {
                out.push(layer);
            }
        }
    }

    inset_shadows(c, &mut out, vcx);

    // Рамка ПОВЕРХ слоя картинки (css-backgrounds-3 §3.7, прим.: «The
    // background is always drawn behind the border»; CSS 2.2 Прил. E; Blink
    // `PaintFillLayers` → `PaintBorder`). Квад рисует рамку ДО детей, а слой
    // плиток — ребёнок, и полупрозрачная/пунктирная рамка оказывалась ПОД
    // картинкой (`origin-border-box` 6.15, `css3-background-origin-*` 0.83).
    // Квад цвета не получает (`apply::apply_paint`); слой повторяет толщины,
    // стиль и скругление рамки и вынесен на толщину сторон — абсолютный
    // ребёнок отсчитывается от padding-box (как полосы сторон ниже).
    let start = out.len();
    box_borders(c, &mut out, vcx);

    border_shape_layer(c, &mut out, vcx);

    // Рамка-картинка рисуется ПОВЕРХ фона и заменяет обычную рамку.
    if let Some(layer) = crate::paint::border_image::layer(c, vcx) {
        out.push(layer);
    }
    stationary(c, &mut out, start, cx);

    if c.background_paint.paints() {
        gradient_stripes(c, &mut out);
    }

    out.extend(outline::decorations(c, vcx));

    // Разные цвета сторон рамки: у GPUI цвет рамки один на элемент, поэтому
    // несовпадающие стороны дорисовываются полосами поверх.
    let start = out.len();
    coloured_borders(c, &mut out, vcx);
    stationary(c, &mut out, start, cx);
    out
}

fn stationary(
    c: &BoxStyle,
    out: &mut Vec<AnyElement>,
    start: usize,
    cx: Option<&crate::layout::cx::LayoutCx>,
) {
    let layers = out.split_off(start);
    out.extend(layers.into_iter().map(|el| {
        crate::interactive::scroll_target::stationary(
            cx.and_then(|cx| cx.scroll.frame.as_ref()),
            cx.and_then(|cx| cx.scroll.target.as_deref()),
            c.self_node,
            el,
        )
    }));
}
