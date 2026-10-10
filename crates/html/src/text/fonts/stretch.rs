//! Stretch for fonts; split out to keep the owning module within 250 lines.

use super::FACES;
use super::alias;

/// Ширина из дескриптора `font-stretch` правила `@font-face` — отрезок в
/// процентах (css-fonts-4 §font-stretch-desc). Одно значение даёт вырожденный
/// отрезок, пропуск и негодная запись — `normal`.
pub(super) fn stretch_desc(value: Option<&str>) -> (f32, f32) {
    fn one(word: &str) -> Option<f32> {
        Some(match word.trim().to_ascii_lowercase().as_str() {
            "ultra-condensed" => 50.0,
            "extra-condensed" => 62.5,
            "condensed" => 75.0,
            "semi-condensed" => 87.5,
            "normal" => 100.0,
            "semi-expanded" => 112.5,
            "expanded" => 125.0,
            "extra-expanded" => 150.0,
            "ultra-expanded" => 200.0,
            pct => pct.strip_suffix('%')?.trim().parse::<f32>().ok()?,
        })
    }
    let Some(value) = value else {
        return (100.0, 100.0);
    };
    let mut parts = value.split_whitespace().filter_map(one);
    let Some(lo) = parts.next() else {
        return (100.0, 100.0);
    };
    let hi = parts.next().unwrap_or(lo);
    (lo.min(hi), lo.max(hi))
}

/// Настоящее имя семейства с учётом ЗАПРОШЕННОЙ ширины начертания.
///
/// css-fonts-4 §font-matching, шаг `font-stretch`: при запросе `<= 100%`
/// сперва перебираются лица НЕ ШИРЕ запроса — от ближайшего вниз, — затем
/// более широкие вверх; при запросе `> 100%` наоборот. Пока выбор был «кто
/// объявлен последним», семь пар `font-stretch-12…18` рисовались файлом
/// `fail.woff`, стоящим вторым, вместо `pass.woff`.
///
/// При РАВНОМ расстоянии побеждает объявленное ПОЗЖЕ — ровно то, что делал
/// прежний одиночный слот: семьи, где на одно имя приходится несколько правил
/// с одинаковым (отсутствующим) дескриптором — `unicode-range`,
/// `first-available-font-*` — не двигаются. Семейство без правил `@font-face`
/// уходит в прежнюю подмену.
pub fn alias_stretch(family: &str, want: Option<f32>) -> Option<String> {
    let key = family.to_ascii_lowercase();
    let want = want.unwrap_or(100.0);
    let picked = FACES.with(|f| {
        let faces = f.borrow();
        let list = faces.get(&key)?;
        let rank = |face: &((f32, f32), String)| -> (u8, f32) {
            let (lo, hi) = face.0;
            let v = want.clamp(lo, hi);
            if want <= 100.0 {
                if v <= want {
                    (0, want - v)
                } else {
                    (1, v - want)
                }
            } else if v >= want {
                (0, v - want)
            } else {
                (1, want - v)
            }
        };
        let mut best: Option<(&str, (u8, f32))> = None;
        for face in list {
            let r = rank(face);
            if best.is_none_or(|(_, b)| (r.0, r.1) <= (b.0, b.1)) {
                best = Some((face.1.as_str(), r));
            }
        }
        best.map(|(name, _)| name.to_string())
    });
    picked.or_else(|| alias(family))
}
