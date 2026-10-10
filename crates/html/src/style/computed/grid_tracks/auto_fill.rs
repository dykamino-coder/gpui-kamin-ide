//! repeat(auto-fill | auto-fit, …): тело повтора, пределы дорожек, число повторов от размера контейнера.

use super::*;

/// Тело авто-повтора СПИСКОМ дорожек: `repeat(auto-fill, max-content
/// min-content)` → `[Single(MaxContent), Single(MinContent)]`.
///
/// Нужно, чтобы раскладка считала гипотетический размер КАЖДОЙ записи тела по
/// её собственной функции (css-grid-3 §7.2.1). `parse_tracks` уже знает и
/// `minmax()`, и `fit-content(N)` (последний как `MinMax(Auto, hi)`), поэтому
/// своего разбора здесь нет.
pub(in crate::style::computed) fn auto_fill_body_tracks(v: &str) -> Option<Vec<TrackSize>> {
    parse_tracks(v).as_deref().and_then(|l| {
        l.iter().find_map(|t| match t {
            TrackSize::AutoRepeat { tracks, .. } => Some(tracks.clone()),
            _ => None,
        })
    })
}

/// Длина тела авто-повтора в дорожках (1, если тело не разобралось).
pub(in crate::style::computed) fn auto_fill_body(v: &str) -> usize {
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
pub(in crate::style::computed) fn auto_fill_max(v: &str) -> (bool, Option<f32>) {
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
pub(in crate::style::computed) fn auto_fill_tracks(v: &str) -> Vec<f32> {
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

pub(in crate::style::computed) fn auto_fill_min(v: &str) -> Option<f32> {
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
pub(in crate::style::computed) fn auto_fill_fit_px(v: &str) -> Option<f32> {
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
pub(in crate::style::computed) fn auto_repeat_outside_intrinsic(v: &str) -> bool {
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
pub(in crate::style::computed) fn auto_fill_intrinsic(v: &str) -> bool {
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
pub(in crate::style::computed) fn auto_fill_pct(v: &str) -> Option<f32> {
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
