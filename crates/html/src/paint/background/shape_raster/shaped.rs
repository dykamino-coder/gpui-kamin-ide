//! Контуры `corner-shape` для отрисовки: внешний — обрезка группы элемента,
//! внутренний — кольцо рамки (css-borders-4 §corner-shape-rendering: рамка
//! лежит между внешним и внутренним контуром, фон и содержимое режет
//! внешний). Прямолинейный контур до восьми вершин уходит многоугольником в
//! шейдер `clip-path: polygon()`, прочие — SVG-путём в растр `clip-path:
//! path()`: так край фигурного угла совпадает с эквивалентной обрезкой.
//! Углы только с выемками и прямыми (оси координат) остаются у
//! `linear_corners` — их покрытие на сетке пикселей двоичное.

use super::contour::{self, Inner, P, Shaped};
use super::{Rrect, linear_corners, parse_rrect};
use crate::paint::background::{MaskLayer, compose_mask_layers};
use crate::style::values::value::Color;
use gpui::RenderImage;
use std::sync::Arc;

/// Разбор `rrect(...)` в CSS-точках коробки bw×bh; None — углы круглые или
/// контур целиком прямоугольный по осям (его ведёт `linear_corners`).
fn shaped(args: &str, bw: f32, bh: f32) -> Option<(Shaped, Rrect)> {
    let r = parse_rrect(args, bw, bh, 1.0)?;
    let shaped = r
        .corners
        .iter()
        .zip(r.k)
        .any(|(&(rx, ry), k)| rx > 0.0 && ry > 0.0 && (k - 1.0).abs() > 1e-3);
    if !shaped || linear_corners::contour(&r, bw, bh, [0.0; 4]).is_some() {
        return None;
    }
    Some((
        Shaped {
            rect: (0.0, 0.0, bw, bh),
            radii: r.corners,
            k: r.k,
        },
        r,
    ))
}

/// Внешний контур коробки bw×bh в CSS-точках от её угла.
pub fn contour_outline(args: &str, bw: f32, bh: f32) -> Option<contour::Path> {
    shaped(args, bw, bh).map(|(s, _)| contour::outer(&s))
}

/// Кольцо рамки: многоугольник внутреннего контура (рисуется обратной
/// обрезкой группы) либо готовая картинка в цвете рамки.
pub enum Ring {
    Outside(Vec<P>),
    Image(Arc<RenderImage>),
}

/// Кольцо по записи `rrect(... / t r b l)` на растре pw×ph при плотности sf.
pub fn contour_ring(args: &str, pw: u32, ph: u32, sf: f32, colour: Color) -> Option<Ring> {
    let (bw, bh) = (pw as f32 / sf, ph as f32 / sf);
    let (s, r) = shaped(args, bw, bh)?;
    let [t, rr, b, l] = r.ring?;
    let body = match contour::inner(&s, [t, rr, b, l]) {
        Inner::Polygon(p) if p.len() < 3 => String::new(),
        Inner::Polygon(p) if p.len() <= 8 => return Some(Ring::Outside(p)),
        Inner::Polygon(p) => {
            let d = contour::Path {
                start: p[0],
                segs: p[1..].iter().map(|q| contour::Seg::Line(*q)).collect(),
            };
            format!(r##"<path fill="#ffffff" d="{}"/>"##, d.svg_d())
        }
        // Прямоугольник минус вырезы углов (Blink: `Op(kDifference)`):
        // белый прямоугольник и чёрные вырезы в маске-светимости, ею гасится
        // заливка всего холста — край прямоугольника сглажен один раз.
        Inner::Clipped { rect, clips } => {
            let d: Vec<String> = clips.iter().map(|c| c.svg_d()).collect();
            format!(
                r##"<defs><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="{bw}" height="{bh}"><rect x="{}" y="{}" width="{}" height="{}" fill="#ffffff"/><path fill="#000000" d="{}"/></mask></defs><rect width="{bw}" height="{bh}" fill="#ffffff" mask="url(#m)"/>"##,
                rect.0,
                rect.1,
                rect.2,
                rect.3,
                d.join(" ")
            )
        }
    };
    let markup = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}">{body}</svg>"##
    );
    let image = crate::svg::raster::rasterize(&markup, bw, bh)?;
    let mask = compose_mask_layers(
        &[MaskLayer {
            image,
            tile: [0.0, 0.0, pw as f32, ph as f32],
            no_repeat: (true, true),
            gap: (0.0, 0.0),
            snap: false,
            op: 0,
            luminance: true,
        }],
        pw,
        ph,
    )?;
    linear_corners::ring(&mask, colour).map(Ring::Image)
}
