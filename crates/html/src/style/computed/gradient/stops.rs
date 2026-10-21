//! Позиции остановок CSS-градиента по умолчанию и их монотонная коррекция.

/// Расставить позиции стопов по правилам css-images: крайние без позиции — на
/// края, промежуточные — поровну между соседями с позициями, и позиции не
/// убывают.
pub(crate) fn place_stops(
    raw: Vec<(crate::style::values::value::Color, Option<f32>)>,
) -> Vec<(crate::style::values::value::Color, f32)> {
    let last = raw.len() - 1;
    let mut out: Vec<(crate::style::values::value::Color, f32)> = Vec::with_capacity(raw.len());
    // Зажим снизу — только позицией ПРЕДШЕСТВЕННИКА (css-images-3 §3.5.3):
    // у первого стопа его нет, и отрицательная позиция законна. Нулевой
    // пол сдвигал `calc(-65535000px)` в ноль, и вся коробка красилась
    // первым цветом (`gradient-eval-*`).
    let mut floor = f32::NEG_INFINITY;
    for (i, (colour, pos)) in raw.iter().enumerate() {
        let at = match pos {
            Some(v) => v.max(floor),
            None if i == 0 => 0.0,
            None if i == last => 1.0f32.max(floor),
            None => {
                // Доля до следующего стопа с позицией (или до конца).
                let (mut next, mut steps) = (1.0f32, (last - i + 1) as f32);
                for (j, (_, p)) in raw.iter().enumerate().skip(i + 1) {
                    if let Some(v) = p {
                        next = v.max(floor);
                        steps = (j - i + 1) as f32;
                        break;
                    }
                }
                floor + (next - floor) / steps
            }
        };
        floor = at;
        out.push((*colour, at));
    }
    out
}
