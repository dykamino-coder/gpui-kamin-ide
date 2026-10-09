//! Внешние и внутренние резкие тени коробки (css-backgrounds-3 §7.1).
// owner: A

use crate::render::paint_rect_minus;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{IntoElement, Styled, div, px};

pub(crate) fn outer_shadows(
    c: &Computed,
    out: &mut Vec<gpui::AnyElement>,
) {
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
            c.color.unwrap_or(crate::style::values::value::Color {
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
                crate::paint::background::exact_layer::ExactLayer::inset(
                    [-bt, -br, -bb, -bl],
                    move |r: gpui::Bounds<gpui::Pixels>, window: &mut gpui::Window| {
                        // Blink offsets the UNSNAPPED border box and snaps
                        // the result (a 72.5-device-pixel top with a -62.5
                        // offset lands on row 10, not 11).
                        let r = crate::paint::background::exact_layer::positioning(r);
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
}

pub(crate) fn inset_shadows(
    c: &Computed,
    out: &mut Vec<gpui::AnyElement>,
) {
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
            c.color.unwrap_or(crate::style::values::value::Color {
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
                crate::paint::background::exact_layer::ExactLayer::inset(
                    [0.0; 4],
                    move |r: gpui::Bounds<gpui::Pixels>, window: &mut gpui::Window| {
                        let r = crate::paint::background::exact_layer::positioning(r);
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
                (crate::style::apply::radius_px(c, r).unwrap_or(0.0) - side(a).max(side(b))).max(0.0)
            };
            ring = ring
                .rounded_tl(px(pad(c.radius.tl, bw.top, bw.left)))
                .rounded_tr(px(pad(c.radius.tr, bw.top, bw.right)))
                .rounded_br(px(pad(c.radius.br, bw.bottom, bw.right)))
                .rounded_bl(px(pad(c.radius.bl, bw.bottom, bw.left)));
        }
        out.push(ring.into_any_element());
    }
}
