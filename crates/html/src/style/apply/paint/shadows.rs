//! apply_paint, этап теней: цвет тени без своего цвета (currentColor), внутренние и внешние box-shadow.

use super::*;

pub(super) fn paint_shadows(mut d: Div, c: &Computed) -> Div {
    // Тень без своего цвета — цветом текста ЭТОГО элемента (метка:
    // отрицательная альфа; css-backgrounds-3 §7, currentColor).
    let shadow_colour = |sh: &crate::style::computed::Shadow| {
        if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::style::values::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        }
    };
    // У `border-shape` обе тени повторяют фигуру и рисуются растром
    // (`render::grouped` — наружные, `render::decorations` — внутренние);
    // прямоугольные примитивы квада легли бы поверх и мимо фигуры.
    if !c.inset_shadows.is_empty() && c.border_shape.is_none() {
        d.style().inset_box_shadow = Some(
            c.inset_shadows
                .iter()
                // css-backgrounds-3 §7.1: «The first shadow is on top»; the
                // list is painted bottom-up, so the last one goes first.
                .rev()
                // Резкую внутреннюю рисует слой-кольцо в декорациях: примитив
                // с нулевым размытием вырождается в шейдере (как у внешней).
                .filter(|s| s.blur > 0.0)
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    // Шейдер gpui читает `blur_radius` как σ гаусса, а в CSS
                    // радиус размытия — вдвое больше σ (css-backgrounds-3
                    // §box-shadow: «blur radius … resulting shadow must
                    // approximate … a Gaussian blur with a standard deviation
                    // equal to half the blur radius»; Blink `BlurAsSigma`).
                    // Прежде тень выходила вдвое шире — как у `text-shadow`
                    // (`text_shadow_layers`), которая σ уже делит.
                    blur_radius: px(s.blur * 0.5),
                    spread_radius: px(s.spread),
                    inset: true,
                })
                .collect(),
        );
    }
    if !c.shadows.is_empty() && c.border_shape.is_none() {
        d = d.shadow(
            c.shadows
                .iter()
                // css-backgrounds-3 §7.1: the first shadow is on top.
                .rev()
                // Резкую тень (без размытия) рисует слой-квад в декорациях:
                // примитив с нулевым размытием вырождается в шейдере.
                .filter(|s| s.blur > 0.0)
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    // Шейдер gpui читает `blur_radius` как σ гаусса, а в CSS
                    // радиус размытия — вдвое больше σ (css-backgrounds-3
                    // §box-shadow: «blur radius … resulting shadow must
                    // approximate … a Gaussian blur with a standard deviation
                    // equal to half the blur radius»; Blink `BlurAsSigma`).
                    // Прежде тень выходила вдвое шире — как у `text-shadow`
                    // (`text_shadow_layers`), которая σ уже делит.
                    blur_radius: px(s.blur * 0.5),
                    spread_radius: px(s.spread),
                    inset: false,
                })
                .collect::<Vec<_>>(),
        );
    }
    d
}
