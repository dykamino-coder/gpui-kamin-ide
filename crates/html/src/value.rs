//! Разбор значений CSS: длины, цвета, числа.
//!
//! Отдельный модуль, потому что одно и то же значение приходит из трёх мест —
//! `style=""`, правило в `<style>` и значение по умолчанию тега, — и разбирать
//! его надо одинаково.

/// Длина в терминах, которые понимает GPUI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    /// `12px`, `1.5rem` — переводим всё в px по базовому размеру шрифта.
    Px(f32),
    /// `50%` — доля родителя.
    Pct(f32),
    /// `auto`
    Auto,
}

impl Len {
    /// `1rem` = 16px, как в браузере по умолчанию. Свой базовый размер шрифта
    /// мы не задаём: документ рисуется внутри чата, где размер уже выбран.
    const REM_PX: f32 = 16.0;

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.eq_ignore_ascii_case("auto") {
            return Some(Len::Auto);
        }
        if let Some(num) = s.strip_suffix('%') {
            return num.trim().parse::<f32>().ok().map(|v| Len::Pct(v / 100.0));
        }
        for (suffix, factor) in [
            ("px", 1.0),
            ("rem", Self::REM_PX),
            // `em` без каскада шрифтов считаем от базового — приблизительно, но
            // предсказуемо; точный `em` требует наследования размера, которого
            // в модели стиля GPUI нет.
            ("em", Self::REM_PX),
            ("pt", 96.0 / 72.0),
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return num.trim().parse::<f32>().ok().map(|v| Len::Px(v * factor));
            }
        }
        // Голое число: в CSS допустимо только для 0, но модель часто пишет
        // `padding: 8` — принимаем как px, иначе виджет разъезжается.
        s.parse::<f32>().ok().map(Len::Px)
    }
}

/// Цвет в формате GPUI (`Rgba` → `Hsla` конвертируется на месте применения).
#[derive(Clone, Copy, Debug, PartialEq)]
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

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
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
        named(s)
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        let h = hex.trim();
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

    fn parse_rgb(inner: &str) -> Option<Self> {
        // Принимаем и запятые, и пробельный синтаксис `rgb(1 2 3 / 50%)`.
        let cleaned = inner.replace('/', " ");
        let parts: Vec<&str> = cleaned
            .split([',', ' '])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let chan = |p: &str| -> Option<f32> {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f32>().ok().map(|v| v / 255.0)
            }
        };
        let alpha = parts.get(3).map_or(Some(1.0), |p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f32>().ok()
            }
        })?;
        Some(Color {
            r: chan(parts[0])?,
            g: chan(parts[1])?,
            b: chan(parts[2])?,
            a: alpha,
        })
    }
}

/// Именованные цвета. Полный список CSS — 148 имён; держим те, что реально
/// встречаются в разметке модели и в нашей вёрстке, плюс базовые 16.
fn named(name: &str) -> Option<Color> {
    let rgb = |r: u8, g: u8, b: u8| {
        Some(Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        })
    };
    match name.to_ascii_lowercase().as_str() {
        "black" => rgb(0, 0, 0),
        "white" => rgb(255, 255, 255),
        "red" => rgb(255, 0, 0),
        "green" => rgb(0, 128, 0),
        "lime" => rgb(0, 255, 0),
        "blue" => rgb(0, 0, 255),
        "yellow" => rgb(255, 255, 0),
        "cyan" | "aqua" => rgb(0, 255, 255),
        "magenta" | "fuchsia" => rgb(255, 0, 255),
        "gray" | "grey" => rgb(128, 128, 128),
        "silver" => rgb(192, 192, 192),
        "maroon" => rgb(128, 0, 0),
        "olive" => rgb(128, 128, 0),
        "navy" => rgb(0, 0, 128),
        "purple" => rgb(128, 0, 128),
        "teal" => rgb(0, 128, 128),
        "orange" => rgb(255, 165, 0),
        "gold" => rgb(255, 215, 0),
        "pink" => rgb(255, 192, 203),
        "brown" => rgb(165, 42, 42),
        "crimson" => rgb(220, 20, 60),
        "coral" => rgb(255, 127, 80),
        "salmon" => rgb(250, 128, 114),
        "tomato" => rgb(255, 99, 71),
        "orangered" => rgb(255, 69, 0),
        "darkred" => rgb(139, 0, 0),
        "indigo" => rgb(75, 0, 130),
        "violet" => rgb(238, 130, 238),
        "steelblue" => rgb(70, 130, 180),
        "skyblue" => rgb(135, 206, 235),
        "dodgerblue" => rgb(30, 144, 255),
        "royalblue" => rgb(65, 105, 225),
        "cornflowerblue" => rgb(100, 149, 237),
        "seagreen" => rgb(46, 139, 87),
        "forestgreen" => rgb(34, 139, 34),
        "limegreen" => rgb(50, 205, 50),
        "springgreen" => rgb(0, 255, 127),
        "darkgreen" => rgb(0, 100, 0),
        "khaki" => rgb(240, 230, 140),
        "beige" => rgb(245, 245, 220),
        "ivory" => rgb(255, 255, 240),
        "lavender" => rgb(230, 230, 250),
        "plum" => rgb(221, 160, 221),
        "slategray" | "slategrey" => rgb(112, 128, 144),
        "darkslategray" | "darkslategrey" => rgb(47, 79, 79),
        "dimgray" | "dimgrey" => rgb(105, 105, 105),
        "lightgray" | "lightgrey" => rgb(211, 211, 211),
        "darkgray" | "darkgrey" => rgb(169, 169, 169),
        "whitesmoke" => rgb(245, 245, 245),
        "gainsboro" => rgb(220, 220, 220),
        "goldenrod" => rgb(218, 165, 32),
        "chocolate" => rgb(210, 105, 30),
        "sienna" => rgb(160, 82, 45),
        "turquoise" => rgb(64, 224, 208),
        "aquamarine" => rgb(127, 255, 212),
        "mediumpurple" => rgb(147, 112, 219),
        "rebeccapurple" => rgb(102, 51, 153),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(Len::parse("12px"), Some(Len::Px(12.0)));
        assert_eq!(Len::parse(" 1.5rem "), Some(Len::Px(24.0)));
        assert_eq!(Len::parse("50%"), Some(Len::Pct(0.5)));
        assert_eq!(Len::parse("auto"), Some(Len::Auto));
        // Голое число принимаем: модель часто пишет `padding: 8`.
        assert_eq!(Len::parse("8"), Some(Len::Px(8.0)));
        assert_eq!(Len::parse("нет"), None);
    }

    #[test]
    fn colors_hex() {
        assert_eq!(
            Color::parse("#fff"),
            Some(Color {
                r: 1.,
                g: 1.,
                b: 1.,
                a: 1.
            })
        );
        let c = Color::parse("#8ab4f8").unwrap();
        assert!((c.r - 0.541).abs() < 0.01 && (c.b - 0.972).abs() < 0.01);
        assert_eq!(
            Color::parse("#00000080").map(|c| (c.a * 100.).round()),
            Some(50.0)
        );
    }

    #[test]
    fn colors_functions_and_names() {
        assert_eq!(
            Color::parse("rgb(255, 0, 0)"),
            Some(Color {
                r: 1.,
                g: 0.,
                b: 0.,
                a: 1.
            })
        );
        assert_eq!(Color::parse("rgba(0 0 0 / 50%)").map(|c| c.a), Some(0.5));
        assert_eq!(
            Color::parse("teal").map(|c| (c.g * 255.).round()),
            Some(128.0)
        );
        assert_eq!(Color::parse("transparent").map(|c| c.a), Some(0.0));
        assert_eq!(Color::parse("не-цвет"), None);
    }
}
