//! grid_style, этапы рядов и неявной сетки: repeat() по оси рядов, inline-grid, неявные дорожки grid-auto-*, лунки, grid-auto-flow, subgrid.

use super::*;

pub(super) fn grid_implicit(mut d: Div, c: &Computed, flip: bool) -> Div {
    // Неявные дорожки: элементов больше, чем описано — их размер задаёт
    // `grid-auto-*`, иначе они выходят по содержимому.
    let (auto_line, auto_flow_axis) = if flip {
        (&c.grid_auto_cols, &c.grid_auto_rows)
    } else {
        (&c.grid_auto_rows, &c.grid_auto_cols)
    };
    if let Some(t) = auto_line {
        d.style().grid_auto_rows = Some(track(t));
        if !c.grid_auto_rows_list.is_empty() {
            d.style().grid_auto_rows_list = Some(c.grid_auto_rows_list.iter().map(track).collect());
        }
    }
    if let Some(t) = auto_flow_axis {
        d.style().grid_auto_cols = Some(track(t));
        if !c.grid_auto_cols_list.is_empty() {
            d.style().grid_auto_cols_list = Some(c.grid_auto_cols_list.iter().map(track).collect());
        }
    }
    // Лунки на пути сетки: одинокий `repeat(auto-*)` с дорожками ПО
    // СОДЕРЖИМОМУ (css-grid-3 §7.2.1 «Intrinsic Tracks and repeat()»,
    // Overview.bs:444-500) отдаётся раскладке телом как есть — число повторов
    // по гипотетическим размерам считает `taffy::compute::grid::lanes`. Ветки
    // выше умеют только точечное тело и подменяли его счётом колонок.
    if c.lanes_taffy {
        let row_dir = crate::dom::lanes_row_dir(c);
        let (repeat, body, list) = if row_dir {
            (c.auto_repeat_rows, &c.auto_repeat_body_rows, &c.grid_rows)
        } else {
            (c.auto_repeat_cols, &c.auto_repeat_body_cols, &c.grid_tracks)
        };
        if let (None, Some(r), Some(body)) = (list, repeat, body)
            && r.track.is_none()
            && r.track_pct.is_none()
            && !body.is_empty()
            // Только тело с дорожкой ПО СОДЕРЖИМОМУ: точечное тело ведут
            // ветки выше (★ ЗАМЕРЕНО: `row-auto-repeat-014`, тело
            // `[v] 10px [w] 10px [x] 10px`, 0.00 → 18.83 при отдаче сюда).
            && body.iter().any(|t| {
                !matches!(
                    t,
                    TrackSize::Single(crate::style::computed::Track::Px(_) | crate::style::computed::Track::Pct(_))
                )
            })
        {
            let line = vec![gpui::GridTrack::AutoRepeat {
                fit: r.fit,
                tracks: body.iter().map(track).collect(),
            }];
            d = if row_dir != flip {
                d.grid_template_rows(line)
            } else {
                d.grid_template_cols(line)
            };
        }
    }
    if let Some(f) = c.grid_auto_flow {
        // Направление наполнения тоже логическое: «по рядам» значит «вдоль
        // строки», а строка при вертикальном письме идёт сверху вниз.
        let f = if flip {
            match f {
                AutoFlow::Row => AutoFlow::Col,
                AutoFlow::Col => AutoFlow::Row,
                AutoFlow::RowDense => AutoFlow::ColDense,
                AutoFlow::ColDense => AutoFlow::RowDense,
            }
        } else {
            f
        };
        d.style().grid_auto_flow = Some(match f {
            AutoFlow::Row => gpui::GridAutoFlow::Row,
            AutoFlow::Col => gpui::GridAutoFlow::Column,
            AutoFlow::RowDense => gpui::GridAutoFlow::RowDense,
            AutoFlow::ColDense => gpui::GridAutoFlow::ColumnDense,
        });
    }
    // Подсетка (css-grid-2 §9): раскладке — ФИЗИЧЕСКИЕ подсеточные оси и
    // признак зазора `normal` (§subgrid-gaps: «same size gutters as its
    // parent grid»). Дорожки подсеточной оси taffy берёт у родительской
    // сетки уже размеренными (`taffy::compute::grid::subgrid`), а элементы
    // подсетки вкладываются в дорожки родителя. Оси логические: при
    // вертикальном письме колонки подсетки — физические ряды (Blink
    // `grid_item.cc:192-197`). Обособленная подсетка подсеткой не является
    // (§subgrid-listing, `dom::subgrid_inhibited`).
    if (c.subgrid_cols || c.subgrid_rows) && !crate::dom::subgrid_inhibited(c) {
        let col_gap_normal = c.gap.and_then(|g| g.1).or(c.column_gap).is_none();
        let row_gap_normal = c.gap.and_then(|g| g.0).is_none();
        let (cols, rows, col_gap, row_gap) = if flip {
            (
                c.subgrid_rows,
                c.subgrid_cols,
                row_gap_normal,
                col_gap_normal,
            )
        } else {
            (
                c.subgrid_cols,
                c.subgrid_rows,
                col_gap_normal,
                row_gap_normal,
            )
        };
        d.style().grid_subgrid = Some(
            u8::from(cols)
                | (u8::from(rows) << 1)
                | (u8::from(col_gap) << 2)
                | (u8::from(row_gap) << 3),
        );
    }
    d
}

pub(super) fn grid_rows_repeat(mut d: Div, c: &Computed, flip: bool) -> Div {
    // Повтор по ОСИ РЯДОВ: у классической сетки его не было вовсе, и
    // `grid-template-rows: repeat(auto-fill, …)` уходил в никуда — ряды
    // становились неявными, нулевой высоты.
    if let Some(r) = c.grid_rows_repeat() {
        let lo = match r.track_pct {
            Some(k) => gpui::GridTrack::Percent(k),
            None => gpui::GridTrack::Pixels(px(r.track.unwrap_or(0.0))),
        };
        // Максимум `minmax(N, auto | k fr)` — как у колонок выше.
        let unit = if r.max_auto {
            gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Auto)))
        } else if let Some(k) = r.max_fr {
            gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Fraction(k))))
        } else {
            lo
        };
        let line = vec![gpui::GridTrack::AutoRepeat {
            fit: r.fit,
            tracks: vec![unit],
        }];
        d = if flip {
            d.grid_template_cols(line)
        } else {
            d.grid_template_rows(line)
        };
    } else if let (None, Some(r), Some(body)) =
        (&c.grid_rows, c.auto_repeat_rows, &c.auto_repeat_body_rows)
        && body.len() > 1
        && body
            .iter()
            .all(|t| matches!(t, TrackSize::Single(crate::style::computed::Track::Px(_))))
    {
        // Тело повтора рядов из НЕСКОЛЬКИХ точечных дорожек
        // (`repeat(auto-fill, [v] 10px [w] 10px [x] 10px [y])`) — тем же видом,
        // что у колонок выше: прежде ряды такой записи не получали шаблона
        // вовсе, и имена линий повтора (css-grid-2 §7.2.3.1 «names … in the
        // repeat() are repeated as well») разрешались по пустой явной сетке —
        // у лунок и у сетки-эталона по-разному (`row-auto-repeat-014`).
        let line = vec![gpui::GridTrack::AutoRepeat {
            fit: r.fit,
            tracks: body.iter().map(track).collect(),
        }];
        d = if flip {
            d.grid_template_cols(line)
        } else {
            d.grid_template_rows(line)
        };
    }
    // Строчная сетка ОБНИМАЕТ свои Px-дорожки (shrink-to-fit): блочная
    // ширина на всю строку ломала все пары с `display: inline grid` в
    // разметке эталонов (subgrid-alignment-in-subgridded-axis: серый фон до
    // края страницы вместо 100px). gpui-размер — border-box: паддинги и
    // рамки сверху.
    if c.display == Some(Display::InlineGrid)
        && c.width.is_none()
        && let Some(tracks) = &c.grid_tracks
    {
        let all_px: Option<f32> = tracks.iter().try_fold(0.0f32, |acc, t| match t {
            crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(w)) => {
                Some(acc + w)
            }
            _ => None,
        });
        if let Some(mut total) = all_px.filter(|t| *t > 0.0) {
            let px_of = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let b = c.borders();
            total += px_of(c.column_gap) * (tracks.len().saturating_sub(1)) as f32
                + px_of(c.padding.left)
                + px_of(c.padding.right)
                + px_of(b.left)
                + px_of(b.right);
            d = d.w(px(total));
        }
    }
    d
}
