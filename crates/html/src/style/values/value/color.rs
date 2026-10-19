//! Цвет CSS (Color): разбор hex/rgb/hsl и функций цвета, тёмная схема.

use super::*;
use crate::document::state::ValueCx;

/// Цвет в формате GPUI (`Rgba` → `Hsla` конвертируется на месте применения).
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn to_hsla(self) -> gpui::Hsla {
        gpui::Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
        .into()
    }

    pub fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let s = raw.trim();
        // `light-dark(светлый, тёмный)` (css-color-5 §light-dark): вариант по
        // используемой схеме узла. Прежде функция не разбиралась, и
        // объявление пропадало целиком.
        if s.get(..11)
            .is_some_and(|h| h.eq_ignore_ascii_case("light-dark("))
            && s.ends_with(')')
        {
            let args = crate::style::css::split_args(&s[11..s.len() - 1]);
            if args.len() == 2 {
                return Self::parse(args[usize::from(dark_scheme())], vcx);
            }
            return None;
        }
        if s.eq_ignore_ascii_case("transparent") {
            return Some(Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.,
            });
        }
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        if let Some(inner) = s
            .strip_prefix("rgba(")
            .or_else(|| s.strip_prefix("rgb("))
            .and_then(|v| v.strip_suffix(')'))
        {
            return Self::parse_rgb(inner);
        }
        // `hsl()` — ирония: внутреннее представление GPUI и есть HSL, но записи
        // этой не понимали, и цвет молча терялся.
        if let Some(inner) = s
            .strip_prefix("hsla(")
            .or_else(|| s.strip_prefix("hsl("))
            .and_then(|v| v.strip_suffix(')'))
        {
            return Self::parse_hsl(inner);
        }
        // Записи CSS Color 4 (`lab`, `oklch`, `color()`, `color-mix()` и
        // родня) — своим разбором: они задают цвет в других системах
        // координат, и без настоящего преобразования не приблизить.
        if let Some((r, g, b, a)) = crate::style::values::color_space::parse(s, vcx) {
            return Some(Color {
                r: r.clamp(0.0, 1.0),
                g: g.clamp(0.0, 1.0),
                b: b.clamp(0.0, 1.0),
                a,
            });
        }
        named(s)
    }

    pub(super) fn parse_hex(hex: &str) -> Option<Self> {
        let h = hex;
        // Срезы ниже — байтовые: не-ASCII знак (`#aфa`) резал бы UTF-8
        // посреди кода и РОНЯЛ процесс на произвольной странице.
        if !h.is_ascii() {
            return None;
        }
        let byte = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|v| v as f32 / 255.0)
        };
        // Короткая форма `#abc` — каждый разряд удваивается.
        let nib = |i: usize| {
            u8::from_str_radix(&h[i..i + 1], 16)
                .ok()
                .map(|v| (v * 17) as f32 / 255.0)
        };
        match h.len() {
            3 => Some(Color {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: 1.0,
            }),
            4 => Some(Color {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: nib(3)?,
            }),
            6 => Some(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: 1.0,
            }),
            8 => Some(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }

    /// `hsl(210 40% 50% / 80%)` и `hsl(210, 40%, 50%)`.
    pub(super) fn parse_hsl(inner: &str) -> Option<Self> {
        color_hsl::parse(inner)
    }

    pub(super) fn parse_rgb(inner: &str) -> Option<Self> {
        color_channels::rgb(inner)
    }
}
