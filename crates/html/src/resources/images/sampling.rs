//! Общая выборка плотно упакованного BGRA: ближайшая точка или билинейная
//! интерполяция в премультиплицированной альфе с возвратом в straight alpha.
//! Нормализация входа, ограничения размеров и кэш остаются у вызывающего пути.

pub(crate) enum Sampling {
    Nearest,
    /// Формула дробной части сохраняет порядок округления вызывающего пути в f32.
    Bilinear(fn(f32) -> f32),
}

pub(crate) fn resize(
    bytes: &[u8],
    (iw, ih): (usize, usize),
    (w, h): (u32, u32),
    sampling: Sampling,
) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let fy = (y as f32 + 0.5) * ih as f32 / h as f32 - 0.5;
        for x in 0..w {
            let fx = (x as f32 + 0.5) * iw as f32 / w as f32 - 0.5;
            let Sampling::Bilinear(fraction) = sampling else {
                let sx = (fx + 0.5).floor().clamp(0.0, iw as f32 - 1.0) as usize;
                let sy = (fy + 0.5).floor().clamp(0.0, ih as f32 - 1.0) as usize;
                out.extend_from_slice(&bytes[(sy * iw + sx) * 4..][..4]);
                continue;
            };
            let (mut color, mut alpha) = ([0.0f32; 3], 0.0f32);
            for (dy, wy) in [(0, 1.0 - fraction(fy)), (1, fraction(fy))] {
                for (dx, wx) in [(0, 1.0 - fraction(fx)), (1, fraction(fx))] {
                    let sx = (fx.floor() as isize + dx).clamp(0, iw as isize - 1) as usize;
                    let sy = (fy.floor() as isize + dy).clamp(0, ih as isize - 1) as usize;
                    let p = &bytes[(sy * iw + sx) * 4..][..4];
                    let a = p[3] as f32 * wx * wy;
                    alpha += a;
                    for c in 0..3 {
                        color[c] += p[c] as f32 * a;
                    }
                }
            }
            for c in color {
                out.push(if alpha > 0.0 {
                    (c / alpha).round() as u8
                } else {
                    0
                });
            }
            out.push(alpha.round() as u8);
        }
    }
    out
}
