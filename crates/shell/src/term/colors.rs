//! Цвета терминала: ANSI-палитра и конверсия alacritty.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::term::ANSI16;

/// Цвет alacritty → 0xRRGGBBAA; None = default fg темы.
pub fn color_u32(c: alacritty_terminal::vte::ansi::Color) -> Option<u32> {
    use alacritty_terminal::vte::ansi::{Color, NamedColor};
    match c {
        Color::Spec(rgb) => {
            Some(((rgb.r as u32) << 24) | ((rgb.g as u32) << 16) | ((rgb.b as u32) << 8) | 0xff)
        }
        Color::Indexed(i) => Some(indexed_color(i)),
        Color::Named(n) => {
            let idx: usize = match n {
                NamedColor::Black | NamedColor::DimBlack => 0,
                NamedColor::Red | NamedColor::DimRed => 1,
                NamedColor::Green | NamedColor::DimGreen => 2,
                NamedColor::Yellow | NamedColor::DimYellow => 3,
                NamedColor::Blue | NamedColor::DimBlue => 4,
                NamedColor::Magenta | NamedColor::DimMagenta => 5,
                NamedColor::Cyan | NamedColor::DimCyan => 6,
                NamedColor::White | NamedColor::DimWhite => 7,
                NamedColor::BrightBlack => 8,
                NamedColor::BrightRed => 9,
                NamedColor::BrightGreen => 10,
                NamedColor::BrightYellow => 11,
                NamedColor::BrightBlue => 12,
                NamedColor::BrightMagenta => 13,
                NamedColor::BrightCyan => 14,
                NamedColor::BrightWhite => 15,
                _ => return None, // Foreground/Background/Cursor → тема
            };
            Some(ANSI16[idx])
        }
    }
}
/// 256-цветный индекс → RGBA (16 ANSI + куб 6×6×6 + 24 серых).
pub fn indexed_color(i: u8) -> u32 {
    match i {
        0..=15 => ANSI16[i as usize],
        16..=231 => {
            let v = i as u32 - 16;
            let comp = |x: u32| if x == 0 { 0 } else { 55 + 40 * x };
            let (r, g, b) = (comp(v / 36), comp(v / 6 % 6), comp(v % 6));
            (r << 24) | (g << 16) | (b << 8) | 0xff
        }
        232..=255 => {
            let g = 8 + 10 * (i as u32 - 232);
            (g << 24) | (g << 16) | (g << 8) | 0xff
        }
    }
}
