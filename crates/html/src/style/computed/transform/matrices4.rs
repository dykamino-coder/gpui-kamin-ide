//! Методы Transform: накопление 2D/3D-составляющих (push*), матрицы 4×4 сдвига, масштаба,
//! перспективы и поворота.

use super::*;

impl Transform {
    /// Поворот вокруг произвольной оси, СПЛЮЩЕННЫЙ на плоскость экрана.
    ///
    /// Сплющивание (css-transforms-2 §3d-transform-rendering) — это
    /// вычёркивание третьей строки и третьего столбца 4x4-матрицы, поэтому
    /// плоская часть — ровно верхний 2x2 блок матрицы поворота Родрига, а
    /// m33 = z²(1-cos a) + cos a нужен для `backface-visibility`.
    /// Проверка: ось Z даёт обычный поворот, ось Y — diag(cos a, 1), ось X —
    /// diag(1, cos a), то есть ровно `rotateZ`/`rotateY`/`rotateX`.
    pub fn axis_rot(x: f32, y: f32, z: f32, a: f32) -> ([[f32; 2]; 2], f32) {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return ([[1.0, 0.0], [0.0, 1.0]], 1.0);
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        (
            [
                [x * x * k + c, x * y * k - z * s],
                [y * x * k + z * s, y * y * k + c],
            ],
            z * z * k + c,
        )
    }

    /// Дописать плоскую функцию справа в ОБЕ матрицы: `M := M · [l | v]`.
    pub(crate) fn push(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        self.push2(l, v);
        let mut f = IDENTITY4;
        f[0][0] = l[0][0];
        f[0][1] = l[0][1];
        f[1][0] = l[1][0];
        f[1][1] = l[1][1];
        f[0][3] = v[0][0];
        f[1][3] = v[1][0];
        let pct = [
            [v[0][1], v[0][2]],
            [v[1][1], v[1][2]],
            [0.0, 0.0],
            [0.0, 0.0],
        ];
        self.push4(f, pct);
    }

    /// Только плоская 2×3 (`lin`/`tr`) — для объёмных функций, чья
    /// сплющенная тень нужна SVG и клипу.
    // Matrix multiplication uses the same row/column indices in both operands.
    #[allow(clippy::needless_range_loop)]
    pub(crate) fn push2(&mut self, l: [[f32; 2]; 2], v: [[f32; 3]; 2]) {
        let m = self.lin;
        self.lin = [
            [
                m[0][0] * l[0][0] + m[0][1] * l[1][0],
                m[0][0] * l[0][1] + m[0][1] * l[1][1],
            ],
            [
                m[1][0] * l[0][0] + m[1][1] * l[1][0],
                m[1][0] * l[0][1] + m[1][1] * l[1][1],
            ],
        ];
        for i in 0..2 {
            for k in 0..3 {
                self.tr[i][k] += m[i][0] * v[0][k] + m[i][1] * v[1][k];
            }
        }
    }

    /// Дописать 4×4 справа: `M4 := M4 · f`; `pct` — доли размера в столбце
    /// сдвига функции `f` (строки 0..2: доля ширины, доля высоты).
    pub(crate) fn push4(&mut self, f: [[f32; 4]; 4], pct: [[f32; 2]; 4]) {
        let mut fw = [[0.0f32; 4]; 4];
        let mut fh = [[0.0f32; 4]; 4];
        for k in 0..4 {
            fw[k][3] = pct[k][0];
            fh[k][3] = pct[k][1];
        }
        self.push4_full(f, [fw, fh]);
    }

    /// `M4 := M4 · F` с долями размера во ВСЕЙ матрице: `M(w, h) = M +
    /// w·Mw + h·Mh`. Перспектива между сдвигами в процентах
    /// (`translateY(50%) perspective(…) translateY(-50%)`) переносит долю
    /// и в столбец z, и в строку w — один столбец сдвига её терял.
    /// Произведение линеаризуется: `(M·F)_w = Mw·F + M·Fw` (члены w·h и
    /// w² возможны лишь у `matrix3d` с перспективой по x/y — отброшены).
    pub(crate) fn push4_full(&mut self, f: [[f32; 4]; 4], fp: [[[f32; 4]; 4]; 2]) {
        let m = self.m4;
        let mp = self.m4_pct;
        let add = |a: [[f32; 4]; 4], b: [[f32; 4]; 4]| {
            let mut r = a;
            for i in 0..4 {
                for j in 0..4 {
                    r[i][j] += b[i][j];
                }
            }
            r
        };
        self.m4_pct = [0, 1].map(|a| add(mul4(mp[a], f), mul4(m, fp[a])));
        self.m4 = mul4(m, f);
    }

    /// `translate3d(x, y, z)` в точках.
    pub fn translate4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][3] = x;
        m[1][3] = y;
        m[2][3] = z;
        m
    }

    /// `scale3d(x, y, z)`.
    pub fn scale4(x: f32, y: f32, z: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[0][0] = x;
        m[1][1] = y;
        m[2][2] = z;
        m
    }

    /// `perspective(d)`: m34 = −1/d (css-transforms-2 §perspective()); d уже
    /// не меньше 1px — clamp делает вызывающий.
    pub fn perspective4(d: f32) -> [[f32; 4]; 4] {
        let mut m = IDENTITY4;
        m[3][2] = -1.0 / d;
        m
    }

    /// `rotate3d(x, y, z, a)`: R = cos·I + sin·[u]× + (1−cos)·u·uᵀ — верхний
    /// 2×2 блок и m33 ровно те же, что в `axis_rot`.
    pub fn rot4(x: f32, y: f32, z: f32, a: f32) -> [[f32; 4]; 4] {
        let len = (x * x + y * y + z * z).sqrt();
        if len <= 0.0 {
            return IDENTITY4;
        }
        let (x, y, z) = (x / len, y / len, z / len);
        let (c, s) = (a.cos(), a.sin());
        let k = 1.0 - c;
        [
            [x * x * k + c, x * y * k - z * s, x * z * k + y * s, 0.0],
            [y * x * k + z * s, y * y * k + c, y * z * k - x * s, 0.0],
            [z * x * k - y * s, z * y * k + x * s, z * z * k + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}
