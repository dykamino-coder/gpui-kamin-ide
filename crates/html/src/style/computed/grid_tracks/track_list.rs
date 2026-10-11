//! Разбор списка дорожек grid-template-*: размеры, minmax(), fit-content(), repeat(), счёт дорожек.

use super::*;

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v197, `scout-gridoof-2026-09.md` жила И1,
/// 8 хунков): `fit-content(N)` теряет функцию дорожки в переводе CSS→gpui и
/// доезжает как `minmax(auto, N)` — ЗАКРЕПЛЕНИЕ вместо ПОТОЛКА; патч заводил
/// грани `Track::FitPx/FitPct`, вариант `GridTrack::FitContent` в
/// `vendor/gpui/src/geometry.rs` и ветки в `vendor/gpui/src/taffy.rs`.
/// Обещание +0…+2. Срез 1481 пара: **+27/−337**. Падение не логическое —
/// целые семейства ушли в «красное видно»: `block-aspect-ratio-*`,
/// `flex-aspect-ratio-*`, `grid-aspect-ratio-*`, `intrinsic-size-*`,
/// `multicol-rule-*`, таблицы, `hanging-punctuation-*`, `boundary-shaping-*`.
/// Новый вариант перечисления в вендорном `GridTrack` меняет раскладку далеко
/// за пределами сетки: под него идут ВСЕ дорожечные размеры gpui. Возвращать
/// только вместе с полным перебором потребителей `GridTrack` и своим сводом.
pub(in crate::style::computed) fn parse_tracks(v: &str) -> Option<Vec<TrackSize>> {
    fn single(t: &str) -> Option<Track> {
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
            return fr.trim().parse().ok().map(Track::Fr);
        }
        match Len::parse(t) {
            Some(Len::Px(px)) => Some(Track::Px(px)),
            Some(Len::Pct(p)) => Some(Track::Pct(p)),
            // Единицы шрифта откладываются: раньше они роняли разбор, а с
            // ним и ВЕСЬ список дорожек — сетка выходила из равных долей.
            Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_) | Len::Ic(_))) => Some(Track::Font(l)),
            _ => None,
        }
    }

    /// Одна запись списка: `minmax(a, b)` либо одиночная дорожка.
    fn one(t: &str) -> Option<TrackSize> {
        let t = t.trim();
        if let Some(inner) = t.strip_prefix("minmax(").and_then(|s| s.strip_suffix(')')) {
            let (lo, hi) = inner.split_once(',')?;
            return Some(TrackSize::MinMax(single(lo)?, single(hi)?));
        }
        // `fit-content(N)` — дорожка по содержимому, но НЕ ШИРЕ N: раньше
        // сводилась к `Px(N)`, и потолок работал полом
        // (column-intrinsic-maximums).
        // Аргумент — `<length-percentage>` (css-grid-1 §7.2.3), а не только
        // точки: `fit-content(30%)` не разбирался и ронял `parse_tracks` на
        // `None` для ВСЕГО списка (`out.push(one(&token)?)` ниже), после чего
        // сетка сводилась к равным колонкам по `count_tracks`. Верхняя грань
        // теперь идёт через `single`, который знает px, проценты и единицы
        // шрифта; нераспознанный аргумент — это `auto`, а не потеря шаблона.
        if let Some(inner) = t
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let hi = match single(inner).unwrap_or(Track::Auto) {
                Track::Px(v) => Track::FitPx(v),
                Track::Pct(v) => Track::FitPct(v),
                other => other,
            };
            return Some(TrackSize::MinMax(Track::Auto, hi));
        }
        single(t).map(TrackSize::Single)
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
                .filter_map(|t| one(t))
                .collect();
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
        out.push(one(&token)?);
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
