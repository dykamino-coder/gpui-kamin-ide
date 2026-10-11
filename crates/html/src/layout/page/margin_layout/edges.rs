//! Размеры и поля марджин-боксов вдоль кромки листа (css-page-3 §margin-dimension).

use super::Pref;

/// Две коробки из трёх (`[первая, не решаемая, вторая]`) делят доступную
/// длину — Blink `ResolveTwoEdgeMarginBoxLengths`. Итог — длины ПО ПОЛЯМ.
pub(super) fn resolve_two(p: [Pref; 3], avail: f32) -> (f32, f32) {
    let mut flex_avail = avail;
    let (mut tmin, mut tmax) = (0.0f32, 0.0f32);
    for x in &p {
        if x.auto {
            tmin += x.min_len();
            tmax += x.max_len();
        } else {
            flex_avail -= x.max_len();
        }
    }
    let (space, unflexed, factors): (f32, [f32; 3], [f32; 3]) = if flex_avail > tmax {
        let u = [p[0].max_len(), p[1].max_len(), p[2].max_len()];
        (flex_avail - tmax, u, u)
    } else {
        let u = [p[0].min_len(), p[1].min_len(), p[2].min_len()];
        let space = flex_avail - tmin;
        let f = if space > 0.0 {
            [
                p[0].max_len() - p[0].min_len(),
                p[1].max_len() - p[1].min_len(),
                p[2].max_len() - p[2].min_len(),
            ]
        } else {
            u
        };
        (space, u, f)
    };
    let mut first = unflexed[0];
    let mut second = unflexed[2];
    if p[0].auto {
        if p[2].auto {
            let total = factors[0] + factors[2];
            if total > 0.0 {
                first += space * factors[0] / total;
            }
        } else {
            first = avail - second;
        }
    }
    if p[2].auto {
        second = avail - first;
    }
    (first, second)
}

/// Длины трёх коробок стороны по главной оси (border box; `None` — коробки
/// нет). Blink `CalculateEdgeMarginBoxSizes`.
pub fn edge_sizes(prefs: [Option<Pref>; 3], avail: f32) -> [f32; 3] {
    let mut p: [Pref; 3] = [Pref::default(); 3];
    let mut out = [0.0f32; 3];
    let mut auto_max = 0.0f32;
    let mut any_auto = false;
    for i in 0..3 {
        if let Some(x) = prefs[i] {
            p[i] = x;
            out[i] = x.max_len();
            if x.auto {
                any_auto = true;
                auto_max += x.max_len();
            }
        }
    }
    // Ни у одной `auto`-коробки нет содержимого — место делится поровну.
    if any_auto && auto_max == 0.0 {
        for x in p.iter_mut().filter(|x| x.auto) {
            *x = Pref {
                min: 1.0,
                max: 1.0,
                margins: 0.0,
                auto: true,
            };
        }
    }
    if prefs[1].is_some() {
        if p[1].auto {
            // Воображаемая коробка AC — удвоенная начальная, затем удвоенная
            // конечная; центр — меньший из двух исходов (центровка B).
            let (c1, _) = resolve_two([p[1], Pref::default(), p[0].doubled()], avail);
            let (c2, _) = resolve_two([p[1], Pref::default(), p[2].doubled()], avail);
            out[1] = c1.min(c2);
        }
        let side = avail - out[1];
        if p[0].auto {
            out[0] = side / 2.0;
        }
        if p[2].auto {
            out[2] = side - side / 2.0;
        }
    } else {
        let (a, c) = resolve_two(p, avail);
        out[0] = a;
        out[2] = c;
    }
    for i in 0..3 {
        out[i] = (out[i] - p[i].margins).max(0.0);
    }
    out
}

/// Поля коробки по оси, прилегающей к краю бумаги (Blink
/// `ResolveMarginsForPageMarginBox`): `auto` делят остаток, пере-определение
/// снимается полем, смотрящим ОТ центра листа (`at_start` — коробка у
/// начального края оси: верхнего или левого).
pub fn edge_margins(
    start: Option<f32>,
    end: Option<f32>,
    border_box: f32,
    avail: f32,
    at_start: bool,
) -> (f32, f32) {
    let (mut s, mut e) = (start.unwrap_or(0.0), end.unwrap_or(0.0));
    let extra = (avail - border_box - s - e).max(0.0);
    match (start, end) {
        (None, None) => {
            s += extra / 2.0;
            e += extra - extra / 2.0;
        }
        (None, Some(_)) => s += extra,
        (Some(_), None) => e += extra,
        _ => {}
    }
    let gap = avail - (border_box + s + e);
    if gap != 0.0 {
        if at_start {
            s += gap;
        } else {
            e += gap;
        }
    }
    (s, e)
}
