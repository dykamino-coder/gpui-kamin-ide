//! Дорожки сетки: имена линий, размещение, repeat(auto-fill), разбор дорожек.

use crate::style::computed::*;
use crate::style::values::value::Len;

/// Токены записи шаблона для имён линий: `[имена]`, `функция(…)`, слова.
fn line_name_tokens(v: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut paren, mut bracket) = (0i32, 0i32);
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        cur.clear();
    };
    for ch in v.chars() {
        match ch {
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            '[' if paren == 0 => {
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
                bracket += 1;
                cur.push(ch);
            }
            ']' if paren == 0 => {
                bracket -= 1;
                cur.push(ch);
                if bracket == 0 {
                    flush(&mut cur, &mut out);
                }
            }
            c if c.is_whitespace() && paren == 0 && bracket == 0 => flush(&mut cur, &mut out),
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// `[a b]` → `["a", "b"]`.
fn bracket_names(t: &str) -> Option<Vec<String>> {
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.split_whitespace().map(str::to_string).collect())
}

/// Имена линий тела `repeat(…)`: у шаблона — по линиям вокруг дорожек тела
/// (дорожек + 1), у `<line-name-list>` подсетки — по записи на линию.
fn repeat_body_names(rest: &str, subgrid: bool) -> Vec<Vec<String>> {
    let mut lines: Vec<Vec<String>> = if subgrid { Vec::new() } else { vec![Vec::new()] };
    for t in line_name_tokens(rest) {
        match bracket_names(&t) {
            Some(names) if subgrid => lines.push(names),
            Some(names) => {
                if let Some(last) = lines.last_mut() {
                    last.extend(names);
                }
            }
            None if !subgrid => lines.push(Vec::new()),
            None => {}
        }
    }
    lines
}

/// Имена линий записи `grid-template-*` (css-grid-2 §7.2.2 `<line-names>`,
/// §subgrid-listing `<line-name-list>`): по списку имён на линию между
/// КОМПОНЕНТАМИ шаблона — `repeat(N, …)` раскрыт на месте, как в
/// `parse_tracks`, а `repeat(auto-fill|auto-fit, …)` остаётся одним
/// компонентом со своими именами (`repeat`). `None` — имён нет.
pub(super) fn parse_line_names(v: &str) -> Option<gpui::GridAxisLineNames> {
    let tokens = line_name_tokens(v);
    let subgrid = tokens.first().is_some_and(|t| t.eq_ignore_ascii_case("subgrid"));
    let mut out = gpui::GridAxisLineNames::default();
    let mut lines: Vec<Vec<String>> = if subgrid { Vec::new() } else { vec![Vec::new()] };
    let mut any = false;
    let mut after = false;
    for t in tokens.iter().skip(usize::from(subgrid)) {
        if let Some(names) = bracket_names(t) {
            any |= !names.is_empty();
            if subgrid {
                lines.push(names);
            } else if let Some(last) = lines.last_mut() {
                last.extend(names);
            }
            continue;
        }
        if let Some(inner) = t.strip_prefix("repeat(").and_then(|r| r.strip_suffix(')')) {
            let (count, rest) = inner.split_once(',')?;
            let body = repeat_body_names(rest, subgrid);
            any |= body.iter().any(|b| !b.is_empty());
            let count = count.trim();
            if count.eq_ignore_ascii_case("auto-fill") || count.eq_ignore_ascii_case("auto-fit") {
                if after {
                    return None;
                }
                out.before = std::mem::take(&mut lines);
                out.repeat = Some(body);
                lines = if subgrid { Vec::new() } else { vec![Vec::new()] };
                after = true;
            } else {
                let n: usize = count.parse().ok()?;
                for _ in 0..n.min(64) {
                    if subgrid {
                        lines.extend(body.iter().cloned());
                    } else {
                        if let (Some(last), Some(first)) = (lines.last_mut(), body.first()) {
                            last.extend(first.iter().cloned());
                        }
                        lines.extend(body.iter().skip(1).cloned());
                    }
                }
            }
            continue;
        }
        if !subgrid {
            lines.push(Vec::new());
        }
    }
    if !any {
        return None;
    }
    if after {
        out.after = lines;
    } else {
        out.before = lines;
    }
    Some(out)
}

/// Грань размещения по имени линии (css-grid-2 §8.3): `a`, `a 2`, `-1 a`,
/// `span a`, `span 2 a`. Без имени — `None` (числовую грань разбирает
/// `parse_placement`).
pub(super) fn parse_named_placement(v: &str) -> Option<gpui::GridNamedLine> {
    let (mut span, mut num, mut name) = (false, None::<i16>, None::<String>);
    for t in v.split_whitespace() {
        if t.eq_ignore_ascii_case("span") {
            span = true;
        } else if let Ok(n) = t.parse::<i16>() {
            num = Some(n);
        } else if t.eq_ignore_ascii_case("auto") {
            return None;
        } else {
            name = Some(t.to_string());
        }
    }
    let name = name?;
    Some(if span {
        gpui::GridNamedLine::Span(name, num.unwrap_or(1).max(1) as u16)
    } else {
        gpui::GridNamedLine::Line(name, num.unwrap_or(0))
    })
}

/// Обе грани `grid-column`/`grid-row`: при одном значении-имени конец — то
/// же имя («if the first value is a <custom-ident>, the grid-row-end/
/// grid-column-end longhand is also set to that <custom-ident>», §8.4).
pub(super) fn parse_named_pair(v: &str) -> [Option<gpui::GridNamedLine>; 2] {
    match v.split_once('/') {
        Some((a, b)) => [parse_named_placement(a), parse_named_placement(b)],
        None => {
            let start = parse_named_placement(v);
            let end = match &start {
                Some(gpui::GridNamedLine::Line(name, 0)) => Some(gpui::GridNamedLine::Line(name.clone(), 0)),
                _ => None,
            };
            [start, end]
        }
    }
}

pub(super) fn parse_placement(v: &str) -> Placement {
    let v = v.trim();
    if let Some(n) = v.strip_prefix("span") {
        return n
            .trim()
            .parse()
            .map(Placement::Span)
            .unwrap_or(Placement::Auto);
    }
    v.parse().map(Placement::Line).unwrap_or(Placement::Auto)
}

/// `grid-column: 1 / 3` либо `span 2`.
pub(super) fn parse_span(v: &str) -> Option<(Placement, Placement)> {
    match v.split_once('/') {
        Some((a, b)) => Some((parse_placement(a), parse_placement(b))),
        None => Some((parse_placement(v), Placement::Auto)),
    }
}

/// Одна дорожка сетки в терминах CSS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Px(f32),
    Fr(f32),
    /// Доля ШИРИНЫ СЕТКИ (`25%`) — не путать с долей остатка (`fr`).
    Pct(f32),
    /// Длина в единицах ШРИФТА (`2ch`, `1em`, `8rem`): кегль и метрики на
    /// разборе ещё неизвестны, величина считается вместе с прочими `em`.
    Font(Len),
    Auto,
    MinContent,
    MaxContent,
    /// Предел `fit-content(N)` — только как ВЕРХНЯЯ грань `minmax(auto, …)`:
    /// дорожка по содержимому, зажатая N (css-grid-2 §7.2.4,
    /// `fit-content( <length-percentage> )`), а не фиксированный максимум.
    FitPx(f32),
    FitPct(f32),
}

impl Track {
    /// Перевести отложенную длину в точки: единицы шрифта известны только
    /// после разрешения кегля узла.
    pub(crate) fn resolve_font_one(&mut self, family: &str, size_px: f32) {
        if let Track::Font(l) = *self {
            *self = Track::Px(crate::text::metrics::spacing_px(Some(l), family, size_px));
        }
    }
}

impl TrackSize {
    /// То же для обеих граней записи.
    pub(crate) fn resolve_font(&mut self, family: &str, size_px: f32) {
        match self {
            TrackSize::Single(t) => t.resolve_font_one(family, size_px),
            TrackSize::MinMax(a, b) => {
                a.resolve_font_one(family, size_px);
                b.resolve_font_one(family, size_px);
            }
            TrackSize::AutoRepeat { tracks, .. } => {
                for t in tracks.iter_mut() {
                    t.resolve_font(family, size_px);
                }
            }
        }
    }
}

/// Дорожка целиком: одиночная либо пара граней `minmax(a, b)`.
///
/// Обе грани нужны по-настоящему: `minmax(120px, 1fr)` — это «не уже 120, а
/// дальше забирай остаток». Сведение к одной грани меняет ширину колонки.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackSize {
    Single(Track),
    MinMax(Track, Track),
    /// `repeat(auto-fill | auto-fit, …)` — сколько дорожек влезет; при
    /// `fit` пустые схлопываются (css-grid-2 §auto-repeat). Считает это
    /// раскладка: на разборе ширины контейнера ещё нет.
    AutoRepeat {
        fit: bool,
        tracks: Vec<TrackSize>,
    },
}

/// Разрезать короткую запись сетки по косой черте ВНЕ скобок.
///
/// `grid: repeat(4, auto) / 1fr` — черта внутри `repeat()` не разделитель, и
/// резать по первой попавшейся нельзя.
pub(super) fn split_slash(v: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return (v[..i].trim(), v[i + 1..].trim()),
            _ => {}
        }
    }
    (v.trim(), "")
}

/// Убрать слово `auto-flow` (и `dense`) из стороны короткой записи сетки.
pub(super) fn strip_auto_flow(v: &str) -> &str {
    v.trim()
        .trim_start_matches("auto-flow")
        .trim()
        .trim_start_matches("dense")
        .trim()
        .trim_end_matches("dense")
        .trim()
        .trim_end_matches("auto-flow")
        .trim()
}

/// Разбор списка дорожек. `repeat(n, X)` разворачивается в n одинаковых;
/// `minmax()` сводится к своей верхней грани — нижняя у нас всегда
/// `min-content`, чего достаточно для разметки документов.
/// Повтор «сколько влезет»: `repeat(auto-fill | auto-fit, <дорожка>)`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AutoRepeat {
    /// `auto-fit` — пустые дорожки схлопываются, остаток делят непустые.
    pub fit: bool,
    /// Размер дорожки в точках; `None` — дорожка по содержимому (`auto`).
    pub track: Option<f32>,
    /// Доля контейнера, если дорожка задана процентом: `repeat(auto-fill,
    /// 25%)` — четыре дорожки в трёхстах точках. В точки её переводит
    /// раскладка: на разборе ширины контейнера ещё нет
    /// (`column-auto-repeat-002`).
    pub track_pct: Option<f32>,
    /// Дорожка ПО СОДЕРЖИМОМУ (`max-content`/`min-content`/`fit-content`):
    /// число повторов задают сами элементы — по дорожке на каждого
    /// (row-auto-repeat-max-content-001).
    pub intrinsic: bool,
    /// Дорожка названа `min-content`: меряется САМЫМ УЗКИМ местом
    /// содержимого, а не самым широким.
    pub intrinsic_min: bool,
    /// Потолок `fit-content(N)`: дорожка по содержимому, но не шире N.
    pub fit_px: Option<f32>,
    /// Максимум `minmax(N, auto)`: дорожка растягивается остатком
    /// (css-grid-2 §12.8 «Stretch auto Tracks»); прежде терялся, и живые
    /// дорожки `auto-fit` оставались минимумом
    /// (`grid-content-distribution-with-collapsed-tracks-004`).
    pub max_auto: bool,
    /// Максимум `minmax(N, k fr)`: доля остатка.
    pub max_fr: Option<f32>,
    /// Сколько дорожек в ТЕЛЕ повтора: `repeat(auto-fill, fit-content(100px)
    /// fit-content(100px))` — две. Число повторов делит место на ВСЁ тело
    /// (css-grid-2 §7.2.3.2; Blink `CalculateAutomaticRepetitions`,
    /// `repeater_size`), а скалярная ветка раскладки видела одну дорожку.
    pub body: usize,
}

/// Тело авто-повтора СПИСКОМ дорожек: `repeat(auto-fill, max-content
/// min-content)` → `[Single(MaxContent), Single(MinContent)]`.
///
/// Нужно, чтобы раскладка считала гипотетический размер КАЖДОЙ записи тела по
/// её собственной функции (css-grid-3 §7.2.1). `parse_tracks` уже знает и
/// `minmax()`, и `fit-content(N)` (последний как `MinMax(Auto, hi)`), поэтому
/// своего разбора здесь нет.
pub(super) fn auto_fill_body_tracks(v: &str) -> Option<Vec<TrackSize>> {
    parse_tracks(v).as_deref().and_then(|l| {
        l.iter().find_map(|t| match t {
            TrackSize::AutoRepeat { tracks, .. } => Some(tracks.clone()),
            _ => None,
        })
    })
}

/// Длина тела авто-повтора в дорожках (1, если тело не разобралось).
pub(super) fn auto_fill_body(v: &str) -> usize {
    parse_tracks(v)
        .as_deref()
        .and_then(|l| {
            l.iter().find_map(|t| match t {
                TrackSize::AutoRepeat { tracks, .. } => Some(tracks.len()),
                _ => None,
            })
        })
        .unwrap_or(1)
        .max(1)
}

/// Максимум `minmax(lo, hi)` в авто-повторе: `auto` или доля `fr`.
pub(super) fn auto_fill_max(v: &str) -> (bool, Option<f32>) {
    let Some(rest) = v.split("minmax(").nth(1) else {
        return (false, None);
    };
    let Some(inner) = rest.find(')').map(|i| &rest[..i]) else {
        return (false, None);
    };
    let hi = inner.split_once(',').map(|x| x.1).unwrap_or("").trim();
    if hi == "auto" {
        return (true, None);
    }
    let fr = hi
        .strip_suffix("fr")
        .and_then(|k| k.trim().parse::<f32>().ok());
    (false, fr)
}

/// Размер повторяемой дорожки в `repeat(auto-fill | auto-fit, …)`.
///
/// Берётся либо нижняя граница `minmax(N, …)`, либо сама дорожка, если она
/// задана точкой: `repeat(auto-fill, 100px)` — три колонки в трёхстах точках.
/// Все дорожки тела повтора `repeat(auto-fill | auto-fit, …)` в точках.
///
/// Тело бывает из нескольких дорожек — `repeat(auto-fill, 50px 50px)`
/// повторяет ПАРУ. Пока бралась одна, `Len::parse("50px 50px")` не разбирался
/// вовсе, и сетка не получала дорожек: `grid-auto-repeat-multiple-values-*`
/// рисовались одной плитой во всю ширину. Имена линий (`[all x v]`) к размеру
/// не относятся и выбрасываются.
///
/// Пусто, если тело из одной дорожки или хоть один кусок не разобрался: такой
/// случай ведёт прежняя ветка по `auto_fill_min`.
pub(super) fn auto_fill_tracks(v: &str) -> Vec<f32> {
    if v.contains("minmax(") || v.contains("fit-content(") {
        return Vec::new();
    }
    let Some(rest) = v.split("repeat(").nth(1) else {
        return Vec::new();
    };
    let Some(end) = rest.rfind(')') else {
        return Vec::new();
    };
    let Some(body) = rest[..end].split_once(',').map(|(_, b)| b) else {
        return Vec::new();
    };
    // Имена линий в квадратных скобках размера не несут.
    let mut clean = String::with_capacity(body.len());
    let mut depth = 0usize;
    for ch in body.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => clean.push(ch),
            _ => {}
        }
    }
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(parts.len());
    for t in parts {
        match Len::parse(t) {
            Some(Len::Px(px)) => out.push(px),
            _ => return Vec::new(),
        }
    }
    out
}

pub(super) fn auto_fill_min(v: &str) -> Option<f32> {
    let px_of = |t: &str| match Len::parse(t.trim()) {
        Some(Len::Px(px)) => Some(px),
        _ => None,
    };
    if let Some(rest) = v.split("minmax(").nth(1)
        && let Some(inner) = rest.find(')').map(|i| &rest[..i])
    {
        let mut args = inner.splitn(2, ',');
        let lo = args.next().unwrap_or("");
        let hi = args.next().unwrap_or("");
        // Число повторов считается по МАКСИМАЛЬНОЙ функции дорожки, если
        // она определённая, иначе по минимальной (css-grid-1 §7.2.3.2):
        // `minmax(min-content, 100px)` повторяется сотнями точек, а
        // `minmax(100px, 1fr)` — сотней из минимума.
        // Максимум ниже минимума поднимается до него (css-grid-2 §7.2.3.1:
        // «the max will be floored by the min» — `grid-auto-repeat-minmax`).
        return match (px_of(hi), px_of(lo)) {
            (Some(h), Some(l)) => Some(h.max(l)),
            (h, l) => h.or(l),
        };
    }
    let rest = v.split("repeat(").nth(1)?;
    let inner = &rest[..rest.rfind(')')?];
    px_of(inner.split(',').nth(1)?)
}

/// Потолок `fit-content(N)` в авто-повторе: дорожка не шире N.
pub(super) fn auto_fill_fit_px(v: &str) -> Option<f32> {
    let rest = v.split("fit-content(").nth(1)?;
    let inner = &rest[..rest.find(')')?];
    match Len::parse(inner.trim()) {
        Some(Len::Px(px)) => Some(px),
        _ => None,
    }
}

/// Голая интрин-дорожка СНАРУЖИ `repeat(auto-fill | auto-fit, …)`.
///
/// css-grid-1 `<auto-track-list>` разрешает вокруг авто-повтора только
/// `<fixed-size>`; css-grid-3 §7.2.1 ослабила запись лишь ВНУТРИ `repeat()`.
/// `minmax()` снаружи законен в обе стороны (`minmax(<fixed-breadth>,
/// <track-breadth>)` и `minmax(<inflexible-breadth>, <fixed-breadth>)`) и сюда
/// НЕ попадает: иначе под нож ушли бы валидные
/// `css-grid/grid-definition/grid-auto-fill-columns-001` и родня, а также
/// `grid-lanes/invalidation/grid-lanes-change-intrinsic-size-with-auto-repeat-tracks-001`
/// (`repeat(auto-fill, 20px) minmax(min-content, 40px)`).
/// Имена линий в скобках размера не несут и негодности не создают.
pub(super) fn auto_repeat_outside_intrinsic(v: &str) -> bool {
    let toks = tokenize_tracks(v);
    if !toks
        .iter()
        .any(|t| t.starts_with("repeat(") && (t.contains("auto-fill") || t.contains("auto-fit")))
    {
        return false;
    }
    toks.iter().any(|t| {
        let t = t.trim();
        if t.starts_with("repeat(") || (t.starts_with('[') && t.ends_with(']')) {
            return false;
        }
        t == "auto" || t == "min-content" || t == "max-content" || t.starts_with("fit-content(")
    })
}

/// Дорожка повтора задана ПО СОДЕРЖИМОМУ: `repeat(auto-fill, max-content)`
/// и родня. Точечного размера у неё нет — повторы считают сами элементы.
pub(super) fn auto_fill_intrinsic(v: &str) -> bool {
    let Some(rest) = v.split("repeat(").nth(1) else {
        return false;
    };
    let inner = match rest.rfind(')') {
        Some(i) => &rest[..i],
        None => rest,
    };
    // `auto`-дорожку меряет прежний счёт по самому большому элементу
    // (column-auto-repeat-auto-001) — сюда только content-ключи.
    ["max-content", "min-content", "fit-content"]
        .iter()
        .any(|k| inner.contains(k))
        && auto_fill_min(v).is_none()
        && auto_fill_pct(v).is_none()
}

/// Доля контейнера в `repeat(auto-fill | auto-fit, N%)`.
///
/// Считается там же, где и точечный размер, но остаётся долей: в точки её
/// переводит раскладка, когда ширина контейнера уже решена.
pub(super) fn auto_fill_pct(v: &str) -> Option<f32> {
    let pct_of = |t: &str| match Len::parse(t.trim()) {
        Some(Len::Pct(k)) => Some(k),
        _ => None,
    };
    if let Some(rest) = v.split("minmax(").nth(1)
        && let Some(inner) = rest.find(')').map(|i| &rest[..i])
    {
        let mut args = inner.splitn(2, ',');
        let lo = args.next().unwrap_or("");
        let hi = args.next().unwrap_or("");
        // Как и в точках: максимум сильнее минимума (css-grid-1 §7.2.3.2).
        return pct_of(hi).or_else(|| pct_of(lo));
    }
    let rest = v.split("repeat(").nth(1)?;
    let inner = &rest[..rest.rfind(')')?];
    pct_of(inner.split(',').nth(1)?)
}

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
pub(super) fn parse_tracks(v: &str) -> Option<Vec<TrackSize>> {
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
fn tokenize_tracks(v: &str) -> Vec<String> {
    line_name_tokens(v)
}

/// Число колонок в `grid-template-columns`: и `repeat(3, 1fr)`, и `1fr 1fr`.
pub(super) fn count_tracks(v: &str) -> Option<u16> {
    if let Some(inner) = v.strip_prefix("repeat(").and_then(|s| s.strip_suffix(')')) {
        return inner.split(',').next()?.trim().parse().ok();
    }
    let n = v.split_whitespace().count();
    (n > 0).then_some(n as u16)
}
