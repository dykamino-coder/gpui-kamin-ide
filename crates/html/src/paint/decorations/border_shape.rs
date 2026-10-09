//! Рамка `border-shape` (css-borders-4 §border-shape) растровым слоем.
// owner: A

use crate::render::*;

pub(crate) fn border_shape_layer(
    c: &Computed,
    out: &mut Vec<gpui::AnyElement>,
) {
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
}
