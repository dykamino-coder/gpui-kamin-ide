//! Разбор списка дорожек grid-template-*: размеры, minmax(), fit-content(), repeat(), счёт дорожек.

use super::*;
use crate::values::ValueCx;

/// Разбор списка дорожек `grid-template-*` в `TrackSize`.
/// Keep percentages and mixed lengths until the grid dimensions are known.
pub(in crate::style::computed) fn parse_tracks(
    v: &str,
    vcx: ValueCx<'_>,
) -> Option<Vec<TrackSize>> {
    fn single(t: &str, vcx: ValueCx<'_>) -> Option<Track> {
        let t = t.trim();
        // Именованная линия перед дорожкой: `[side] 240px`. Имя не несёт
        // размера, поэтому просто отбрасывается — но НЕ вместе с дорожкой.
        let t = t.trim_start_matches('[');
        let t = match t.find(']') {
            Some(at) => t[at + 1..].trim(),
            None => t,
        };
        if t == "auto" {
            return Some(Track::Auto);
        }
        if t == "min-content" {
            return Some(Track::MinContent);
        }
        if t == "max-content" {
            return Some(Track::MaxContent);
        }

        if let Some(fr) = t.strip_suffix("fr") {
            let value: f32 = fr.trim().parse().ok()?;
            return (value.is_finite() && value >= 0.0).then_some(Track::Fr(value));
        }
        match Len::parse(t, vcx) {
            Some(Len::Px(px)) => Some(Track::Px(px)),
            Some(Len::Pct(p)) => Some(Track::Pct(p)),
            // Единицы шрифта откладываются: раньше они роняли разбор, а с
            // ним и ВЕСЬ список дорожек — сетка выходила из равных долей.
            Some(
                l @ (Len::Em(_)
                | Len::EmPx(..)
                | Len::Ch(_)
                | Len::Ex(_)
                | Len::Ic(_)
                | Len::Lh(_)
                | Len::LhPx(..)
                | Len::Vw(_)
                | Len::Vh(_)
                | Len::Calc(_)
                | Len::Math(_)),
            ) => Some(Track::Font(l)),
            _ => None,
        }
    }

    /// Одна запись списка: `minmax(a, b)` либо одиночная дорожка.
    fn one(t: &str, vcx: ValueCx<'_>) -> Option<TrackSize> {
        let t = t.trim();
        if let Some(inner) = t.strip_prefix("minmax(").and_then(|s| s.strip_suffix(')')) {
            let args = crate::style::css::split_args(inner);
            let [lo, hi] = args.as_slice() else {
                return None;
            };
            let lo = single(lo, vcx)?;
            if matches!(lo, Track::Fr(_)) {
                return None;
            }
            return Some(TrackSize::MinMax(lo, single(hi, vcx)?));
        }
        // `fit-content(N)` — дорожка по содержимому, но НЕ ШИРЕ N: раньше
        // сводилась к `Px(N)`, и потолок работал полом
        // (column-intrinsic-maximums).
        // Аргумент — `<length-percentage>` (css-grid-1 §7.2.3), а не только
        // точки: `fit-content(30%)` не разбирался и ронял `parse_tracks` на
        // `None` для ВСЕГО списка (`out.push(one(&token)?)` ниже), после чего
        // сетка сводилась к равным колонкам по `count_tracks`. Верхняя грань
        // теперь идёт через `single`, который знает px, проценты и единицы
        // Reject an invalid argument together with the whole declaration.
        if let Some(inner) = t
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let hi = match single(inner, vcx)? {
                Track::Px(v) => Track::FitPx(v),
                Track::Pct(v) => Track::FitPct(v),
                Track::Font(v) => Track::FitFont(v),
                _ => return None,
            };
            return Some(TrackSize::MinMax(Track::Auto, hi));
        }
        single(t, vcx).map(TrackSize::Single)
    }

    // Список дорожек с раскрытием `repeat(N, …)` НА МЕСТЕ. Раньше `repeat`
    // понимался только когда занимал весь список: запись `200px repeat(2, 1fr)`
    // теряла повтор целиком, и вместо трёх колонок оставалась одна — молча,
    // потому что нераспознанное просто отсеивалось.
    let mut out: Vec<TrackSize> = vec![];
    for token in tokenize_tracks(v) {
        if let Some(inner) = token
            .strip_prefix("repeat(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let (count, rest) = inner.split_once(',')?;
            let count = count.trim();
            let unit: Vec<TrackSize> = tokenize_tracks(rest)
                .iter()
                .filter(|t| !(t.starts_with('[') && t.ends_with(']')))
                .map(|t| one(t, vcx))
                .collect::<Option<_>>()?;
            if unit.is_empty() {
                return None;
            }
            // Число повторов бывает не числом: `auto-fill` и `auto-fit`
            // считает раскладка — ей известна ширина контейнера.
            if count.eq_ignore_ascii_case("auto-fill") || count.eq_ignore_ascii_case("auto-fit") {
                out.push(TrackSize::AutoRepeat {
                    fit: count.eq_ignore_ascii_case("auto-fit"),
                    tracks: unit,
                });
                continue;
            }
            let count: usize = count.parse().ok()?;
            if count == 0 {
                return None;
            }
            for _ in 0..count.min(64) {
                out.extend(unit.iter().cloned());
            }
            continue;
        }
        // Отдельный токен имени линии — не дорожка: `[a] 50px [b] 40px [c]`
        // задаёт ДВЕ дорожки и три имени. Раньше такой токен ронял весь список
        // (`one` возвращал `None`), и сетка схлопывалась в одну колонку —
        // молча, включая ЭТАЛОНЫ (`subgrid/line-names-002-ref` рисовался
        // пустым).
        let bare = token.trim();
        if bare.starts_with('[') && bare.ends_with(']') {
            continue;
        }
        // Нераспознанная дорожка — повод отказаться от всего списка: тихо
        // укоротить его значит переставить всех детей.
        out.push(one(&token, vcx)?);
    }
    (!out.is_empty()).then_some(out)
}

/// CSS Grid 2 §7.2.2: a bracketed line-name group is one component, even
/// with multiple names or without whitespace before the adjacent track.
/// Use the same boundaries for sizes and names so their positions agree.
pub(super) fn tokenize_tracks(v: &str) -> Vec<String> {
    line_name_tokens(v)
}

/// Число колонок в `grid-template-columns`: и `repeat(3, 1fr)`, и `1fr 1fr`.
pub(in crate::style::computed) fn count_tracks(v: &str) -> Option<u16> {
    if let Some(inner) = v.strip_prefix("repeat(").and_then(|s| s.strip_suffix(')')) {
        return inner.split(',').next()?.trim().parse().ok();
    }
    let n = v.split_whitespace().count();
    (n > 0).then_some(n as u16)
}
