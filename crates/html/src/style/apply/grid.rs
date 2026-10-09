//! Сетка: дорожки, имена линий, размещение элементов, grid_style.

use crate::style::apply::*;

/// Дорожка сетки в терминах GPUI. Нижняя грань всегда `min-content`: без неё
/// колонка на узкой панели схлопывается в ноль и содержимое обрезается.
/// Одна грань дорожки.
pub(crate) fn bound(t: &Track) -> gpui::GridTrack {
    match t {
        Track::Px(v) => gpui::GridTrack::Pixels(px(*v)),
        Track::Auto => gpui::GridTrack::Auto,
        Track::MinContent => gpui::GridTrack::MinContent,
        Track::MaxContent => gpui::GridTrack::MaxContent,
        Track::Fr(f) => gpui::GridTrack::Fraction(*f),
        Track::Pct(p) => gpui::GridTrack::Percent(*p),
        // Сюда единица шрифта дойти не должна: её переводит в точки
        // разрешение кегля. Если всё же дошла — ведём себя как `auto`.
        Track::Font(_) => gpui::GridTrack::Auto,
        Track::FitPx(v) => gpui::GridTrack::FitContentPx(px(*v)),
        Track::FitPct(p) => gpui::GridTrack::FitContentPercent(*p),
    }
}

/// Дорожка сетки: одиночная либо пара граней.
///
/// Одиночная переносится как есть — оборачивать её в `minmax` нельзя, иначе
/// нижняя грань разрешает колонке вырасти сверх заданного (поймано сравнением
/// с Chrome: колонка 120px выходила 200). Исключение — доля свободного места:
/// `1fr` в CSS и есть `minmax(auto, 1fr)`, иначе она схлопывается под
/// содержимым.
pub(crate) fn track(t: &TrackSize) -> gpui::GridTrack {
    match t {
        TrackSize::MinMax(lo, hi) => gpui::GridTrack::MinMax(Box::new((bound(lo), bound(hi)))),
        TrackSize::Single(Track::Fr(f)) => gpui::GridTrack::MinMax(Box::new((
            gpui::GridTrack::Auto,
            gpui::GridTrack::Fraction(*f),
        ))),
        TrackSize::Single(one) => bound(one),
        TrackSize::AutoRepeat { fit, tracks } => gpui::GridTrack::AutoRepeat {
            fit: *fit,
            tracks: tracks.iter().map(track).collect(),
        },
    }
}

/// `justify-content`/`align-content` → распределение GPUI.
pub(crate) fn to_content(j: Justify) -> gpui::AlignContent {
    match j {
        Justify::Center => gpui::AlignContent::Center,
        Justify::Start => gpui::AlignContent::FlexStart,
        Justify::End => gpui::AlignContent::FlexEnd,
        // Начало и конец ОСИ ПИСЬМА: у раскладки это отдельные значения, и
        // при обратном направлении ряда они не совпадают с гибкими.
        Justify::WmStart | Justify::Left => gpui::AlignContent::Start,
        Justify::WmEnd | Justify::Right => gpui::AlignContent::End,
        Justify::Between => gpui::AlignContent::SpaceBetween,
        Justify::Around => gpui::AlignContent::SpaceAround,
        // `space-evenly` отличается от `space-around` шириной крайних
        // промежутков — сводить их в одно значение нельзя.
        Justify::Evenly => gpui::AlignContent::SpaceEvenly,
        Justify::Stretch => gpui::AlignContent::Stretch,
    }
}

/// Имена линий контейнера-сетки и именованные грани элемента — раскладке
/// (css-grid-2 §7.2.2, §8.3; разрешает taffy `NamedLineResolver`, через
/// подсетки — с наследованием имён родителя, §9 (d)). Оси контейнера
/// ЛОГИЧЕСКИЕ и переставляются при вертикальном письме, как его дорожки
/// (`grid_style`); у подсеточной оси список — `<line-name-list>`. Грани
/// элемента идут той же осью, что и его числовые (`grid_location`).
pub(crate) fn grid_line_names(c: &Computed) -> Option<gpui::GridLineNames> {
    let mut out = gpui::GridLineNames::default();
    let grid = matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid));
    if grid {
        let flip = c.vertical == Some(true);
        let subgrid_ok = !crate::dom::subgrid_inhibited(c);
        let (cols, rows) = (c.grid_col_line_names.clone(), c.grid_row_line_names.clone());
        let (cols_sub, rows_sub) = (c.subgrid_cols && subgrid_ok, c.subgrid_rows && subgrid_ok);
        // Логическая ось → (шаблон, подсетка) физической оси.
        let mut put = |names: Option<gpui::GridAxisLineNames>, sub: bool, physical_cols: bool| {
            let Some(names) = names else { return };
            match (sub, physical_cols) {
                (true, true) => out.subgrid_columns = Some(names),
                (true, false) => out.subgrid_rows = Some(names),
                (false, true) => out.columns = Some(names),
                (false, false) => out.rows = Some(names),
            }
        };
        put(cols, cols_sub, !flip);
        put(rows, rows_sub, flip);
        // Области шаблона неявно называют линии `имя-start`/`имя-end`
        // (css-grid-2 §7.3.2 «implicitly-assigned line names»): без них
        // `A-start -1` при `[A-start]` и области `A` видел лишь явную линию,
        // а `B -1`/`span B` уходили за явную сетку. Прямоугольник области —
        // по её ячейкам (шаблон уже проверен на прямоугольность разбором).
        if let Some(areas) = c.grid_areas.as_ref() {
            let mut rects: Vec<(String, u16, u16, u16, u16)> = vec![];
            for (r, cells) in areas.iter().enumerate() {
                for (k, cell) in cells.iter().enumerate() {
                    if cell.chars().all(|ch| ch == '.') {
                        continue;
                    }
                    let (r, k) = (r as u16 + 1, k as u16 + 1);
                    match rects.iter_mut().find(|a| a.0 == *cell) {
                        Some(a) => {
                            a.1 = a.1.min(r);
                            a.2 = a.2.max(r + 1);
                            a.3 = a.3.min(k);
                            a.4 = a.4.max(k + 1);
                        }
                        None => rects.push((cell.clone(), r, r + 1, k, k + 1)),
                    }
                }
            }
            let size = (
                areas.len() as u16,
                areas.iter().map(|r| r.len()).max().unwrap_or(0) as u16,
            );
            if flip {
                for a in &mut rects {
                    *a = (std::mem::take(&mut a.0), a.3, a.4, a.1, a.2);
                }
                out.area_size = (size.1, size.0);
            } else {
                out.area_size = size;
            }
            out.areas = rects;
        }
    }
    if placement_flip(c) {
        out.column = c.grid_row_named.clone();
        out.row = c.grid_col_named.clone();
    } else {
        out.column = c.grid_col_named.clone();
        out.row = c.grid_row_named.clone();
    }
    let empty = out.columns.is_none()
        && out.rows.is_none()
        && out.subgrid_columns.is_none()
        && out.subgrid_rows.is_none()
        && out.areas.is_empty()
        && out.column.iter().all(Option::is_none)
        && out.row.iter().all(Option::is_none);
    (!empty).then_some(out)
}

/// Размещение элемента сетки — для обёртки `render::content_sized`: в
/// дорожках родителя стоит ОНА, а не сам элемент (css-grid-2 §8: размещение —
/// свойство элемента сетки, а элементом здесь служит обёртка). Числовые грани
/// и именованные (без имён линий самого элемента: обёртка — своя сетка).
pub(crate) fn grid_item_placement(c: &Computed) -> (Option<gpui::GridLocation>, Option<gpui::GridLineNames>) {
    let location = (c.grid_col.is_some() || c.grid_row.is_some()).then(|| {
        let span = |p: Option<(Placement, Placement)>| {
            let (a, b) = p.unwrap_or((Placement::Auto, Placement::Auto));
            to_placement(a)..to_placement(b)
        };
        if placement_flip(c) {
            gpui::GridLocation { row: span(c.grid_col), column: span(c.grid_row) }
        } else {
            gpui::GridLocation { row: span(c.grid_row), column: span(c.grid_col) }
        }
    });
    let named = c.grid_col_named.iter().chain(c.grid_row_named.iter()).any(Option::is_some);
    let (column, row) = if placement_flip(c) {
        (c.grid_row_named.clone(), c.grid_col_named.clone())
    } else {
        (c.grid_col_named.clone(), c.grid_row_named.clone())
    };
    let names = named.then(|| gpui::GridLineNames { column, row, ..Default::default() });
    (location, names)
}

/// Элемент вертикальной сетки (и лунок на её пути): его логические грани
/// ложатся на переставленные физические оси (см. `grid_style`, `flip`).
pub(crate) fn placement_flip(c: &Computed) -> bool {
    c.parent_grid >= 2
}

pub(crate) fn to_placement(p: Placement) -> gpui::GridPlacement {
    match p {
        Placement::Auto => gpui::GridPlacement::Auto,
        Placement::Line(n) => gpui::GridPlacement::Line(n),
        Placement::Span(n) => gpui::GridPlacement::Span(n),
    }
}

/// Стиль контейнера-сетки: дорожки, неявные дорожки, направление.
pub(crate) fn grid_style(mut d: Div, c: &Computed) -> Div {
    d = d.grid();
    d.style().grid_axis_reversed = Some(grid_flow_axes::reversed(c));
    // Контейнер лунок на пути сетки (`dom::lanes_as_grid`): раскладку лунками
    // делает taffy. Порог `flow-tolerance: normal` — 1em (css-grid-3
    // Overview.bs:828-831), `infinite` разбор держит бесконечными точками.
    if c.lanes_taffy {
        let em = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let (tolerance, tolerance_pct) = match c.lanes_tolerance {
            Some(Len::Px(v)) => (v, None),
            Some(Len::Em(k)) => (k * em, None),
            Some(Len::Pct(k)) => (0.0, Some(k)),
            _ => (em, None),
        };
        // Ось решётки taffy — ФИЗИЧЕСКАЯ, а `grid-lanes-direction` —
        // логическая: при вертикальном письме колонки идут по y, ряды — по x
        // (та же перестановка, что у дорожек сетки ниже, `flip`).
        // ★ ЗАМЕРЕНО: `vertical-rl` как `fill-reverse` колоночных лунок —
        // лунки вертикального письма (116 пар) +1/−1, не взято.
        let vertical = c.vertical == Some(true);
        let logical_rows = crate::dom::lanes_row_dir(c);
        d.style().grid_lanes = Some(gpui::GridLanesFlow {
            rows: logical_rows != vertical,
            track_reverse: c.lanes_track_reverse,
            fill_reverse: c.lanes_fill_reverse,
            dense: c.lanes_dense,
            tolerance,
            tolerance_pct,
            stack_block: vertical && !logical_rows,
        });
        // По оси укладки `normal` — это НЕ растяжка (css-grid-3
        // Overview.bs:1161-1225: самовыравнивание лишь у элементов над
        // проёмом; Blink `ResolvedAlignSelf(normal)` :1056-1060), а общий
        // путь `align-items: stretch` в стиль не пишет — для сетки это
        // умолчание. Лункам явная растяжка нужна в стиле.
        if c.align_items == Some(crate::computed::Align::Stretch) {
            d.style().align_items = Some(gpui::AlignItems::Stretch);
        }
    }
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
                && !matches!(c.display, Some(crate::computed::Display::GridLanes)) =>
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
    } else if let (None, Some(r), Some(body)) = (&c.grid_rows, c.auto_repeat_rows, &c.auto_repeat_body_rows)
        && body.len() > 1
        && body.iter().all(|t| matches!(t, TrackSize::Single(crate::computed::Track::Px(_))))
    {
        // Тело повтора рядов из НЕСКОЛЬКИХ точечных дорожек
        // (`repeat(auto-fill, [v] 10px [w] 10px [x] 10px [y])`) — тем же видом,
        // что у колонок выше: прежде ряды такой записи не получали шаблона
        // вовсе, и имена линий повтора (css-grid-2 §7.2.3.1 «names … in the
        // repeat() are repeated as well») разрешались по пустой явной сетке —
        // у лунок и у сетки-эталона по-разному (`row-auto-repeat-014`).
        let line = vec![gpui::GridTrack::AutoRepeat { fit: r.fit, tracks: body.iter().map(track).collect() }];
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
    if c.display == Some(Display::InlineGrid) && c.width.is_none() {
        if let Some(tracks) = &c.grid_tracks {
            let all_px: Option<f32> = tracks.iter().try_fold(0.0f32, |acc, t| match t {
                crate::computed::TrackSize::Single(crate::computed::Track::Px(w)) => Some(acc + w),
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
    }
    if let Some(rows) = &c.grid_rows {
        // Ряды областей сверх шаблона — тоже явные (см. `with_areas` выше).
        let tracks = with_areas(rows, area_rows, &c.grid_auto_rows, &c.grid_auto_rows_list);
        d = if flip {
            d.grid_template_cols(tracks)
        } else {
            d.grid_template_rows(tracks)
        };
    }
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
            d.style().grid_auto_rows_list =
                Some(c.grid_auto_rows_list.iter().map(track).collect());
        }
    }
    if let Some(t) = auto_flow_axis {
        d.style().grid_auto_cols = Some(track(t));
        if !c.grid_auto_cols_list.is_empty() {
            d.style().grid_auto_cols_list =
                Some(c.grid_auto_cols_list.iter().map(track).collect());
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
                    TrackSize::Single(crate::computed::Track::Px(_) | crate::computed::Track::Pct(_))
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
            (c.subgrid_rows, c.subgrid_cols, row_gap_normal, col_gap_normal)
        } else {
            (c.subgrid_cols, c.subgrid_rows, col_gap_normal, row_gap_normal)
        };
        d.style().grid_subgrid = Some(
            u8::from(cols) | (u8::from(rows) << 1) | (u8::from(col_gap) << 2) | (u8::from(row_gap) << 3),
        );
    }
    d
}
