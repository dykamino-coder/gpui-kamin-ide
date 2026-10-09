//! Украшения коробки: тени, рамки по форме, слои градиентов.
// owner: A

use crate::render::*;

pub mod border_shape;
pub mod gradient_layer;
pub mod shadows;

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
pub(crate) mod outline;
pub(crate) mod text_shadows;
pub(crate) fn decorations(c: &Computed, empty: bool) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];

    // РЕЗКАЯ тень (без размытия): примитив тени с нулевым размытием
    // вырождается в шейдере, поэтому она рисуется слоем-квадом, раздутым на
    // разлёт. Радиус фигуры — по спеке (css-backgrounds-3 §7.1): нулевой
    // остаётся острым, иначе растёт на разлёт; доля считается от размера
    // раздутой фигуры (известные ширина и высота).
    // css-backgrounds-3 §7.1: «The first shadow is on top» — paint bottom-up.
    for sh in c.shadows.iter().rev() {
        // Тень без цвета помечена отрицательной альфой и берёт `color`
        // (css-backgrounds-3 §7.1) — как в `apply::shadow_colour`.
        // У `border-shape` тени повторяют фигуру — растром под группой
        // (`grouped`, `Grouped::under`), квад здесь лёг бы прямоугольником.
        if sh.blur > 0.0 || sh.color.a == 0.0 || c.border_shape.is_some() {
            continue;
        }
        let colour = if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        };
        let spread = sh.spread;
        let radius = match c.radius.tl {
            // §7.1: при радиусе МЕНЬШЕ разлёта разлёт сперва домножается на
            // 1 + (r − 1)³, r = радиус / разлёт — острый угол переходит в
            // круглый без скачка (`box-shadow-radius-000`: 5 px при разлёте
            // 20 дают 16.56, а не 25). Отрицательный разлёт убавляет с полом 0.
            Some(Len::Px(v)) if v > 0.0 && spread > v => {
                let k = v / spread;
                v + spread * (1.0 + (k - 1.0).powi(3))
            }
            Some(Len::Px(v)) if v > 0.0 => (v + spread).max(0.0),
            Some(Len::Pct(k)) => match (c.width, c.height) {
                (Some(Len::Px(w)), Some(Len::Px(h))) => {
                    k * (w + 2.0 * spread).min(h + 2.0 * spread)
                }
                _ => 0.0,
            },
            _ => 0.0,
        };
        // Слой живёт СРЕДИ детей и рисовался бы поверх фона коробки, а тень
        // обязана быть ПОД ней — поэтому центр слоя прозрачный: краску несёт
        // РАМКА толщиной в разлёт (со сдвигом по смещению тени). Точная
        // фигура «раздутое минус коробка» этим покрыта при |смещении| не
        // больше разлёта — обычный случай резкой тени.
        let widths = [
            (spread - sh.y).max(0.0),
            (spread + sh.x).max(0.0),
            (spread + sh.y).max(0.0),
            (spread - sh.x).max(0.0),
        ];
        // Абсолютный слой отсчитывается от padding-box (как полосы сторон и
        // слой рамки ниже), а фигура тени — от border-box (§7.1: «as if the
        // border-box of the element were opaque»): без выноса на толщину рамки
        // кольцо ложилось ВНУТРЬ неё (`box-shadow-039`: оранжевое на чёрном).
        let bw = c.borders();
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let (bt, br, bb, bl) = (side(bw.top), side(bw.right), side(bw.bottom), side(bw.left));
        let square = [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl]
            .iter()
            .all(|r| matches!(r, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0))));
        if square && c.radius_ell.is_none() && c.corner_shape.is_none() {
            // A square sharp shadow is «the shadow rect minus the border box»
            // (css-backgrounds-3 §7.1) — up to four plain rectangles. They are
            // painted as FILLS, the same primitive as a box background, so a
            // translucent shadow blends exactly like the equivalent colored
            // box (`box-shadow-039`: the border-ring primitive gave 128 where
            // a 50% fill gives 127), and any offset larger than the spread is
            // covered too.
            let (dx, dy) = (sh.x, sh.y);
            let fill = colour.to_hsla();
            out.push(
                crate::background::exact_layer::ExactLayer::inset(
                    [-bt, -br, -bb, -bl],
                    move |r: gpui::Bounds<gpui::Pixels>, window: &mut gpui::Window| {
                        // Blink offsets the UNSNAPPED border box and snaps
                        // the result (a 72.5-device-pixel top with a -62.5
                        // offset lands on row 10, not 11).
                        let r = crate::background::exact_layer::positioning(r);
                        // `r` is the box's unrounded border box; the shadow
                        // rect is it moved by the offset and grown by the
                        // spread (§7.1). Edges snap to the device grid.
                        let (bl, bt) = (f32::from(r.left()), f32::from(r.top()));
                        let (br, bb) = (f32::from(r.right()), f32::from(r.bottom()));
                        let (rl, rt) = (bl + dx - spread, bt + dy - spread);
                        let (rr, rb) = (br + dx + spread, bb + dy + spread);
                        paint_rect_minus(window, (rl, rt, rr, rb), (bl, bt, br, bb), fill);
                    },
                )
                .into_any_element(),
            );
            continue;
        }
        let mut ring = div()
            .absolute()
            .top(px(sh.y - spread - bt))
            .left(px(sh.x - spread - bl))
            .right(px(-sh.x - spread - br))
            .bottom(px(-sh.y - spread - bb))
            .rounded(px(radius))
            .border_t(px(widths[0]))
            .border_r(px(widths[1]))
            .border_b(px(widths[2]))
            .border_l(px(widths[3]))
            .border_color(colour.to_hsla());
        // The ring's edges are the shadow's edges: Blink paints a box shadow
        // from the pixel-snapped border geometry, so they snap to device
        // pixels like the box's own border edges (`apply::apply_paint`;
        // `box-shadow-outset-without-border-radius-001` draws its reference
        // with borders).
        ring.style().css_border_snap = Some(true);
        out.push(ring.into_any_element());
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
            crate::interact::FilterLayer {
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
            if let Some(layer) = crate::background::layer(l) {
                out.push(layer);
            }
        }
    } else if let Some(layer) = crate::background::layer(c) {
        out.push(layer);
    } else if c.gradient_as_tile() {
        // Градиент с размером/повтором/позицией — той же механикой плитки:
        // источник понимает записи `linear-gradient(...)`.
        let mut tiled = c.clone();
        tiled.bg_image = tiled.gradient_raw.clone();
        if let Some(layer) = crate::background::layer(&tiled) {
            out.push(layer);
        }
    }

    // РЕЗКАЯ ВНУТРЕННЯЯ тень (без размытия). §7.1: «An inner box-shadow casts
    // a shadow as if everything outside the padding edge were opaque» — тень =
    // padding-box МИНУС фигура, сжатая на разлёт и сдвинутая на смещение.
    // Слой ровно в padding-box (абсолютный ребёнок от него и отсчитывается),
    // краску несёт рамка толщиной «разлёт ± смещение»: её внутренний край и
    // есть фигура. Слой — после плиток фона и до рамки: «inner shadows …
    // immediately above the background … (below the borders and border image)».
    for sh in c.inset_shadows.iter().rev() {
        // У `border-shape` внутренняя тень — растр по внутреннему контуру
        // (слой фигуры ниже), кольцо здесь было бы прямоугольным.
        if sh.blur > 0.0 || sh.color.a == 0.0 || c.border_shape.is_some() {
            continue;
        }
        let colour = if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        };
        let s = sh.spread;
        let widths = [
            (s + sh.y).max(0.0),
            (s - sh.x).max(0.0),
            (s - sh.y).max(0.0),
            (s + sh.x).max(0.0),
        ];
        if widths.iter().all(|v| *v <= 0.0) {
            continue;
        }
        let square = [c.radius.tl, c.radius.tr, c.radius.br, c.radius.bl]
            .iter()
            .all(|r| matches!(r, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0))));
        if square && c.radius_ell.is_none() && c.corner_shape.is_none() {
            // Square inner shadow = padding box minus the padding box moved by
            // the offset and shrunk by the spread (css-backgrounds-3 §7.1),
            // from the unrounded padding box, painted as fills like the outer
            // one (`box-shadow-041`: the ring's inner edge came from the
            // snapped box and missed a device column).
            let (dx, dy) = (sh.x, sh.y);
            let fill = colour.to_hsla();
            out.push(
                crate::background::exact_layer::ExactLayer::inset(
                    [0.0; 4],
                    move |r: gpui::Bounds<gpui::Pixels>, window: &mut gpui::Window| {
                        let r = crate::background::exact_layer::positioning(r);
                        let (pl, pt) = (f32::from(r.left()), f32::from(r.top()));
                        let (pr, pb) = (f32::from(r.right()), f32::from(r.bottom()));
                        let hole = (
                            (pl + dx + s).max(pl),
                            (pt + dy + s).max(pt),
                            (pr + dx - s).min(pr),
                            (pb + dy - s).min(pb),
                        );
                        paint_rect_minus(window, (pl, pt, pr, pb), hole, fill);
                    },
                )
                .into_any_element(),
            );
            continue;
        }
        let mut ring = div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .border_t(px(widths[0]))
            .border_r(px(widths[1]))
            .border_b(px(widths[2]))
            .border_l(px(widths[3]))
            .border_color(colour.to_hsla());
        // Snapped like the box's border edges (see the outer shadow ring).
        ring.style().css_border_snap = Some(true);
        // Внешний край кольца — padding-box: скругление «радиус − рамка»
        // (§5.4, как у `clip_layer`).
        if !c.radius_masked() && c.border_shape.is_none() {
            let bw = c.borders();
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let pad = |r: Option<Len>, a: Option<Len>, b: Option<Len>| {
                (crate::apply::radius_px(c, r).unwrap_or(0.0) - side(a).max(side(b))).max(0.0)
            };
            ring = ring
                .rounded_tl(px(pad(c.radius.tl, bw.top, bw.left)))
                .rounded_tr(px(pad(c.radius.tr, bw.top, bw.right)))
                .rounded_br(px(pad(c.radius.br, bw.bottom, bw.right)))
                .rounded_bl(px(pad(c.radius.bl, bw.bottom, bw.left)));
        }
        out.push(ring.into_any_element());
    }

    // Рамка ПОВЕРХ слоя картинки (css-backgrounds-3 §3.7, прим.: «The
    // background is always drawn behind the border»; CSS 2.2 Прил. E; Blink
    // `PaintFillLayers` → `PaintBorder`). Квад рисует рамку ДО детей, а слой
    // плиток — ребёнок, и полупрозрачная/пунктирная рамка оказывалась ПОД
    // картинкой (`origin-border-box` 6.15, `css3-background-origin-*` 0.83).
    // Квад цвета не получает (`apply::apply_paint`); слой повторяет толщины,
    // стиль и скругление рамки и вынесен на толщину сторон — абсолютный
    // ребёнок отсчитывается от padding-box (как полосы сторон ниже).
    if let Some((colour, [t, r, b, l])) = crate::apply::border_layer(c) {
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
            let rad = |l: Option<Len>| crate::apply::radius_px(c, l).unwrap_or(0.0);
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
    if let Some((colour, [t, r, b, l])) = crate::apply::double_border(c) {
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
            let rad = |x: Option<Len>| crate::apply::radius_px(c, x).unwrap_or(0.0);
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
            let colour = uniform
                .or(c.border_color)
                .or(c.color)
                .unwrap_or(crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                });
            let colour = crate::background::border_paint(c, colour);
            let spec = crate::background::rrect_spec(c, Some(widths));
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
                            crate::background::rasterize_ring(args, pw, ph, sf, colour)
                        {
                            let _ = window.paint_image_with_sampling(bounds, gpui::Corners::default(), img, 0, false, gpui::ImageSampling::Linear);
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

    // Рамка `border-shape` (css-borders-4 §border-shape): одна фигура — SVG-
    // обводка толщиной «relevant side» по центру контура, две — заливка между
    // внешней и внутренней. Квад цвета не получает (`apply::apply_paint`),
    // полосы разных сторон не рисуются (ниже). Слой — растр `svg::rasterize`
    // на border-box плюс вынос (`Computed::border_shape_ext`) — тот же
    // контур, что у маски группы (`background::border_shape_path`).
    if let Some(bs) = c.border_shape.clone() {
        let (stroke, colour) = c.border_shape_stroke();
        let colour = crate::background::border_paint(c, colour);
        let outer_out = c.geometry_outsets(bs.outer_box);
        let inner = bs
            .inner
            .clone()
            .map(|(s, k)| (s, c.geometry_outsets(k)));
        let ext = c.border_shape_ext();
        let side_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let w = c.borders();
        let [t, r, b, l] = [side_px(w.top), side_px(w.right), side_px(w.bottom), side_px(w.left)];
        // Внутренняя тень по внутреннему контуру фигуры (css-borders-4
        // §border-shape-shadow-interaction: «cast as if everything outside
        // the shape defined by the inner path were opaque»; Blink
        // `PaintInsetBoxShadowForBorderShape`) — растром на той же области,
        // что кольцо, ПОД кольцом и над фоном (css-backgrounds-3 §box-shadow:
        // «inner shadows … immediately above the background»).
        let inset = c.resolved_shadows(true);
        if inset.iter().any(|(_, k)| k.a > 0.0) {
            let (bs, outer_out, inner) = (bs.clone(), outer_out, inner.clone());
            out.push(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let (cw, ch) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        let (bw, bh) = (cw - ext[3] - ext[1], ch - ext[0] - ext[2]);
                        let markup = crate::background::border_shape_shadow_svg(
                            (bs.outer.as_str(), outer_out),
                            inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                            stroke,
                            &inset,
                            true,
                            bw,
                            bh,
                            ext[3],
                            ext[0],
                            cw,
                            ch,
                        );
                        if let Some(markup) = markup
                            && let Some(img) = crate::svg::rasterize(&markup, cw, ch)
                        {
                            let _ = window.paint_image_with_sampling(bounds, gpui::Corners::default(), img, 0, false, gpui::ImageSampling::Linear);
                        }
                    },
                )
                .absolute()
                .top(px(-(t + ext[0])))
                .left(px(-(l + ext[3])))
                .right(px(-(r + ext[1])))
                .bottom(px(-(b + ext[2])))
                .into_any_element(),
            );
        }
        // При обрезке переполнения кольцо уходит НАД буфер группы
        // (`grouped` → `Grouped::over`): здесь оно легло бы под детей.
        if (inner.is_some() || stroke > 0.0) && colour.a > 0.0 && !c.border_shape_clips() {
            out.push(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let (cw, ch) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        let (bw, bh) = (cw - ext[3] - ext[1], ch - ext[0] - ext[2]);
                        let markup = crate::background::border_shape_ring_svg(
                            (bs.outer.as_str(), outer_out),
                            inner.as_ref().map(|(s, o)| (s.as_str(), *o)),
                            stroke,
                            colour,
                            bw,
                            bh,
                            ext[3],
                            ext[0],
                            cw,
                            ch,
                        );
                        if let Some(markup) = markup
                            && let Some(img) = crate::svg::rasterize(&markup, cw, ch)
                        {
                            let _ = window.paint_image_with_sampling(bounds, gpui::Corners::default(), img, 0, false, gpui::ImageSampling::Linear);
                        }
                    },
                )
                // Абсолютный ребёнок считается от padding-box — слой накрывает
                // рамку и вынос отрицательными отступами.
                .absolute()
                .top(px(-(t + ext[0])))
                .left(px(-(l + ext[3])))
                .right(px(-(r + ext[1])))
                .bottom(px(-(b + ext[2])))
                .into_any_element(),
            );
        }
    }

    // Рамка-картинка рисуется ПОВЕРХ фона и заменяет обычную рамку.
    if let Some(layer) = crate::border_image::layer(c) {
        out.push(layer);
    }

    // `backdrop-filter` с цветовыми функциями (filter-effects-2
    // §BackdropFilterProperty: `<filter-value-list>` как у `filter`) — матрица
    // 4×5 тем же проходом подложки. Область — border-box со скруглением
    // (§3 шаг 3: «Apply a clip to the contents of T', using the border box
    // shape of B, with border-radius»): абсолютный слой отсчитывается от
    // padding-box, рамка выносится отрицательными краями. Под корнем
    // подложки у предка (§BackdropRoot) матрица не рисуется: наш кадр — не
    // «Backdrop Root Image» такого корня, копия внесла бы в подложку то, что
    // лежит под корнем (`backdrop-filter-backdrop-root-*`).
    // `backdrop-filter: url(#id)` (filter-effects-2 §BackdropFilterProperty:
    // `<filter-value-list>` допускает `<url>`): SVG `<filter>` из одного
    // примитива с аффинной формулой — та же матрица (`svg_filter_matrix`;
    // `backdrop-filter-svg`).
    let matrix = c
        .backdrop_color
        .and_then(|f| f.color_matrix())
        .or_else(|| {
            c.backdrop_ref
                .as_deref()
                .and_then(|id| mask_def(&format!("filter:{id}")))
                .and_then(|def| svg_filter_matrix(&def))
        })
        .filter(|_| !c.backdrop_root_above);
    if let Some(m) = matrix {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let radius = c.backdrop_blur.unwrap_or(0.0);
        // Своя `mask-image` делает элемент группой (`grouped`), а буфер группы
        // рисуется до кадра: подложку поднимает в кадр `paint_backdrop_filter`
        // (filter-effects-2 Overview.bs:61-66, шаги 1-5;
        // `backdrop-filter-good-and-bad-mask-image`). Форма `clip-path` —
        // нет: без обрезки формой подъём дал бы подложку во всю коробку.
        let hoist = c.mask_image.is_some()
            && c.clip_shape.is_none()
            && c.clip_polygon.is_none()
            && c.clip_inset.is_none()
            && c.clip_edges.is_none()
            && c.clip_xywh.is_none()
            && !c.clip_bare_box;
        let corners = [
            side(c.radius.tl),
            side(c.radius.tr),
            side(c.radius.br),
            side(c.radius.bl),
        ];
        let b = c.borders();
        out.push(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let [tl, tr, br, bl] = corners;
                    window.paint_backdrop_filter(
                        bounds,
                        gpui::Corners {
                            top_left: px(tl),
                            top_right: px(tr),
                            bottom_right: px(br),
                            bottom_left: px(bl),
                        },
                        radius,
                        m,
                        hoist,
                    );
                },
            )
            .absolute()
            .top(px(-side(b.top)))
            .right(px(-side(b.right)))
            .bottom(px(-side(b.bottom)))
            .left(px(-side(b.left)))
            .into_any_element(),
        );
    } else if let Some(radius) = c.backdrop_blur {
        // `backdrop-filter: blur(N)`: размывает то, что под элементом. Рисуется
        // проходом рендера (патч gpui), поэтому это канвас, а не стиль.
        let corner = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        out.push(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    window.paint_backdrop_blur_radius(
                        bounds,
                        gpui::Corners::all(px(corner)),
                        radius,
                    );
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element(),
        );
    }

    // Градиент из пяти и более стопов: заливка несёт четыре (патч GPUI), а
    // дальше осевой градиент по-прежнему рисуется полосами — по слою на пару
    // соседних стопов. Наклонный полосами не выразить.
    // Градиент, ушедший в растровую плитку (`gradient_as_tile`), полосами не
    // дублируется: они красили ВСЮ коробку поверх плитки — мимо размера,
    // повтора и пространства смешения.
    if let Some(g) = c.gradient.as_ref().filter(|_| !c.gradient_as_tile()) {
        let vertical = matches!(g.angle_deg as i32, 0 | 180);
        let horizontal = matches!(g.angle_deg as i32, 90 | 270);
        let reverse = matches!(g.angle_deg as i32, 0 | 270);
        // Стопы в точках рисуются полосами точной ширины: доля от них не
        // считается, длина оси известна только коробке. Отсчёт полос — от
        // верха/лева; обратное направление (0/270deg) идёт от низа/права.
        if !g.stops_px.is_empty() && !g.radial && (vertical || horizontal) {
            // Полосы в точках могут выйти за коробку (стопы длиннее оси) —
            // фон обрезается её краем, поэтому все полосы живут в общем
            // обрезающем слое на всю коробку.
            let mut bands: Vec<AnyElement> = vec![];
            // Фиксация css-images-3 §3.5.3 п.2: позиция не меньше наибольшей
            // из предыдущих. Без неё пара `green 4em, red 3em` (обратный
            // порядок — так пишут жёсткий край) пропускалась целиком, и под
            // полосами оставался долевой градиент на всю коробку
            // (`white-space-intrinsic-size-017/018`).
            let mut fixed = g.stops_px.clone();
            for i in 1..fixed.len() {
                fixed[i].1 = fixed[i].1.max(fixed[i - 1].1);
            }
            // До первого стопа — его цвет, после последнего — цвет последнего
            // (§3.5.3): полосы между стопами этого места не красили.
            const FAR: f32 = 1.0e5;
            let mut ext = Vec::with_capacity(fixed.len() + 2);
            if let Some(first) = fixed.first() {
                ext.push((first.0, first.1.min(0.0) - FAR));
            }
            ext.extend(fixed.iter().copied());
            if let Some(last) = fixed.last() {
                ext.push((last.0, last.1 + FAR));
            }
            for pair in ext.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (p0, p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (from, to) = (a.0, b.0);
                let band = crate::computed::Gradient {
                    angle_deg: if vertical { 180.0 } else { 90.0 },
                    radial: false,
                    circle: false,
                    from: if reverse { to } else { from },
                    to: if reverse { from } else { to },
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                    // Полоса наследует пространство интерполяции исходного градиента.
                    space: g.space,
                    hue: g.hue,
                };
                let layer = div().absolute().bg(crate::apply::fill(&band));
                bands.push(
                    match (vertical, reverse) {
                        (true, false) => layer.left_0().right_0().top(px(p0)).h(px(p1 - p0)),
                        (true, true) => layer.left_0().right_0().bottom(px(p0)).h(px(p1 - p0)),
                        (false, false) => layer.top_0().bottom_0().left(px(p0)).w(px(p1 - p0)),
                        (false, true) => layer.top_0().bottom_0().right(px(p0)).w(px(p1 - p0)),
                    }
                    .into_any_element(),
                );
            }
            out.push(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .overflow_hidden()
                    .children(bands)
                    .into_any_element(),
            );
        } else if g.stops.len() > 4 && !g.radial && (vertical || horizontal) {
            // Полоса перекрывает фон родителя целиком, поэтому скругление
            // приходится повторять на крайних полосах: иначе углы блока
            // становятся прямыми.
            let corner = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let last_band = g.stops.len() - 2;
            for (idx, pair) in g.stops.windows(2).enumerate() {
                let (a, b) = (pair[0], pair[1]);
                let (mut p0, mut p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (mut from, mut to) = (a.0, b.0);
                if reverse {
                    // Отсчёт полос всегда сверху/слева, поэтому обратное
                    // направление разворачивает и порядок, и цвета.
                    (p0, p1) = (1.0 - p1, 1.0 - p0);
                    (from, to) = (to, from);
                }
                let band = crate::computed::Gradient {
                    angle_deg: g.angle_deg,
                    radial: false,
                    circle: false,
                    from,
                    to,
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                    // Полоса наследует пространство интерполяции исходного градиента.
                    space: g.space,
                    hue: g.hue,
                };
                let mut layer = div().absolute().bg(crate::apply::fill(&band));
                // «Первая» полоса по направлению отрисовки, а не по списку:
                // при обратном направлении список развёрнут.
                let first_edge = if reverse { idx == last_band } else { idx == 0 };
                let last_edge = if reverse { idx == 0 } else { idx == last_band };
                if vertical {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_tr(px(corner(c.radius.tr)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_bl(px(corner(c.radius.bl)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                } else {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_bl(px(corner(c.radius.bl)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_tr(px(corner(c.radius.tr)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                }
                out.push(
                    if vertical {
                        layer
                            .left_0()
                            .right_0()
                            .top(gpui::relative(p0))
                            .h(gpui::relative(p1 - p0))
                    } else {
                        layer
                            .top_0()
                            .bottom_0()
                            .left(gpui::relative(p0))
                            .w(gpui::relative(p1 - p0))
                    }
                    .into_any_element(),
                );
            }
        }
    }

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
        let fallback = c.border_color.or(c.color).unwrap_or(crate::value::Color {
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
