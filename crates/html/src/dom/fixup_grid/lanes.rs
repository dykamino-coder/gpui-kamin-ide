//! Контейнер лунок (css-grid-3) на путь обычной сетки.
//!
//! `lanes_as_grid` переводит `display: grid-lanes` в `grid`/`inline-grid` с
//! пометкой `lanes_taffy`: по оси решётки доступна «the full power of grid
//! layout» (css-grid-3 §grid-lanes-track-templates), и шаблоны, зазоры,
//! выравнивание, подсетки и дети идут тем же кодом, что у сетки-эталона,
//! а лунками раскладывает taffy. `lanes_row_dir` — ось лунок: явное
//! `grid-lanes-direction`, иначе та ось, по которой объявлены дорожки.

use crate::dom::*;
use crate::style::computed::{Computed, Display};

/// Абсолютный ПОТОМОК сетки размещается по её линиям, а не по статической
/// позиции: если содержащий блок такого элемента — сама сетка, то `grid-row`
/// и `grid-column` задают ему прямоугольник области (css-grid-2 §9). Раскладка
/// знает только ПРЯМЫХ детей сетки, поэтому потомок поднимается к ней. Стиль к
/// этому моменту уже вычислен, и переезд по дереву его не меняет.
/// Ось лунок контейнера: `true` — лунки РЯДАМИ (ось решётки — ряды).
///
/// Без явного `grid-lanes-direction` направление выдаёт ТА ОСЬ, по которой
/// объявлены дорожки — то же правило, что у `render::lanes`.
pub(crate) fn lanes_row_dir(s: &Computed) -> bool {
    let row_tracks =
        s.grid_rows.is_some() || s.auto_repeat_rows.is_some() || s.grid_auto_fill_row.is_some();
    let col_tracks =
        s.grid_tracks.is_some() || s.auto_repeat_cols.is_some() || s.grid_auto_fill_min.is_some();
    s.lanes_row.unwrap_or(row_tracks && !col_tracks)
}

/// Контейнер лунок — на путь СЕТКИ, раскладку лунками делает taffy
/// (`vendor/taffy/src/compute/grid/lanes.rs`, css-grid-3).
///
/// css-grid-3 §grid-lanes-track-templates (Overview.bs:414-433): по оси
/// решётки «the full power of grid layout is available» — шаблоны, линии,
/// области, явная и неявная сетка «formed in the same way as for a regular
/// grid container», а дорожки размеряются алгоритмом css-grid-2 §12
/// (Overview.bs:619-669). Поэтому контейнер становится обычной сеткой с
/// пометкой `lanes_taffy`: шаблоны, зазоры, выравнивание и дети идут ТЕМ ЖЕ
/// путём, что у сетки-эталона (`grid-subgridded-to-grid-lanes/**` — та же
/// разметка на `inline-grid`), а не рукописной оценкой `render::lanes`.
///
/// Все контейнеры лунок идут сюда: вертикальное письмо — осями из
/// `paint/apply/grid.rs` (`grid_style`, `placement_flip`), `rtl` — как у сетки-эталона
/// (зеркала строчной оси у сетки taffy нет, и эталоны `inline-grid` с `rtl`
/// рисуются тем же путём). Подсетки среди
/// детей идут тем же путём: срез им режет `subgrid_takes_parent_tracks`
/// ровно как у сетки-эталона; интрин-дорожки в `repeat(auto-*)` считает
/// taffy (css-grid-3 §7.2.1).
pub(crate) fn lanes_as_grid(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        lanes_as_grid(&mut el.children);
        if el.style.cascaded().display != Some(Display::GridLanes) {
            continue;
        }
        lanes_to_grid(el.style.cascaded_mut());
    }
}

/// Перевод контейнера лунок на путь сетки (см. `lanes_as_grid`).
pub(crate) fn lanes_to_grid(style: &mut Computed) {
    style.display = Some(if style.lanes_inline {
        Display::InlineGrid
    } else {
        Display::Grid
    });
    style.lanes_taffy = true;
}
