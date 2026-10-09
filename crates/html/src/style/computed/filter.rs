//! Filter: функции filter/backdrop-filter.

use crate::style::computed::*;

/// `filter`: цветовое преобразование элемента.
///
/// Размытие поддерева требует отрисовки в отдельную текстуру, которой в GPUI
/// нет; всё остальное — арифметика над цветом, и её можно применить прямо к
/// собственным цветам элемента, не трогая конвейер отрисовки.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Filter {
    /// Доля обесцвечивания.
    pub grayscale: f32,
    /// Множитель яркости.
    pub brightness: f32,
    /// Множитель насыщенности.
    pub saturate: f32,
    /// Доля инверсии.
    pub invert: f32,
    /// Доля сепии.
    pub sepia: f32,
    /// Множитель непрозрачности.
    pub opacity: f32,
    /// Поворот тона в градусах.
    pub hue_rotate: f32,
    /// Множитель контраста.
    pub contrast: f32,
    /// Радиус размытия поддерева в точках: `filter: blur(N)`.
    ///
    /// Цветовые функции считаются по цвету каждого примитива, а размытию
    /// нужна готовая картинка поддерева — поэтому оно живёт отдельно от
    /// остальных полей и включает отрисовку в свой буфер.
    pub blur: f32,
}

impl Filter {
    /// Нейтральный фильтр: ничего не меняет.
    pub fn neutral() -> Filter {
        Filter {
            grayscale: 0.0,
            brightness: 1.0,
            saturate: 1.0,
            invert: 0.0,
            sepia: 0.0,
            opacity: 1.0,
            hue_rotate: 0.0,
            contrast: 1.0,
            blur: 0.0,
        }
    }

    /// Цветовые функции одной аффинной матрицей 4×5 над НЕумноженным RGBA:
    /// строки R, G, B, A по пять чисел (четыре множителя и сдвиг), порядок
    /// функций — как в `apply`. None — матрица единичная (остаётся разве что
    /// размытие). filter-effects-1 §«Supported filter functions»: каждая
    /// цветовая функция — `feColorMatrix`/`feComponentTransfer` с линейной
    /// формулой, их цепочка — произведение матриц.
    pub fn color_matrix(&self) -> Option<[f32; 20]> {
        // Три строки RGB: множители при r, g, b и сдвиг.
        type M = [[f32; 4]; 3];
        const ID: M = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        // `a` поверх `b`: a·(b·x + tb) + ta.
        fn then(a: &M, b: &M) -> M {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let s: f32 = (0..3).map(|k| a[i][k] * b[k][j]).sum();
                    if j == 3 { s + a[i][3] } else { s }
                })
            })
        }
        // (1 − k)·I + k·T — доля `k` пути к матрице `t` (без сдвига).
        fn toward(t: [[f32; 3]; 3], k: f32) -> M {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    if j == 3 {
                        return 0.0;
                    }
                    let id = if i == j { 1.0 } else { 0.0 };
                    id + (t[i][j] - id) * k
                })
            })
        }
        let diag = |k: f32, shift: f32| -> M {
            [
                [k, 0.0, 0.0, shift],
                [0.0, k, 0.0, shift],
                [0.0, 0.0, k, shift],
            ]
        };
        let mut m = ID;
        if self.sepia > 0.0 {
            let t = [
                [0.393, 0.769, 0.189],
                [0.349, 0.686, 0.168],
                [0.272, 0.534, 0.131],
            ];
            m = then(&toward(t, self.sepia.min(1.0)), &m);
        }
        if self.grayscale > 0.0 || self.saturate != 1.0 {
            let l = [0.2126f32, 0.7152, 0.0722];
            m = then(&toward([l, l, l], self.grayscale.min(1.0)), &m);
            // lum + (c − lum)·s = I + (L − I)·(1 − s)
            m = then(&toward([l, l, l], 1.0 - self.saturate), &m);
        }
        if self.invert > 0.0 {
            let k = self.invert.min(1.0);
            m = then(&diag(1.0 - 2.0 * k, k), &m);
        }
        if self.contrast != 1.0 {
            m = then(&diag(self.contrast, 0.5 - 0.5 * self.contrast), &m);
        }
        if self.hue_rotate != 0.0 {
            let a = self.hue_rotate.to_radians();
            let (cos, sin) = (a.cos(), a.sin());
            let h = [
                [
                    0.213 + cos * 0.787 - sin * 0.213,
                    0.715 - cos * 0.715 - sin * 0.715,
                    0.072 - cos * 0.072 + sin * 0.928,
                ],
                [
                    0.213 - cos * 0.213 + sin * 0.143,
                    0.715 + cos * 0.285 + sin * 0.140,
                    0.072 - cos * 0.072 - sin * 0.283,
                ],
                [
                    0.213 - cos * 0.213 - sin * 0.787,
                    0.715 - cos * 0.715 + sin * 0.715,
                    0.072 + cos * 0.928 + sin * 0.072,
                ],
            ];
            m = then(&toward(h, 1.0), &m);
        }
        if self.brightness != 1.0 {
            m = then(&diag(self.brightness, 0.0), &m);
        }
        if m == ID && self.opacity >= 1.0 {
            return None;
        }
        Some([
            m[0][0], m[0][1], m[0][2], 0.0, m[0][3], //
            m[1][0], m[1][1], m[1][2], 0.0, m[1][3], //
            m[2][0], m[2][1], m[2][2], 0.0, m[2][3], //
            0.0, 0.0, 0.0, self.opacity.min(1.0), 0.0,
        ])
    }

    /// Применить к цвету.
    pub fn apply(&self, c: Color) -> Color {
        // Filter Effects 1 §6.1 caps conversion amounts at one when used.
        // Keep the specified value intact for animation interpolation.
        let (mut r, mut g, mut b) = (c.r, c.g, c.b);
        // Порядок как в CSS: функции применяются слева направо, а записаны
        // они у нас в фиксированном порядке — для набора без повторов это то
        // же самое.
        if self.sepia > 0.0 {
            let k = self.sepia.min(1.0);
            let (sr, sg, sb) = (
                0.393 * r + 0.769 * g + 0.189 * b,
                0.349 * r + 0.686 * g + 0.168 * b,
                0.272 * r + 0.534 * g + 0.131 * b,
            );
            r += (sr - r) * k;
            g += (sg - g) * k;
            b += (sb - b) * k;
        }
        if self.grayscale > 0.0 || self.saturate != 1.0 {
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            // Обесцвечивание тянет к яркости, насыщение — от неё.
            let k = self.grayscale.min(1.0);
            r += (lum - r) * k;
            g += (lum - g) * k;
            b += (lum - b) * k;
            let sat = self.saturate;
            r = lum + (r - lum) * sat;
            g = lum + (g - lum) * sat;
            b = lum + (b - lum) * sat;
        }
        if self.invert > 0.0 {
            let k = self.invert.min(1.0);
            r += (1.0 - r - r) * k;
            g += (1.0 - g - g) * k;
            b += (1.0 - b - b) * k;
        }
        if self.contrast != 1.0 {
            // Аффинный контраст: растяжение вокруг середины (filter-effects-1
            // §contrast: c*k + 0.5 - 0.5k).
            let k = self.contrast;
            r = (r - 0.5) * k + 0.5;
            g = (g - 0.5) * k + 0.5;
            b = (b - 0.5) * k + 0.5;
        }
        if self.hue_rotate != 0.0 {
            // Матрица поворота тона из спецификации фильтров.
            let a = self.hue_rotate.to_radians();
            let (cos, sin) = (a.cos(), a.sin());
            let (r0, g0, b0) = (r, g, b);
            r = (0.213 + cos * 0.787 - sin * 0.213) * r0
                + (0.715 - cos * 0.715 - sin * 0.715) * g0
                + (0.072 - cos * 0.072 + sin * 0.928) * b0;
            g = (0.213 - cos * 0.213 + sin * 0.143) * r0
                + (0.715 + cos * 0.285 + sin * 0.140) * g0
                + (0.072 - cos * 0.072 - sin * 0.283) * b0;
            b = (0.213 - cos * 0.213 - sin * 0.787) * r0
                + (0.715 - cos * 0.715 + sin * 0.715) * g0
                + (0.072 + cos * 0.928 + sin * 0.072) * b0;
        }
        if self.brightness != 1.0 {
            r *= self.brightness;
            g *= self.brightness;
            b *= self.brightness;
        }
        Color {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: (c.a * self.opacity.min(1.0)).clamp(0.0, 1.0),
        }
    }
}
