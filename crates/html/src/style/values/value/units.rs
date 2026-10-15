//! Единицы окна `vmin`/`vmax`/`vi`/`vb` с приставками `s`/`l`/`d` (css-values-4 §6.1.2) и единицы контейнера `cq*` (css-conditional-5 §container-lengths): разбор и свёртка в точки.
//!
//! Своего варианта `Len` у них нет: `Len` разбирается полусотней `match` по
//! крейту, поэтому единица доживает слагаемым `Sum::ext` в арене `calc` и
//! сворачивается там же, где `vw`/`vh` (`Computed::resolve_viewport`).
//!
//! Малое, большое и динамическое окна у нас совпадают: интерфейса браузера,
//! который прячется при прокрутке, нет, а спецификация в этом случае
//! разрешает равенство («the large and small viewport sizes are
//! equivalent»). Поэтому `svh`/`lvh`/`dvh` — тот же `vh`.

use super::{Len, Sum, calc_store, css_number};

/// Номер единицы в `Sum::ext`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ext {
    Vmin,
    Vmax,
    Vi,
    Vb,
    Cqw,
    Cqh,
    Cqi,
    Cqb,
    Cqmin,
    Cqmax,
}

pub const EXT_N: usize = 10;

/// Предел длины верхнего уровня: так Blink зажимает бесконечность
/// (LayoutUnit, 26 бит целой части), css-values-4 §10.13 «clamped to the
/// range of values the property allows».
const LIMIT: f32 = 33_554_428.0;

/// `<число><единица окна или контейнера>`; `None` — это не такая единица.
pub(super) fn parse_unit(s: &str) -> Option<Len> {
    const VIEW: [(&str, Option<Ext>, bool); 6] = [
        ("vmin", Some(Ext::Vmin), false),
        ("vmax", Some(Ext::Vmax), false),
        ("vi", Some(Ext::Vi), false),
        ("vb", Some(Ext::Vb), false),
        ("vw", None, false),
        ("vh", None, true),
    ];
    for (base, ext, high) in VIEW {
        for pre in ["s", "l", "d", ""] {
            let Some(num) = s
                .strip_suffix(base)
                .and_then(|r| r.strip_suffix(pre))
                .and_then(css_number)
            else {
                continue;
            };
            let k = num / 100.0;
            return Some(match ext {
                Some(e) => ext_len(e, k),
                None if high => Len::Vh(k),
                None => Len::Vw(k),
            });
        }
    }
    for (unit, e) in [
        ("cqmin", Ext::Cqmin),
        ("cqmax", Ext::Cqmax),
        ("cqw", Ext::Cqw),
        ("cqh", Ext::Cqh),
        ("cqi", Ext::Cqi),
        ("cqb", Ext::Cqb),
    ] {
        if let Some(num) = s.strip_suffix(unit).and_then(css_number) {
            return Some(ext_len(e, num / 100.0));
        }
    }
    None
}

fn ext_len(e: Ext, k: f32) -> Len {
    let mut s = Sum::default();
    s.ext[e as usize] = k;
    Len::Calc(calc_store(s))
}

impl Sum {
    /// Коэффициенты всех природ по порядку номеров `nature`.
    pub(super) fn natures(&self) -> [f32; 10 + EXT_N] {
        let mut f = [0.0; 10 + EXT_N];
        f[..10].copy_from_slice(&[
            self.px, self.pct, self.em, self.ch, self.ex, self.ic, self.cap, self.lh, self.vh,
            self.vw,
        ]);
        f[10..].copy_from_slice(&self.ext);
        f
    }

    /// Сумма одной природы — обратное к `nature`.
    pub(super) fn of_nature(i: u8, v: f32) -> Self {
        let mut s = Sum::default();
        match i {
            0 => s.px = v,
            1 => {
                s.pct = v;
                s.has_percentage = true;
            }
            2 => s.em = v,
            3 => s.ch = v,
            4 => s.ex = v,
            5 => s.ic = v,
            6 => s.cap = v,
            7 => s.lh = v,
            8 => s.vh = v,
            9 => s.vw = v,
            n => s.ext[usize::from(n) - 10] = v,
        }
        s
    }

    /// Верхний уровень выражения: NaN становится нулём, бесконечность —
    /// пределом (css-values-4 §10.13).
    pub(super) fn censored(self) -> Self {
        let c = |v: f32| {
            if v.is_nan() {
                0.0
            } else {
                v.clamp(-LIMIT, LIMIT)
            }
        };
        Sum {
            lh: c(self.lh),
            px: c(self.px),
            pct: c(self.pct),
            em: c(self.em),
            ch: c(self.ch),
            ex: c(self.ex),
            ic: c(self.ic),
            cap: c(self.cap),
            vh: c(self.vh),
            vw: c(self.vw),
            ext: self.ext.map(c),
            ..self
        }
    }

    /// Свернуть единицы окна и контейнера в точки. `viewport` — уже без
    /// полос прокрутки; `container` — содержимое ближайшего контейнера
    /// запросов размера, а без него — малое окно (css-conditional-5: «If no
    /// eligible query container can be found, then use the small viewport
    /// size»). Оси `vi`/`vb`/`cqi`/`cqb` — по письму самой коробки.
    /// `false` — сворачивать было нечего.
    pub fn fold_viewport(
        &mut self,
        viewport: (f32, f32),
        vertical: bool,
        container: Option<(f32, f32)>,
    ) -> bool {
        if self.vw == 0.0 && self.vh == 0.0 && self.ext.iter().all(|v| *v == 0.0) {
            return false;
        }
        let (w, h) = viewport;
        let (cw, ch) = container.unwrap_or(viewport);
        let axis = |a: f32, b: f32| if vertical { (b, a) } else { (a, b) };
        let (vi, vb) = axis(w, h);
        let (ci, cb) = axis(cw, ch);
        let bases = [
            w.min(h),
            w.max(h),
            vi,
            vb,
            cw,
            ch,
            ci,
            cb,
            cw.min(ch),
            cw.max(ch),
        ];
        self.px +=
            self.vw * w + self.vh * h + self.ext.iter().zip(bases).map(|(k, b)| k * b).sum::<f32>();
        self.vw = 0.0;
        self.vh = 0.0;
        self.ext = [0.0; EXT_N];
        true
    }
}

impl Len {
    /// Сумма с единицами окна или контейнера, которые свернёт только
    /// `Computed::resolve_viewport`. Потребители на отрисовке (позиция и
    /// радиусы градиента) базы не знают — для них такая длина как прежде
    /// не разобрана, а не молча равна нулю.
    pub fn needs_unit_context(self) -> bool {
        matches!(self, Len::Calc(i) if super::calc_get(i).ext.iter().any(|v| *v != 0.0))
    }
}
