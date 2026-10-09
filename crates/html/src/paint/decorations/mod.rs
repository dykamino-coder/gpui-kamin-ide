//! Украшения коробки: тени, рамки по форме, слои градиентов.
// owner: A

use crate::paint::decorations::backdrop::backdrop_matrix;
use crate::paint::decorations::border_shape::border_shape_layer;
use crate::paint::decorations::gradient_stripes::gradient_stripes;
use crate::paint::decorations::shadows::{inset_shadows, outer_shadows};
use crate::paint::effects::mask::mask_def;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Styled, div, px};

mod backdrop;
mod border_shape;
mod gradient_stripes;
mod outline;
mod shadows;
pub(crate) mod text_shadows;

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
pub(crate) fn decorations(c: &Computed, empty: bool) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];

    outer_shadows(c, &mut out);

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
        && let Some(def) = mask_def(&format!("filter:{id}"))
    {
        let fill = match c.background {
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
            crate::paint::effects::filter::FilterLayer {
                def,
                id: id.to_string(),
                fill,
            }
            .into_any_element(),
        );
    }
    // Фоновая картинка идёт первой: она поверх цвета фона и под всем
    // остальным — тот же порядок, что в браузере.
    // Несколько слоёв (css-backgrounds-3 §2.1): плитки каждого слоя своей
    // механикой, снизу вверх — первый в списке рисуется последним, поверх.
    if let Some(layers) = c.bg_layers() {
        for l in layers.iter().rev() {
            if let Some(layer) = crate::paint::background::layer(l) {
                out.push(layer);
            }
        }
    } else if let Some(layer) = crate::paint::background::layer(c) {
        out.push(layer);
    } else if c.gradient_as_tile() {
        // Градиент с размером/повтором/позицией — той же механикой плитки:
        // источник понимает записи `linear-gradient(...)`.
        let mut tiled = c.clone();
        tiled.bg_image = tiled.gradient_raw.clone();
        if let Some(layer) = crate::paint::background::layer(&tiled) {
            out.push(layer);
        }
    }

    inset_shadows(c, &mut out);

    // Рамка ПОВЕРХ слоя картинки (css-backgrounds-3 §3.7, прим.: «The
    // background is always drawn behind the border»; CSS 2.2 Прил. E; Blink
    // `PaintFillLayers` → `PaintBorder`). Квад рисует рамку ДО детей, а слой
    // плиток — ребёнок, и полупрозрачная/пунктирная рамка оказывалась ПОД
    // картинкой (`origin-border-box` 6.15, `css3-background-origin-*` 0.83).
    // Квад цвета не получает (`apply::apply_paint`); слой повторяет толщины,
    // стиль и скругление рамки и вынесен на толщину сторон — абсолютный
    // ребёнок отсчитывается от padding-box (как полосы сторон ниже).
    if let Some((colour, [t, r, b, l])) = crate::style::apply::border_layer(c) {
        let mut layer = div()
            .absolute()
            .top(px(-t))
            .left(px(-l))
            .right(px(-r))
            .bottom(px(-b))
            .border_t(px(t))
            .border_r(px(r))
            .border_b(px(b))
            .border_l(px(l))
            .border_color(colour.to_hsla());
        // The layer IS the box's CSS border: it takes the same device-pixel
        // edge snapping as the quad border (`apply::apply_paint`; CSS 2.1
        // §8.5.3, Blink box_border_painter.cc snaps outer and inner rects).
        layer.style().css_border_snap = Some(true);
        if c.border_dashed == Some(true) {
            layer = layer.border_dashed();
        }
        if c.border_dotted == Some(true) {
            layer.style().border_style = Some(gpui::BorderStyle::Dotted);
        }
        // Скругление — как у квада (`apply::apply_radius`): при маске группы
        // или `border-shape` квад углов не получает, и слой тоже.
        if !c.radius_masked() && c.border_shape.is_none() {
            let rad = |l: Option<Len>| crate::style::apply::radius_px(c, l).unwrap_or(0.0);
            layer = layer
                .rounded_tl(px(rad(c.radius.tl)))
                .rounded_tr(px(rad(c.radius.tr)))
                .rounded_br(px(rad(c.radius.br)))
                .rounded_bl(px(rad(c.radius.bl)));
        }
        out.push(layer.into_any_element());
    }

    // Рамка `double` (css-backgrounds-3 §4.2) — двумя кольцами: линии по
    // трети толщины, зазор — остаток (Blink
    // `BorderEdge::GetDoubleBorderStripeWidths`: 50 → 17/16/17, 5 → 2/1/2).
    // Квад и слой рамки её не красят (`apply::double_border`).
    if let Some((colour, [t, r, b, l])) = crate::style::apply::double_border(c) {
        let third = |w: f32| (w / 3.0).round();
        let (o_t, o_r, o_b, o_l) = (third(t), third(r), third(b), third(l));
        let mut outer = div()
            .absolute()
            .top(px(-t))
            .left(px(-l))
            .right(px(-r))
            .bottom(px(-b))
            .border_t(px(o_t))
            .border_r(px(o_r))
            .border_b(px(o_b))
            .border_l(px(o_l))
            .border_color(colour.to_hsla());
        // Внутренняя линия прилегает к padding-box снаружи.
        let mut inner = div()
            .absolute()
            .top(px(-o_t))
            .left(px(-o_l))
            .right(px(-o_r))
            .bottom(px(-o_b))
            .border_t(px(o_t))
            .border_r(px(o_r))
            .border_b(px(o_b))
            .border_l(px(o_l))
            .border_color(colour.to_hsla());
        if !c.radius_masked() {
            let rad = |x: Option<Len>| crate::style::apply::radius_px(c, x).unwrap_or(0.0);
            let shrink = |x: Option<Len>, a: f32, b: f32| (rad(x) - a.max(b)).max(0.0);
            outer = outer
                .rounded_tl(px(rad(c.radius.tl)))
                .rounded_tr(px(rad(c.radius.tr)))
                .rounded_br(px(rad(c.radius.br)))
                .rounded_bl(px(rad(c.radius.bl)));
            inner = inner
                .rounded_tl(px(shrink(c.radius.tl, t - o_t, l - o_l)))
                .rounded_tr(px(shrink(c.radius.tr, t - o_t, r - o_r)))
                .rounded_br(px(shrink(c.radius.br, b - o_b, r - o_r)))
                .rounded_bl(px(shrink(c.radius.bl, b - o_b, l - o_l)));
        }
        out.push(outer.into_any_element());
        out.push(inner.into_any_element());
    }

    // Рамка при фигурных углах (`corner-shape`, css-borders-4): внешний край
    // — контур, внутренний — он же, сжатый на толщину сторон. Квад её не
    // красит (`apply::apply_paint`); слой — цветной растр кольца в
    // border-box, тем же растеризатором, что и маска группы, — контуры
    // совпадают попиксельно. Разные цвета сторон остаются полосами ниже.
    if c.corner_shaped() {
        let sides: Vec<_> = c.border_colors.iter().flatten().collect();
        let uniform = sides
            .first()
            .filter(|f| sides.iter().all(|s| s == *f))
            .map(|f| **f);
        let mixed = sides.len() > 1 && uniform.is_none();
        let side_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let bw = c.borders();
        let widths = [
            side_px(bw.top),
            side_px(bw.right),
            side_px(bw.bottom),
            side_px(bw.left),
        ];
        if !mixed && widths.iter().any(|w| *w > 0.0) {
            // Без цвета рамка красится цветом текста, без него — чёрным
            // (начальное `border-color: currentColor`).
            let colour = uniform.or(c.border_color).or(c.color).unwrap_or(
                crate::style::values::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            );
            let colour = crate::paint::background::border_paint(c, colour);
            let spec = crate::paint::background::rrect_spec(c, Some(widths));
            let [t, r, b, l] = widths;
            out.push(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let sf = window.scale_factor();
                        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        let (pw, ph) = (
                            (w * sf).round().max(1.0) as u32,
                            (h * sf).round().max(1.0) as u32,
                        );
                        let args = spec.trim_start_matches("rrect(").trim_end_matches(')');
                        if let Some(img) =
                            crate::paint::background::rasterize_ring(args, pw, ph, sf, colour)
                        {
                            let _ = window.paint_image_with_sampling(
                                bounds,
                                gpui::Corners::default(),
                                img,
                                0,
                                false,
                                gpui::ImageSampling::Linear,
                            );
                        }
                    },
                )
                // Абсолютный ребёнок считается от padding-box — кольцо
                // накрывает рамку отрицательными отступами (как полосы ниже).
                .absolute()
                .top(px(-t))
                .left(px(-l))
                .right(px(-r))
                .bottom(px(-b))
                .into_any_element(),
            );
        }
    }

    border_shape_layer(c, &mut out);

    // Рамка-картинка рисуется ПОВЕРХ фона и заменяет обычную рамку.
    if let Some(layer) = crate::paint::border_image::layer(c) {
        out.push(layer);
    }

    backdrop_matrix(c, &mut out);

    gradient_stripes(c, &mut out);

    out.extend(outline::decorations(c));

    // Разные цвета сторон рамки: у GPUI цвет рамки один на элемент, поэтому
    // несовпадающие стороны дорисовываются полосами поверх.
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (30.09): не класть полосу, когда все ЗАДАННЫЕ
    // стороны одного цвета (квад и так красит их, `apply::apply_paint`
    // `!mixed`), — ради удвоенной альфы односторонней полупрозрачной рамки
    // (`grid-gap-decorations-067…082`). Свод v210: +7/−31 — 24 пары
    // `border-image-*` (полосы легли поверх картинки; с прежним правилом для
    // картинки срез щелей/фонов/line-clamp 1329 пар дал +7/−7). Сами 067…082
    // почти не сдвинулись (075/076 0.51 → 0.48), зато непрозрачные
    // односторонние `border-bottom` эталонов `019/048-051` ушли 0.11 → 0.61,
    // пунктирные `016` 0.35 → 1.01: квад рисует одностороннюю рамку не
    // точка в точку как полоса. Выигрыш `007`, `fragmentation-020/021/
    // 025/026` — от того же снятия полосы; разбирать по снимкам обеих сторон.
    let uniform = sides.len() == 4 && sides.iter().all(|s| *s == sides[0]);
    // При `border-shape` рамка — один слой цветом relevant side (спека:
    // stroke-from-border), прямоугольные полосы сторон ей не нужны.
    if !sides.is_empty() && !uniform && c.border_shape.is_none() {
        let side_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let bw = c.borders();
        let (t, r, b, l) = (
            side_px(bw.top),
            side_px(bw.right),
            side_px(bw.bottom),
            side_px(bw.left),
        );
        // Сторона без своего цвета красится общим `border-color`, а без него —
        // цветом текста (`currentColor`): квад при полосах цвета не получает
        // (`apply::apply_paint`, `strips`), и такая сторона иначе пропала бы.
        let fallback = c
            .border_color
            .or(c.color)
            .unwrap_or(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        for (i, colour) in c.border_colors.iter().enumerate() {
            let colour = colour.as_ref().unwrap_or(&fallback);
            let w = [t, r, b, l][i];
            if w <= 0.0 {
                continue;
            }
            // Абсолютный ребёнок считается от ВНУТРЕННЕГО края рамки,
            // поэтому кольцо накрывается отрицательными отступами ровно на
            // толщину сторон — иначе полоса ложится внутрь содержимого.
            let strip = div().absolute().bg(colour.to_hsla());
            out.push(
                match i {
                    0 => strip.top(px(-t)).left(px(-l)).right(px(-r)).h(px(w)),
                    1 => strip.top(px(-t)).right(px(-r)).bottom(px(-b)).w(px(w)),
                    2 => strip.bottom(px(-b)).left(px(-l)).right(px(-r)).h(px(w)),
                    _ => strip.top(px(-t)).left(px(-l)).bottom(px(-b)).w(px(w)),
                }
                .into_any_element(),
            );
        }
    }
    out
}
