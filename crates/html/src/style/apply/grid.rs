//! Сетка: дорожки, имена линий, размещение элементов, grid_style.

use crate::style::apply::*;
use crate::style::computed::{AutoFlow, Computed, Display, Justify, Placement, Track, TrackSize};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

mod lanes;
mod placement;
mod rows_implicit;
mod track_sizes;
use lanes::grid_lanes;
pub(crate) use placement::grid_item_placement;
pub(super) use placement::{grid_line_names, placement_flip, to_placement};
use rows_implicit::{grid_implicit, grid_rows_repeat};
pub(super) use track_sizes::to_content;
use track_sizes::track;

/// Стиль контейнера-сетки: дорожки, неявные дорожки, направление.
pub(super) fn grid_style(mut d: Div, c: &Computed) -> Div {
    d = d.grid();
    d.style().grid_axis_reversed = Some(grid_flow_axes::reversed(c));
    d = grid_lanes(d, c);
    // Оси сетки ЛОГИЧЕСКИЕ: «колонки» идут вдоль строки, «ряды» — вдоль
    // потока. При вертикальном письме строка идёт сверху вниз, а поток —
    // поперёк, поэтому колонки становятся физическими рядами и наоборот.
    // Раскладка под нами письма не знает и считает оси физическими, так что
    // переставляем здесь, на границе.
    let flip = c.vertical == Some(true);
    let along_line = |d: Div, tracks: Vec<gpui::GridTrack>| -> Div {
        if flip {
            d.grid_template_rows(tracks)
        } else {
            d.grid_template_cols(tracks)
        }
    };
    // Список дорожек точнее числа колонок: он несёт ширину по
    // содержимому и фиксированные колонки (патч GPUI, см. доку).
    // Доля дорожки в `repeat(auto-fill, 25%)` считается от размера контейнера,
    // а он известен прямо здесь: `25%` в трёхстах точках — четыре дорожки по
    // 75. Пока доля отбрасывалась, сетка не получала дорожек вовсе
    // (`column-auto-repeat-002` и родня).
    let auto_fill = c.grid_auto_fill_min.or_else(|| {
        let k = c.auto_repeat_cols?.track_pct?;
        match c.width {
            Some(Len::Px(w)) => Some(k * w),
            _ => None,
        }
    });
    // ПРОБОВАЛИ И ОТКАТИЛИ: `grid-template-columns: subgrid` разворачивать в
    // столько СВОИХ дорожек, сколько линий родителя элемент перекрывает.
    // Семейству subgrid-gap +10, но −17 по subgrid-auto-fill и базовым линиям:
    // прежде зелёные пары совпадали с эталоном ИМЕННО одноколоночным
    // поведением, а свои дорожки без настоящих ширин родителя их разломали.
    // Возвращаться только с настоящей передачей дорожек родителя вниз.
    // Явная сетка — не меньше `grid-template-areas`: «The size of the explicit
    // grid is determined by the larger of the number of rows/columns defined
    // by 'grid-template-areas' and the number of rows/columns sized by
    // 'grid-template-rows'/'grid-template-columns'», лишние берут размер
    // `grid-auto-*` (css-grid-2 Overview.bs:1495-1499; Blink
    // `grid_line_resolver.cc:525-538`). Раскладке области не передаются, и их
    // лишние колонки были у неё НЕЯВНЫМИ: размер тот же, но линия `-1`
    // считалась от шаблона (`subgrid/abs-pos-001`: `3 / -1` абсолюта во внешней
    // сетке кончался на линии 11, а не 12). Список `grid-auto-*` из нескольких
    // значений и повтор `auto-fill/fit` не трогаем: там счёт другой.
    let (area_rows, area_cols) = c.grid_areas.as_ref().map_or((0, 0), |a| {
        (a.len(), a.iter().map(|r| r.len()).max().unwrap_or(0))
    });
    let with_areas =
        |tracks: &[TrackSize], want: usize, auto: &Option<TrackSize>, list: &[TrackSize]| {
            let mut out: Vec<gpui::GridTrack> = tracks.iter().map(track).collect();
            let plain = tracks
                .iter()
                .all(|t| !matches!(t, TrackSize::AutoRepeat { .. }));
            if plain && list.is_empty() && want > out.len() {
                let fill = auto.as_ref().map(track).unwrap_or(gpui::GridTrack::Auto);
                out.resize(want, fill);
            }
            out
        };
    match (&c.grid_tracks, c.grid_cols, auto_fill) {
        (Some(tracks), _, _) => {
            d = along_line(
                d,
                with_areas(tracks, area_cols, &c.grid_auto_cols, &c.grid_auto_cols_list),
            )
        }
        // «Сколько влезет» умеет сама раскладка — короткая форма GPUI.
        // Тело повтора из НЕСКОЛЬКИХ дорожек: своего «минимума» оно не даёт
        // (`auto_fill_min` разбирает одну дорожку), поэтому идёт своей ветвью.
        // Раскладка список принимает как есть — `GridTrack::AutoRepeat`
        // хранит `Vec` (`grid-auto-repeat-multiple-values-*` рисовались одной
        // плитой во всю ширину).
        // Поток ЛУНОК разворачивает повтор своим кодом (`render::lanes`), и
        // список дорожек ему только мешает: `column-auto-repeat-013`
        // уходил 0.00 → 10.92. Признак — `lanes_row`/`lanes_inline` и родня,
        // они выставлены только у лунок.
        (None, _, None)
            if !flip
                && c.grid_auto_fill_tracks.len() > 1
                // Поток ЛУНОК доходит сюда уже с `display: grid` (печать
                // `KAMIN_REPEAT_DIAG` показала `Some(Grid)`), поэтому вид его
                // не отсекает: `column-auto-repeat-013` (лунки, черновик)
                // уходит 0.00 → 10.92 — это записанная цена жилы.
                && !matches!(c.display, Some(crate::style::computed::Display::GridLanes)) =>
        {
            d = along_line(
                d,
                vec![gpui::GridTrack::AutoRepeat {
                    fit: c.auto_repeat_cols.is_some_and(|r| r.fit),
                    tracks: c
                        .grid_auto_fill_tracks
                        .iter()
                        .map(|v| gpui::GridTrack::Pixels(px(*v)))
                        .collect(),
                }],
            )
        }
        (None, _, Some(min)) if !flip => {
            // Повтор отдаётся раскладке СВОИМ видом: она считает, сколько
            // дорожек влезет, и при `auto-fit` схлопывает пустые
            // (css-grid-2 §auto-repeat). Прежняя короткая форма подменяла
            // дорожку растяжкой `minmax(min, 1fr)`, и уцелевшие дорожки
            // забирали весь остаток — раздавать было нечего.
            let r = c.auto_repeat_cols;
            // Тело повтора бывает из НЕСКОЛЬКИХ дорожек
            // (`repeat(auto-fill, 50px 50px)`) — раскладка это уже умеет,
            // список идёт в неё как есть.
            let lo = match r.and_then(|r| r.track_pct) {
                Some(k) => gpui::GridTrack::Percent(k),
                None => gpui::GridTrack::Pixels(px(min)),
            };
            // `minmax(N, auto | k fr)`: максимум остаётся у дорожки — растяжка
            // остатком (§12.8) и доли считает сама раскладка.
            let track = match r {
                Some(r) if r.max_auto => {
                    gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Auto)))
                }
                Some(r) if r.max_fr.is_some() => gpui::GridTrack::MinMax(Box::new((
                    lo,
                    gpui::GridTrack::Fraction(r.max_fr.unwrap_or(1.0)),
                ))),
                _ => lo,
            };
            let tracks: Vec<gpui::GridTrack> = vec![track];
            d = along_line(
                d,
                vec![gpui::GridTrack::AutoRepeat {
                    fit: r.is_some_and(|r| r.fit),
                    tracks,
                }],
            )
        }
        (None, Some(n), _) if !flip => d = d.grid_cols(n),
        (None, Some(n), _) => {
            d = d.grid_template_rows((0..n).map(|_| gpui::GridTrack::Auto).collect())
        }
        _ => {}
    }
    d = grid_rows_repeat(d, c, flip);
    if let Some(rows) = &c.grid_rows {
        // Ряды областей сверх шаблона — тоже явные (см. `with_areas` выше).
        let tracks = with_areas(rows, area_rows, &c.grid_auto_rows, &c.grid_auto_rows_list);
        d = if flip {
            d.grid_template_cols(tracks)
        } else {
            d.grid_template_rows(tracks)
        };
    }
    grid_implicit(d, c, flip)
}
