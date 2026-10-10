//! Направление линейного градиента из первой части записи (угол, `to <сторона>`, форма радиального): угол и номер первого стопа.

use super::*;

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
