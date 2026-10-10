//! Имена линий и размещение элемента сетки (grid-row/column) для GPUI, с разворотом осей при вертикальном письме.

use super::*;

/// Имена линий контейнера-сетки и именованные грани элемента — раскладке
/// (css-grid-2 §7.2.2, §8.3; разрешает taffy `NamedLineResolver`, через
/// подсетки — с наследованием имён родителя, §9 (d)). Оси контейнера
/// ЛОГИЧЕСКИЕ и переставляются при вертикальном письме, как его дорожки
/// (`grid_style`); у подсеточной оси список — `<line-name-list>`. Грани
/// элемента идут той же осью, что и его числовые (`grid_location`).
pub(in crate::style::apply) fn grid_line_names(c: &Computed) -> Option<gpui::GridLineNames> {
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
pub(crate) fn grid_item_placement(
    c: &Computed,
) -> (Option<gpui::GridLocation>, Option<gpui::GridLineNames>) {
    let location = (c.grid_col.is_some() || c.grid_row.is_some()).then(|| {
        let span = |p: Option<(Placement, Placement)>| {
            let (a, b) = p.unwrap_or((Placement::Auto, Placement::Auto));
            to_placement(a)..to_placement(b)
        };
        if placement_flip(c) {
            gpui::GridLocation {
                row: span(c.grid_col),
                column: span(c.grid_row),
            }
        } else {
            gpui::GridLocation {
                row: span(c.grid_row),
                column: span(c.grid_col),
            }
        }
    });
    let named = c
        .grid_col_named
        .iter()
        .chain(c.grid_row_named.iter())
        .any(Option::is_some);
    let (column, row) = if placement_flip(c) {
        (c.grid_row_named.clone(), c.grid_col_named.clone())
    } else {
        (c.grid_col_named.clone(), c.grid_row_named.clone())
    };
    let names = named.then(|| gpui::GridLineNames {
        column,
        row,
        ..Default::default()
    });
    (location, names)
}

/// Элемент вертикальной сетки (и лунок на её пути): его логические грани
/// ложатся на переставленные физические оси (см. `grid_style`, `flip`).
pub(in crate::style::apply) fn placement_flip(c: &Computed) -> bool {
    c.parent_grid >= 2
}

pub(in crate::style::apply) fn to_placement(p: Placement) -> gpui::GridPlacement {
    match p {
        Placement::Auto => gpui::GridPlacement::Auto,
        Placement::Line(n) => gpui::GridPlacement::Line(n),
        Placement::Span(n) => gpui::GridPlacement::Span(n),
    }
}
