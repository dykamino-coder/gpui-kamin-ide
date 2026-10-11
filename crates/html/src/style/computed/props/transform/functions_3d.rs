//! Продолжение transform_function: rotateX/Y, rotate3d, translateZ, perspective,
//! scaleZ, scale3d, translate3d, translate*. Ветви в исходном порядке.

use super::*;

#[allow(clippy::too_many_arguments, unused_variables)]
pub(super) fn transform_function_3d(
    t: &mut Transform,
    name: &str,
    arg: &str,
    nums: &[f32],
    first: f32,
    angle: f32,
    angle_at: &impl Fn(usize) -> f32,
    frac_at: &impl Fn(usize, f32) -> f32,
) {
    match name {
        // Поворот вокруг оси экрана БЕЗ перспективы — это
        // прямая проекция на плоскость, то есть сжатие поперёк
        // оси ровно на косинус угла (css-transforms-2 §11):
        // `rotateX(60deg)` даёт половину высоты. Перспективы у
        // нас нет, и приближением это не является — при
        // `perspective: none` так считает и браузер.
        "rotatex" => {
            t.scale.1 *= angle.cos();
            t.m33 *= angle.cos();
            t.push2(Transform::diag(1.0, angle.cos()), NO_SHIFT);
            t.has_3d = true;
            t.push4(Transform::rot4(1.0, 0.0, 0.0, angle), [[0.0; 2]; 4]);
        }
        "rotatey" => {
            t.scale.0 *= angle.cos();
            t.m33 *= angle.cos();
            t.push2(Transform::diag(angle.cos(), 1.0), NO_SHIFT);
            t.has_3d = true;
            t.push4(Transform::rot4(0.0, 1.0, 0.0, angle), [[0.0; 2]; 4]);
        }
        // Поворот вокруг произвольной оси, сплющенный на
        // плоскость экрана: это ТОЧНО верхний 2x2 блок
        // матрицы Родрига (css-transforms-2
        // §3d-transform-rendering — сплющивание вычёркивает
        // третью строку и третий столбец). Прежняя формула
        // `1 - |y|·(1 - cos a)` теряла внедиагональные члены
        // и для косой оси давала не поворот, а сжатие.
        // Единица угла берётся из САМОГО аргумента: `contains
        // ("rad")` срабатывал на «grad» и читал 100grad как
        // 100 радиан.
        "rotate3d" => {
            let (x, y, z) = (
                nums.first().copied().unwrap_or(0.0),
                nums.get(1).copied().unwrap_or(0.0),
                nums.get(2).copied().unwrap_or(0.0),
            );
            if (x * x + y * y + z * z) > 0.0 {
                let a = angle_at(3);
                let (l, m33) = Transform::axis_rot(x, y, z, a);
                let len = (x * x + y * y + z * z).sqrt();
                t.rotate_rad += a * z / len;
                t.scale.0 *= l[0][0];
                t.scale.1 *= l[1][1];
                t.m33 *= m33;
                // Ось строго Z — обычный `rotate`, остаётся на
                // 2D-пути (css-transform-3d-rotate3d-Z-*).
                if x == 0.0 && y == 0.0 {
                    t.push(l, NO_SHIFT);
                } else {
                    t.push2(l, NO_SHIFT);
                    t.has_3d = true;
                    t.push4(Transform::rot4(x, y, z, a), [[0.0; 2]; 4]);
                }
            }
        }
        // Третья ось видна только через `perspective()` в том
        // же списке — копится в 4×4; `translateZ(0)` остаётся
        // на 2D-пути (GPU-подсказка, не геометрия).
        "translatez" => {
            if first != 0.0 {
                t.has_3d = true;
                t.push4(Transform::translate4(0.0, 0.0, first), [[0.0; 2]; 4]);
            }
        }
        // perspective(d): m34 = −1/d; d < 1px считается 1px
        // (css-transforms-2 §perspective(): «treated as 1px»;
        // perspective-zero, -zero-point-five).
        "perspective" => {
            if !nums.is_empty() {
                t.has_3d = true;
                t.push4(Transform::perspective4(first.max(1.0)), [[0.0; 2]; 4]);
            }
        }
        // Масштаб по Z видом не правит, но участвует в m33
        // (обратная сторона) и вырождает 4×4 при нуле.
        "scalez" => {
            t.m33 *= first;
            if first != 1.0 {
                t.has_3d = true;
                t.push4(Transform::scale4(1.0, 1.0, first), [[0.0; 2]; 4]);
            }
        }
        "scale3d" => {
            let sy = nums.get(1).copied().unwrap_or(1.0);
            let sz = nums.get(2).copied().unwrap_or(1.0);
            t.scale.0 *= first;
            t.scale.1 *= sy;
            t.m33 *= sz;
            if sz == 1.0 {
                t.push(Transform::diag(first, sy), NO_SHIFT);
            } else {
                t.push2(Transform::diag(first, sy), NO_SHIFT);
                t.has_3d = true;
                t.push4(Transform::scale4(first, sy, sz), [[0.0; 2]; 4]);
            }
        }
        "translate3d" => {
            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
            let mut v = NO_SHIFT;
            for (i, dest) in [&mut t.translate.0, &mut t.translate.1]
                .into_iter()
                .enumerate()
            {
                if let Some(raw) = parts.get(i) {
                    let d = raw.trim_end_matches("px").parse::<f32>().unwrap_or(0.0);
                    *dest += d;
                    v[i][0] = d;
                }
            }
            let z = parts
                .get(2)
                .and_then(|raw| raw.trim_end_matches("px").parse::<f32>().ok())
                .unwrap_or(0.0);
            if z == 0.0 {
                t.push(Transform::diag(1.0, 1.0), v);
            } else {
                t.push2(Transform::diag(1.0, 1.0), v);
                t.has_3d = true;
                t.push4(Transform::translate4(v[0][0], v[1][0], z), [[0.0; 2]; 4]);
            }
        }
        // Проценты в сдвиге считаются от СВОЕГО размера —
        // на этом стоит типовое центрирование
        // `translate(-50%, -50%)`. Раньше процент срезался как
        // единица, и элемент уезжал на 50 точек.
        "translate" | "translatex" | "translatey" => {
            let parts: Vec<&str> = arg.split(',').map(str::trim).collect();
            let mut v = NO_SHIFT;
            let mut axis = |i: usize, x: bool| {
                let Some(raw) = parts.get(i) else { return };
                let value = raw
                    .trim_end_matches("px")
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .unwrap_or(0.0);
                let row = if x { 0 } else { 1 };
                let dest = if raw.ends_with('%') {
                    // Доля своей ширины по x, высоты по y.
                    v[row][1 + row] = value / 100.0;
                    if x {
                        &mut t.translate_pct.0
                    } else {
                        &mut t.translate_pct.1
                    }
                } else {
                    v[row][0] = value;
                    if x {
                        &mut t.translate.0
                    } else {
                        &mut t.translate.1
                    }
                };
                *dest += value / if raw.ends_with('%') { 100.0 } else { 1.0 };
            };
            match name {
                "translatex" => axis(0, true),
                "translatey" => axis(0, false),
                _ => {
                    axis(0, true);
                    axis(1, false);
                }
            }
            t.push(Transform::diag(1.0, 1.0), v);
        }
        // Скос выразить нечем: матрица GPUI хранит поворот и
        // масштаб, но не сдвиг осей.
        _ => {}
    }
}
