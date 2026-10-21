//! Сепарабельные режимы `background-blend-mode` (CSS Compositing 1 §9) для одного цвета:
//! функция смешения по каналам с учётом альфы подложки и последующее source-over.
//! Несепарабельные режимы возвращают `None` и остаются на пути картинок и групп.

use crate::style::computed::BlendMode;
use crate::style::values::value::Color;

pub(super) fn composite(source: Color, backdrop: Color, mode: BlendMode) -> Option<Color> {
    let blend = |s: f32, b: f32| -> Option<f32> {
        Some(match mode {
            BlendMode::Normal => s,
            BlendMode::Multiply => s * b,
            BlendMode::Screen => b + s - b * s,
            BlendMode::Darken => b.min(s),
            BlendMode::Lighten => b.max(s),
            BlendMode::Overlay => {
                if b <= 0.5 {
                    2.0 * b * s
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
                }
            }
            BlendMode::HardLight => {
                if s <= 0.5 {
                    2.0 * b * s
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
                }
            }
            BlendMode::ColorDodge => {
                if b == 0.0 {
                    0.0
                } else if s == 1.0 {
                    1.0
                } else {
                    (b / (1.0 - s)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if b == 1.0 {
                    1.0
                } else if s == 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - b) / s).min(1.0)
                }
            }
            BlendMode::SoftLight => {
                if s <= 0.5 {
                    b - (1.0 - 2.0 * s) * b * (1.0 - b)
                } else {
                    let d = if b <= 0.25 {
                        ((16.0 * b - 12.0) * b + 4.0) * b
                    } else {
                        b.sqrt()
                    };
                    b + (2.0 * s - 1.0) * (d - b)
                }
            }
            BlendMode::Difference => (b - s).abs(),
            BlendMode::Exclusion => b + s - 2.0 * b * s,
            // Nonseparable modes retain the existing image/group path.
            _ => return None,
        })
    };
    let channel = |s, b| blend(s, b).map(|mixed| (1.0 - backdrop.a) * s + backdrop.a * mixed);
    Some(super::over(
        Color {
            r: channel(source.r, backdrop.r)?,
            g: channel(source.g, backdrop.g)?,
            b: channel(source.b, backdrop.b)?,
            a: source.a,
        },
        backdrop,
    ))
}
