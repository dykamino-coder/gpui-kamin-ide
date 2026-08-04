//! Конверсия kamin-theme (чистый крейт) → типы gpui.

use gpui::Rgba;
use kamin_theme::Color;

/// Тот же цвет с другой альфой — в CSS это `rgba(var(--token), a)`.
/// Раньше эта функция была скопирована в 31 файл слово в слово.
pub fn tint(mut c: Rgba, a: f32) -> Rgba {
    c.a = a;
    c
}

pub fn rgba(c: Color) -> Rgba {
    Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// "#rrggbb" → Rgba (цвета сессий из host); мусор → fallback.
/// Произвольный CSS-цвет из `style={{ color }}` расширения: hex 3/6,
/// `rgb()/rgba()` и базовые именованные. Браузер принимает всё это, поэтому
/// статус-элемент красится и по `red`, и по `rgb(...)` (ревью ц.17: раньше
/// проходил только `#rrggbb`).
pub fn parse_css_color(s: &str, fallback: Rgba) -> Rgba {
    let t = s.trim().to_ascii_lowercase();
    if let Some(hex) = t.strip_prefix('#') {
        if hex.len() == 3 {
            let expanded: String = hex.chars().flat_map(|c| [c, c]).collect();
            return parse_hex(&expanded, fallback);
        }
        return parse_hex(hex, fallback);
    }
    if let Some(rest) = t
        .strip_prefix("rgba(")
        .or_else(|| t.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let parts: Vec<&str> = rest
            .split([',', '/', ' '])
            .filter(|x| !x.is_empty())
            .collect();
        if parts.len() >= 3 {
            let ch = |i: usize| parts[i].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let a = parts
                .get(3)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            return Rgba {
                r: ch(0),
                g: ch(1),
                b: ch(2),
                a,
            };
        }
        return fallback;
    }
    let named: u32 = match t.as_str() {
        "red" => 0xff_0000,
        "green" => 0x00_8000,
        "lime" => 0x00_ff00,
        "blue" => 0x00_00ff,
        "yellow" => 0xff_ff00,
        "orange" => 0xff_a500,
        "purple" => 0x80_0080,
        "magenta" | "fuchsia" => 0xff_00ff,
        "cyan" | "aqua" => 0x00_ffff,
        "pink" => 0xff_c0cb,
        "brown" => 0xa5_2a2a,
        "gold" => 0xff_d700,
        "teal" => 0x00_8080,
        "navy" => 0x00_0080,
        "olive" => 0x80_8000,
        "maroon" => 0x80_0000,
        "silver" => 0xc0_c0c0,
        "gray" | "grey" => 0x80_8080,
        "white" => 0xff_ffff,
        "black" => 0x00_0000,
        _ => return fallback,
    };
    rgba(Color::hex(named))
}

pub fn parse_hex(s: &str, fallback: Rgba) -> Rgba {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return fallback;
    }
    match u32::from_str_radix(hex, 16) {
        Ok(v) => rgba(Color::hex(v)),
        Err(_) => fallback,
    }
}
