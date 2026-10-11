//! Одна функция значения `transform` (rotate/scale/skew/matrix…): разбор тела
//! ветви `"transform"` в Computed::apply_transform. Ветви в исходном порядке;
//! 3D-функции и сдвиги — в `functions_3d`.

use super::*;

#[allow(clippy::too_many_arguments, unused_variables)]
pub(super) fn transform_function(
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
        "rotate" | "rotatez" => {
            t.rotate_rad += angle;
            t.push(Transform::rot(angle), NO_SHIFT);
        }
        "scale" => {
            let sx = frac_at(0, 1.0);
            let sy = frac_at(1, sx);
            t.scale.0 *= sx;
            t.scale.1 *= sy;
            t.push(Transform::diag(sx, sy), NO_SHIFT);
        }
        "skew" => {
            let ay = angle_at(1);
            t.skew_rad.0 += angle;
            t.skew_rad.1 += ay;
            t.push([[1.0, angle.tan()], [ay.tan(), 1.0]], NO_SHIFT);
        }
        // matrix3d(...16): 4x4 в СТОЛБЦОВОМ порядке
        // (css-transforms-2 §matrix3d). Сплющивание —
        // вычёркивание третьей строки и третьего столбца,
        // то есть ровно matrix(m11 m12 m21 m22 m41 m42) =
        // аргументы 1,2,5,6,13,14. Раньше вызов не подходил
        // под `nums.len() == 6` и молча пропадал целиком.
        "matrix3d" if nums.len() == 16 => {
            let (a, b, c2, d) = (nums[0], nums[1], nums[4], nums[5]);
            let (e2, f2) = (nums[12], nums[13]);
            t.translate.0 += e2;
            t.translate.1 += f2;
            t.m33 *= nums[10];
            // Плоское вложение (третья строка/столбец и строка w
            // единичные) остаётся на 2D-пути; иначе — полная
            // 4×4 в СТОЛБЦОВОМ порядке аргументов:
            // m4[строка][столбец] = nums[столбец·4 + строка]
            // (m14/m24 = nums[3]/nums[7] — перспективная
            // строка, transform3d-matrix3d-003/-004; m44 =
            // nums[15] ≠ 1 — деление на w, -005).
            let flat2d = [2usize, 3, 6, 7, 8, 9, 11, 14]
                .iter()
                .all(|&i| nums[i] == 0.0)
                && nums[10] == 1.0
                && nums[15] == 1.0;
            if flat2d {
                t.push([[a, c2], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
            } else {
                t.push2([[a, c2], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
                let mut f = [[0.0f32; 4]; 4];
                for col in 0..4 {
                    for row in 0..4 {
                        f[row][col] = nums[col * 4 + row];
                    }
                }
                t.has_3d = true;
                t.push4(f, [[0.0; 2]; 4]);
            }
        }
        // matrix(a b c d e f): разложение на компоненты
        // (перенос, поворот, масштаб, скос) — QR-подобное,
        // как в css-transforms §16 (декомпозиция).
        "matrix" if nums.len() == 6 => {
            let (a, b, c, d, e2, f2) = (nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]);
            t.translate.0 += e2;
            t.translate.1 += f2;
            t.push([[a, c], [b, d]], [[e2, 0.0, 0.0], [f2, 0.0, 0.0]]);
            let sx = (a * a + b * b).sqrt();
            if sx > 1e-6 {
                t.rotate_rad += b.atan2(a);
                let det = a * d - b * c;
                let sy = det / sx;
                t.scale.0 *= sx;
                t.scale.1 *= sy;
                let shear = (a * c + b * d) / det.max(1e-6);
                t.skew_rad.0 += shear.atan();
            }
        }
        "skewx" => {
            t.skew_rad.0 += angle;
            t.push([[1.0, angle.tan()], [0.0, 1.0]], NO_SHIFT);
        }
        "skewy" => {
            t.skew_rad.1 += angle;
            t.push([[1.0, 0.0], [angle.tan(), 1.0]], NO_SHIFT);
        }
        "scalex" => {
            let k = frac_at(0, 1.0);
            t.scale.0 *= k;
            t.push(Transform::diag(k, 1.0), NO_SHIFT);
        }
        "scaley" => {
            let k = frac_at(0, 1.0);
            t.scale.1 *= k;
            t.push(Transform::diag(1.0, k), NO_SHIFT);
        }
        _ => transform_function_3d(t, name, arg, nums, first, angle, angle_at, frac_at),
    }
}
