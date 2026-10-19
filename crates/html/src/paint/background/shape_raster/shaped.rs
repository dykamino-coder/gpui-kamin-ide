//! Контуры `corner-shape` для отрисовки: внешний — обрезка группы элемента,
//! внутренний — кольцо рамки (css-borders-4 §corner-shape-rendering: рамка
//! лежит между внешним и внутренним контуром, фон и содержимое режет
//! внешний). Прямолинейный контур до восьми вершин уходит многоугольником в
//! шейдер `clip-path: polygon()`, прочие — tiny-skia-путём в растр маски: так край фигурного угла совпадает с эквивалентной обрезкой.
//! Углы только с выемками и прямыми (оси координат) остаются у
//! `linear_corners` — их покрытие на сетке пикселей двоичное.

use super::contour::{self, Inner, P, Shaped};
use super::{Rrect, linear_corners, parse_rrect};
use crate::paint::background::{MaskLayer, compose_mask_layers};
use crate::style::values::value::Color;
use crate::svg::path_commands::Geometry;
use crate::svg::path_scene::*;
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
        }
        .constrained(),
        r,
    ))
}

/// Контур коробки bw×bh, сдвинутый внутрь на t/r/b/l (отрицательные —
/// наружу): форма тени с разлётом и контура `outline` (css-borders-4
/// §corner-shaping). Углы только круглые — None (их ведёт скругление).
pub fn contour_offset(args: &str, bw: f32, bh: f32, insets: [f32; 4]) -> Option<contour::Path> {
    let r = parse_rrect(args, bw, bh, 1.0)?;
    let shaped = r
        .corners
        .iter()
        .zip(r.k)
        .any(|(&(rx, ry), k)| rx > 0.0 && ry > 0.0 && (k - 1.0).abs() > 1e-3);
    let s = Shaped {
        rect: (0.0, 0.0, bw, bh),
        radii: r.corners,
        k: r.k,
    };
    shaped.then(|| contour::offset(&s.constrained(), insets))
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
    let root = match contour::inner(&s, [t, rr, b, l]) {
        Inner::Polygon(p) if p.len() < 3 => group(Vec::new()),
        Inner::Polygon(p) if p.len() <= 8 => return Some(Ring::Outside(p)),
        Inner::Polygon(p) => {
            let path = contour::Path {
                start: p[0],
                segs: p[1..].iter().map(|q| contour::Seg::Line(*q)).collect(),
            };
            let mut d = Geometry::default();
            path.commands(&mut d, 0.0, 0.0);
            coverage(bw, bh, d.finish(), FillRule::NonZero).root
        }
        // Luminance overdraw subtracts corner cutouts before masking the canvas.
        Inner::Clipped { rect, clips } => {
            let mut d = Geometry::default();
            for clip in clips {
                clip.commands(&mut d, 0.0, 0.0);
            }
            let mut children: Vec<_> = rectangle(rect.0, rect.1, rect.2, rect.3, gray(true))
                .into_iter()
                .collect();
            if let Some(data) = d.finish() {
                children.push(shape(data, FillRule::NonZero, Some(gray(false)), None));
            }
            masked(bw, bh, gray(true), group(children))?
        }
    };
    let image = Scene::new(bw, bh, root).rasterize()?;
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
