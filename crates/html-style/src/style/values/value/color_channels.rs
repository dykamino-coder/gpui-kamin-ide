//! Разбор каналов RGB с отдельной грамматикой старой и современной записи.

use super::Color;

/// CSS Color 4 §5.1: запятые требуют трёх каналов одного типа;
/// пробельная запись допускает смешанные типы и `none`.
pub(super) fn rgb(inner: &str) -> Option<Color> {
    let (channels, alpha): (Vec<&str>, Option<&str>) = if inner.contains(',') {
        let parts: Vec<_> = inner.split(',').map(str::trim).collect();
        if !(3..=4).contains(&parts.len())
            || parts.iter().any(|p| p.is_empty() || *p == "none")
            || inner.contains('/')
            || parts[..3]
                .iter()
                .any(|p| p.ends_with('%') != parts[0].ends_with('%'))
        {
            return None;
        }
        (parts[..3].to_vec(), parts.get(3).copied())
    } else {
        let mut sides = inner.split('/');
        let channels: Vec<_> = sides.next()?.split_ascii_whitespace().collect();
        let alpha = sides.next().map(str::trim);
        if channels.len() != 3 || sides.next().is_some() || alpha.is_some_and(|a| a.is_empty()) {
            return None;
        }
        (channels, alpha)
    };
    let component = |p: &str, scale: f32| -> Option<f32> {
        if p == "none" {
            Some(0.0)
        } else if let Some(pct) = p.strip_suffix('%') {
            pct.parse::<f32>().ok().map(|v| v / 100.0)
        } else {
            p.parse::<f32>().ok().map(|v| v / scale)
        }
    };
    // Каналы и альфа зажимаются при разборе (CSS Color 4 §5.1).
    Some(Color {
        r: component(channels[0], 255.0)?.clamp(0.0, 1.0),
        g: component(channels[1], 255.0)?.clamp(0.0, 1.0),
        b: component(channels[2], 255.0)?.clamp(0.0, 1.0),
        a: alpha
            .map_or(Some(1.0), |a| component(a, 1.0))?
            .clamp(0.0, 1.0),
    })
}
