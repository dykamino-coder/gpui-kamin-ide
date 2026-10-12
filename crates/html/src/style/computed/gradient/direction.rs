//! Направление линейного градиента из первой части записи (угол, `to <сторона>`, форма радиального): угол и номер первого стопа.

use super::*;

/// CSS Images 3 §3.1: a corner direction is perpendicular to the diagonal
/// between the two adjacent corners, rather than a fixed 45-degree bearing.
/// Blink CSSLinearGradientValue::CreateGradient derives rise/run from the
/// concrete image size before constructing its endpoints.
pub(crate) fn corner_angle(raw: &str, (width, height): (f32, f32)) -> Option<f32> {
    let inner = raw
        .strip_prefix("repeating-")
        .unwrap_or(raw)
        .strip_prefix("linear-gradient(")?
        .strip_suffix(')')?;
    let head = crate::style::css::split_args(inner).first()?.trim();
    let direction = head.split(" in ").next()?.trim();
    let (x, y) = match direction {
        "to top right" | "to right top" => (1.0, -1.0),
        "to bottom right" | "to right bottom" => (1.0, 1.0),
        "to bottom left" | "to left bottom" => (-1.0, 1.0),
        "to top left" | "to left top" => (-1.0, -1.0),
        _ => return None,
    };
    Some(
        (x * height)
            .atan2(-y * width)
            .to_degrees()
            .rem_euclid(360.0),
    )
}

pub(super) fn gradient_direction(head: &str, radial: bool, mut idx: usize) -> Option<(f32, usize)> {
    let angle = match head {
        a if a.ends_with("deg") => {
            idx = 1;
            a.trim_end_matches("deg").trim().parse().unwrap_or(180.0)
        }
        // Размерность с другой единицей: `grad`/`rad`/`turn` — законный угол,
        // прочее (`90degree`, `100gradian`, `1.57radian`, `0.25turns`) делает
        // запись негодной целиком (`angle-units-001`). Прежде такой довод
        // падал в `_ => 180.0`, не читался цветом и молча пропускался —
        // градиент из оставшихся стопов КРАСИЛ.
        a if !radial && gradient_angle(a).is_some() => {
            idx = 1;
            gradient_angle(a).flatten()?
        }
        "to right" => {
            idx = 1;
            90.0
        }
        "to left" => {
            idx = 1;
            270.0
        }
        "to bottom" => {
            idx = 1;
            180.0
        }
        "to top" => {
            idx = 1;
            0.0
        }
        "to bottom right" | "to right bottom" => {
            idx = 1;
            135.0
        }
        // Остальные два угла (css-images-3 §3.1): без них направление
        // уходило в разбор стопов и выбрасывалось, а градиент шёл сверху вниз.
        "to bottom left" | "to left bottom" => {
            idx = 1;
            225.0
        }
        "to top left" | "to left top" => {
            idx = 1;
            315.0
        }
        "to top right" | "to right top" => {
            idx = 1;
            45.0
        }
        // У радиального первым идёт описание формы (`circle at center`) —
        // цветом оно не разбирается, поэтому просто пропускается.
        first
            if radial && Color::parse(first.split_whitespace().next().unwrap_or("")).is_none() =>
        {
            idx = 1;
            180.0
        }
        _ => 180.0,
    };
    Some((angle, idx))
}

#[cfg(test)]
mod tests {
    use super::corner_angle;

    #[test]
    fn corner_bearing_depends_on_the_concrete_image_ratio() {
        let raw = "linear-gradient(to right bottom, black 50%, white 50%)";
        let angle = corner_angle(raw, (200.0, 100.0)).unwrap();
        assert!((angle - 153.43495).abs() < 0.0001);
        assert_eq!(corner_angle(raw, (100.0, 100.0)), Some(135.0));
        assert_eq!(
            corner_angle("linear-gradient(45deg, black, white)", (200.0, 100.0)),
            None
        );
    }
}
