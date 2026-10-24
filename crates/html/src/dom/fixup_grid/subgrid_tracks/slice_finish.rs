//! Запись среза дорожек в стиль подсетки: края, зазоры, растяжка.
//! Вход — ребёнок-подсетка, стиль родителя, ось и вырезанные дорожки; выход —
//! `grid_rows`/`grid_tracks` подсетки, где из первой и последней дорожки
//! вычтены её собственные поля, рамка и отступы, незаданный зазор взят у
//! родителя (`normal`, css-grid-2 §subgrid-gaps), разница зазоров разложена
//! `subgrid_gap_slice`. В подсеточной оси гасятся размер и self-выравнивание
//! (§subgrid-box-alignment); у ортогональной подсетки ось скрещивается и
//! гасится физический размер.

use super::subgrid_gap_slice;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};

// The slice carries both parent track geometry and child cascade state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_slice(
    child: &mut Element,
    parent: &Computed,
    row_dir: bool,
    parallel: bool,
    _at: usize,
    span: usize,
    _tracks: &[crate::style::computed::TrackSize],
    slice: Vec<crate::style::computed::TrackSize>,
) {
    let px = px_lossy_zero;
    let bs = child.style.cascaded().borders();
    let (lead, trail) = if row_dir {
        (
            px(child.style.cascaded().margin.top)
                + px(bs.top)
                + px(child.style.cascaded().padding.top),
            px(child.style.cascaded().margin.bottom)
                + px(bs.bottom)
                + px(child.style.cascaded().padding.bottom),
        )
    } else {
        (
            px(child.style.cascaded().margin.left)
                + px(bs.left)
                + px(child.style.cascaded().padding.left),
            px(child.style.cascaded().margin.right)
                + px(bs.right)
                + px(child.style.cascaded().padding.right),
        )
    };
    let mut slice = slice;
    if let Some(crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(w))) =
        slice.first_mut()
    {
        *w = (*w - lead).max(0.0);
    }
    if let Some(crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(w))) =
        slice.last_mut()
    {
        *w = (*w - trail).max(0.0);
    }
    // Зазор подсеточной оси у РОДИТЕЛЯ не брать: гейт выше
    // пропускает только обычную сетку, а в `grid-lanes-subgrid-
    // 001c/-002c` и `row-subgrid-grid-gap-005` тест написан на
    // ЛУНКАХ (свой срез — `render::generic_box`), эталон на сетке — стороны
    // разъезжаются. Само правило иное: при разнице зазоров дорожка
    // получает половину разницы с каждой стороны внутреннего
    // стыка, а свой зазор остаётся. Менять только симметрично
    // обоим путям.
    let (prow, pcol) = parent.gap.unwrap_or((None, None));
    let (crow, ccol) = child.style.cascaded().gap.unwrap_or((None, None));
    let par = if row_dir { prow } else { pcol };
    let own = if row_dir { crow } else { ccol };
    // Незаданный зазор подсетки — это `normal`, а он по
    // css-grid-2 §subgrid-gaps значит «такие же зазоры, как у
    // родителя», то есть разница НОЛЬ. Пока `None` считался
    // нулём, разница выходила равной родительскому зазору и
    // дорожки раздувались на его половину.
    // Проверяется СВОЯ ось: первый проход (колонки) уже записал
    // зазор колонок в `child.style.gap`, и прежнее условие «обе
    // оси пусты» во втором проходе ложно — ряды подсетки
    // оставались без зазора (`subgrid-gap-decorations-007`: ряды
    // 0/100/200 вместо 0/110/220 при эталоне `grid-010-ref`).
    let unset = own.is_none();
    let own = own.or(par);
    if own != Some(Len::Px(0.0)) && unset {
        child.style.cascaded_mut().gap = Some(if row_dir { (par, ccol) } else { (crow, par) });
        if !row_dir {
            child.style.cascaded_mut().column_gap = par;
        }
    }
    subgrid_gap_slice(&mut slice, par, own);
    // В подсеточной оси SELF-выравнивание не действует:
    // подсетка держит всю дорожку.
    // css-grid-2 §subgrid-box-alignment: «The subgrid is
    // always stretched in its subgridded dimension(s): the
    // align-self/justify-self properties on it are ignored,
    // as are any specified width/height constraints.»
    //
    // Гейт ПООСЕВОЙ (`subgrid_rows`/`subgrid_cols`), потому
    // что срез приходит в ОБЕ оси, а гасить размер положено
    // только в той, где вправду написано `subgrid`: иначе
    // уходят пять зелёных `standalone-axis-size-*`.
    //
    // Ортогональную подсетку правило пропускает: `grid-template-
    // rows` у неё — ось СВОЯ, и физическое свойство другое.
    // Это отдельный корень, трогать его здесь нельзя — три зелёных
    // `row-subgrid-orthogonal-writing-mode-001/002/003`.
    // ОРТОГОНАЛЬНАЯ подсетка: оси родителя и подсетки
    // скрещены. `Computed` хранит дорожки ЛОГИЧЕСКИ
    // (`apply::grid_style` переставляет их через `flip`), а
    // подсеточная ось называется по шаблону САМОЙ подсетки
    // (css-grid-2 §subgrid-listing): колонки родителя у
    // подсетки с другим письмом — это её РЯДЫ, ряды родителя
    // — её колонки. Blink пишет то же (`grid/grid_item.cc`:
    // `has_subgridded_columns = is_parallel_with_root_grid ?
    // GridTemplateColumns() : GridTemplateRows()`). Прежде
    // срез колонок ложился в колонки подсетки (после `flip` —
    // в ФИЗИЧЕСКИЕ ряды), а §subgrid-box-alignment
    // («always stretched … any specified width/height
    // constraints» игнорируются) у ортогональной подсетки не
    // делался вовсе: вторая половина `subgrid/subgrid-stretch`
    // (восемь коробок `vrl`) держала свои 50/150 вместо
    // дорожки 100. Размер гасится ФИЗИЧЕСКИЙ: ряды
    // горизонтального родителя — высота, колонки — ширина.
    // Разница зазоров по-прежнему пишется в оси родителя —
    // отдельный шаг.
    if !parallel {
        let own = if row_dir {
            child.style.cascaded().subgrid_cols
        } else {
            child.style.cascaded().subgrid_rows
        };
        let vertical_axis = row_dir != parent.wm().vertical();
        // СРЕЗ в скрещенную ось (колонки родителя → `grid_rows`
        // подсетки) не класть: в `grid-subgridded-to-grid-lanes/
        // track-sizing/{column,row}-subgrid-auto-fill-007` тест на
        // ЛУНКАХ (срез режет `render::generic_box`, оси не скрещивает), а
        // эталон — та же разметка на `inline grid` (режет этот
        // проход). Скрещиваются только признак и растяжка ниже;
        // `subgrid-stretch` срезу безразличен (обе оси по 100).
        // Скрещивать можно только вместе с `render::generic_box` (Blink
        // `grid_item.cc:192-207`).
        if row_dir {
            child.style.cascaded_mut().grid_rows = Some(slice);
            child.style.cascaded_mut().align_self = None;
        } else {
            child.style.cascaded_mut().grid_tracks = Some(slice);
            child.style.cascaded_mut().grid_cols = Some(span as u16);
            child.style.cascaded_mut().justify_self = None;
        }
        if own {
            if vertical_axis {
                child.style.cascaded_mut().height = None;
                child.style.cascaded_mut().max_height = None;
                child.style.cascaded_mut().min_height = Some(Len::Px(0.0));
            } else {
                child.style.cascaded_mut().width = None;
                child.style.cascaded_mut().max_width = None;
                child.style.cascaded_mut().min_width = Some(Len::Px(0.0));
            }
            let stretch = Some(crate::style::computed::Align::Stretch);
            if row_dir {
                child.style.cascaded_mut().align_self = stretch;
            } else {
                child.style.cascaded_mut().justify_self = stretch;
            }
        }
    } else if row_dir {
        child.style.cascaded_mut().grid_rows = Some(slice);
        child.style.cascaded_mut().align_self = None;
        if parallel && child.style.cascaded().subgrid_rows {
            child.style.cascaded_mut().height = None;
            child.style.cascaded_mut().max_height = None;
            // Не `None`, а НОЛЬ: `None` вернул бы автоминимум
            // элемента сетки, и подсетка раздулась бы шире
            // своей области. Спека требует «размер
            // игнорируется», а не «минимум по содержимому».
            child.style.cascaded_mut().min_height = Some(Len::Px(0.0));
            child.style.cascaded_mut().align_self = Some(crate::style::computed::Align::Stretch);
        }
    } else {
        child.style.cascaded_mut().grid_tracks = Some(slice);
        child.style.cascaded_mut().grid_cols = Some(span as u16);
        child.style.cascaded_mut().justify_self = None;
        if parallel && child.style.cascaded().subgrid_cols {
            child.style.cascaded_mut().width = None;
            child.style.cascaded_mut().max_width = None;
            child.style.cascaded_mut().min_width = Some(Len::Px(0.0));
            child.style.cascaded_mut().justify_self = Some(crate::style::computed::Align::Stretch);
        }
    }
}
