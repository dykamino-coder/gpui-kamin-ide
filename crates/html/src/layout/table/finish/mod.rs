//! Хвост раскладки таблицы: сетка ячеек, дорожки, рамка и подписи (table_finish).

use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::edge_set;
use crate::style::computed::Computed;
use gpui::{AnyElement, Styled};
mod outer;
use outer::{finish_table_outer, table_outer_box};
mod tracks;
use tracks::table_tracks;
mod floors;
use floors::{table_grid_child, table_size_floors};
mod grid_box;
use grid_box::{paint_table_border, table_grid_box};
mod captions;
use captions::{collect_captions, wrap_with_captions};

use super::context::{RowsOut, TableGeom, TableLayers};

pub(super) fn table_finish(
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    out: &mut RowsOut,
    opts: &RenderOpts,
) -> AnyElement {
    let px_of = |l| geom.px_of(l, opts);
    let vcx = opts.cx.vcx();
    let mut grid_children = std::mem::take(&mut out.under);
    if layers.paint_layers {
        grid_children
            .push(crate::layout::table::paint::CellBgPainter::new(out.cell_bgs.clone()).hooked());
    }
    grid_children.append(&mut out.cells);
    if layers.paint_layers {
        grid_children
            .push(crate::layout::table::paint::EdgePainter::new(out.table_edges.clone()).hooked());
    }
    grid_children.append(&mut out.cells_over);
    let cells = grid_children;
    // Заголовок таблицы живёт ВНЕ коробки таблицы (CSS 2.1 §17.4:
    // анонимная обёртка держит заголовок и коробку) — рамка и обрезка
    // таблицы его не трогают; `caption-side: bottom` ставит его под сетку.
    let mut caps_top: Vec<AnyElement> = Vec::new();
    let mut caps_bot: Vec<AnyElement> = Vec::new();
    collect_captions(geom.e, geom.inherited, opts, &mut caps_top, &mut caps_bot);

    // Оси таблицы ЛОГИЧЕСКИЕ, как и у сетки: колонки идут вдоль строки. При
    // вертикальном письме строка идёт сверху вниз, и дорожки колонок
    // становятся физическими рядами. Своей ветки у таблицы не было, и её
    // сетка строилась физической — мимо уже переставленных осей.
    // Ширины колонок фиксированной раскладки — из ПЕРВОГО ряда
    // (CSS 2.1 §17.5.2.1): ячейка с шириной держит её, остальные делят
    // остаток поровну.
    let (first_row_widths, tracks, table_tall) = table_tracks(geom, opts);
    // Не строить дорожки рядов без `table_tall`: ряд без заданной высоты
    // обязан остаться авто-дорожкой там, где стол не растянут
    // (`margin-applies-to-001…007`, `margin-bottom-applies-to-001…007`,
    // `table-cell-overflow-explicit-height-001/002`,
    // `percentage-sizing-of-table-cell-children-004`,
    // `subpixel-table-cell-height-001`).
    let grid_box = table_grid_box(
        geom.e,
        geom.inherited,
        &geom.row_elements,
        tracks,
        table_tall,
        vcx,
    );
    // Сросшиеся рамки: у таблицы нет паддинга, а кромка между её рамкой и
    // краевыми ячейками одна — ячейки накрывают ВНУТРЕННЮЮ ПОЛОВИНУ рамки
    // (CSS 2.1 §17.6.2). Рамка при этом рисуется ПОВЕРХ фонов ячеек, как и
    // все сросшиеся кромки: обычная рамка коробки красится под детьми и
    // закрашивалась бы их фоном. Поэтому у самой коробки рамка снимается,
    // её место держит паддинг, сетка выезжает на его половину, а красит
    // рамку кольцевой квад ПОСЛЕ сетки.
    let collapse = geom.e.style.border_collapse == Some(true)
        || (geom.e.style.border_collapse.is_none() && geom.e.attr("rules").is_some());
    // Минимумы таблицы меряются ПОЛНОЙ коробкой с рамкой и паддингом
    // (css-tables-3 §computing-the-table-height, CSSWG #5336): пороги
    // пересчитываются в контентные, компенсацию вернёт общий слой.
    let (pad_px, min_h, min_w, table_border_box, fixed_floor) = table_size_floors(
        geom.e,
        geom.spacing,
        geom.cols,
        geom.bw,
        px_of,
        first_row_widths,
        collapse,
    );
    // §17.4: `position` и края — свойства ОБЁРТКИ таблицы, а не её сетки;
    // ширину сетки решает §17.5.2.2 (сжатие по содержимому). Пока коробка
    // одна, абсолютная таблица с ОБОИМИ краями инлайн-оси получала ширину от
    // краёв, и колонки расползались: сжатие у нас выражено только
    // `align_self`, а его у абсолютной коробки с двумя краями не спрашивают.
    let split_wrapper = matches!(
        geom.inherited.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) && edge_set(geom.inherited.inset.left)
        && edge_set(geom.inherited.inset.right)
        && geom.e.style.width.is_none();
    // Стиль СЕТКИ — без позиционирования и краёв: их заберёт обёртка.
    let grid_style = split_wrapper.then(|| {
        let mut c = geom.inherited.clone();
        c.position = None;
        c.inset = Default::default();
        c.z_index = None;
        c
    });
    let inherited: &Computed = grid_style.as_ref().unwrap_or(geom.inherited);
    let needs_clone = collapse
        || table_border_box
        || min_h != geom.e.style.min_height
        || min_w != geom.e.style.min_width
        || fixed_floor.is_some();
    let mut outer = table_outer_box(
        geom.e,
        geom.bw,
        geom.outer_win,
        collapse,
        pad_px,
        min_h,
        min_w,
        table_border_box,
        fixed_floor,
        inherited,
        needs_clone,
        &opts.cx,
    );
    // Таблица без заданной ширины СЖИМАЕТСЯ по содержимому, а не растягивается
    // на родителя (CSS 2.1 §17.5.2, shrink-to-fit). Пока она растягивалась,
    // две короткие колонки разъезжались к противоположным краям — видно на
    // `shaping-tatweel-002`, где одинаковые знаки стояли по краям окна.
    //
    // Заданный `align-self` приём отменяет: у элемента СЕТКИ эта ось —
    // блочная (css-align-3 §6.2, `taffy: grid/alignment.rs:145`), сжатие по
    // строчной оси ведёт `justify-self`, и прижим к началу здесь только
    // отбирал у стола высоту дорожки. Метку ставит
    // `dom::grid_table_items_keep_stretch`; авторский `align-self` она же и
    // пропускает вперёд.
    if geom.e.style.width.is_none() && geom.e.style.align_self.is_none() {
        outer.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    // Пол GRIDMIN (css-tables-3 §3.9): гибкая раскладка не ужимает стол по
    // главной оси ниже min-content его решётки — `vendor/taffy` `flexbox.rs`,
    // признак `item_is_table` (`table-as-item-auto-min-width`, `-wide-content`).
    outer.style().item_is_table = Some(true);
    // Table baselines come from rows, never the empty grid/caption shim.
    outer.style().baseline_unavailable = Some(!geom.have_rows);
    // An enclosing `inline-block` takes no baseline from a table at any block
    // depth (CSS 2.1 §10.8.1 counts line boxes only; Blink
    // `block_layout_algorithm.cc` `PropagateBaselineFromBlockChild`: "table's
    // don't contribute any baselines"). The wrappers below carry the same mark.
    outer.style().no_inline_block_baseline = Some(true);
    // КОРНЕВОЙ стол (`<html display: table>`): родитель — блок стенда, где
    // `align-self` не работает, и стол растягивался на всё окно. Гибкая
    // обёртка возвращает сжатие по содержимому и центрирование `margin: auto`.
    let root_table = matches!(geom.e.tag.as_str(), "html" | "body") && geom.e.style.width.is_none();
    // `border-spacing` задан по ЛОГИЧЕСКИМ осям таблицы: первое значение —
    // зазор между КОЛОНКАМИ (инлайн-ось), второе — между РЯДАМИ (блочная
    // ось). css-writing-modes-3 §7.2 «Dimension Mapping» переносит их на
    // физические оси письмом ТАБЛИЦЫ: в вертикальном письме инлайн-ось
    // вертикальна, поэтому первое значение становится физическим
    // ВЕРТИКАЛЬНЫМ зазором, второе — горизонтальным. Решётка выше уже
    // транспонирована (`grid_box`: дорожки колонок легли в
    // `grid_template_rows`, ряды пошли колонками), а зазоры оставались
    // физическими — стол выходил перекошенным зеркально
    // (`border-spacing-vrl-002`: 160.0×70.0 вместо 100×100). Гейт — тот же
    // `e.style.vertical`, что и у решётки: иначе они разъедутся.
    // Стол БЕЗ рядов не несёт блочных зазоров, без рядов и колонок — и
    // строчных: пустой `<table>` у Blink/Gecko — только рамки. У нас
    // оставалась коробка 4×4 (умолчание 2px), и в
    // `caption-relative-positioning` между двумя подписями светила красная
    // полоса. Столы с рядами не меняются.
    let mut outer = table_grid_child(
        &out.table_edges,
        geom.e,
        geom.spacing,
        &geom.from_cols,
        geom.bw,
        geom.have_rows,
        cells,
        grid_box,
        collapse,
        outer,
    );
    outer = paint_table_border(
        out.table_edges.clone(),
        geom.e,
        geom.bw,
        geom.outer_win,
        collapse,
        inherited,
        outer,
    );
    let shrink_wrap = root_table
        || (geom.e.style.width.is_none()
            // Заданная высота или её порог приходят от РАСКЛАДКИ родителя:
            // обёртка рвёт эту связь (без отсечки ломается
            // `min-height-table-2`).
            && geom.e.style.height.is_none()
            && geom.e.style.min_height.is_none()
            && !inherited.stretched
            && geom.e.style.flex_basis.is_none()
            && geom.e.style.align_self.is_none()
            && geom.e.style.grid_col.is_none()
            && geom.e.style.grid_row.is_none());
    finish_table_outer(
        geom.e,
        caps_top,
        caps_bot,
        split_wrapper,
        inherited,
        root_table,
        outer,
        shrink_wrap,
        vcx,
    )
}
