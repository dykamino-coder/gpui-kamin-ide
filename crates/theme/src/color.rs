//! Цвет без зависимости от gpui — чистый крейт, быстрые unit-тесты.
//! Конверсия в gpui::Rgba — на стороне shell.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// `0xRRGGBB`, alpha = 1.0
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as f32 / 255.0,
            g: ((rgb >> 8) & 0xff) as f32 / 255.0,
            b: (rgb & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// `0xRRGGBB` + альфа (аналог rgba() / color-mix на transparent)
    pub const fn hex_a(rgb: u32, a: f32) -> Self {
        let mut c = Self::hex(rgb);
        c.a = a;
        c
    }

    /// Тинт: тот же цвет с альфой N% — эквивалент
    /// `color-mix(in srgb, X N%, transparent)` (plan/24 §8).
    pub const fn tint(self, alpha: f32) -> Self {
        Self { a: alpha, ..self }
    }

    /// Непрозрачный результат наложения self на подложку `bg`
    /// (для мест, где CSS давал полупрозрачный слой поверх известного фона,
    /// а нам нужен предвычисленный opaque).
    pub fn over(self, bg: Color) -> Color {
        let a = self.a + bg.a * (1.0 - self.a);
        if a == 0.0 {
            return Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
        }
        let blend = |fg_c: f32, bg_c: f32| (fg_c * self.a + bg_c * bg.a * (1.0 - self.a)) / a;
        Color {
            r: blend(self.r, bg.r),
            g: blend(self.g, bg.g),
            b: blend(self.b, bg.b),
            a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_channels() {
        let c = Color::hex(0x313240);
        assert!((c.r - 0x31 as f32 / 255.0).abs() < 1e-6);
        assert!((c.g - 0x32 as f32 / 255.0).abs() < 1e-6);
        assert!((c.b - 0x40 as f32 / 255.0).abs() < 1e-6);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn tint_keeps_rgb() {
        let c = Color::hex(0x89b4fa).tint(0.10);
        assert_eq!(c.r, Color::hex(0x89b4fa).r);
        assert_eq!(c.a, 0.10);
    }

    #[test]
    fn over_opaque_bg_yields_opaque() {
        let out = Color::hex_a(0xffffff, 0.5).over(Color::hex(0x000000));
        assert_eq!(out.a, 1.0);
        assert!((out.r - 0.5).abs() < 1e-3);
    }
}
