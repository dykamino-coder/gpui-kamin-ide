//! Коробка ячейки из каскадированного стиля: inherit-отступы и рамки,
//! полкромки сросшейся модели.

use crate::dom::Element;
use crate::layout::table::{collapsed_cell_edge, table_spanning_size};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
// Cell preparation returns its element, geometry and border decoration together.
#[allow(clippy::type_complexity)]
pub(super) fn prepare_cell_box(
    e: &Element,
    cols_collapsed: &[bool],
    collapse_cells: bool,
    win_edges: &std::collections::HashMap<u64, [f32; 4]>,
    table_is_vertical: bool,
    table_font: f32,
    table_family: &str,
    px_of: &impl Fn(Option<Len>) -> f32,
    row: &Element,
    col_ix: usize,
    cm: &Computed,
    span_cols: u16,
    spans_collapsed: bool,
    clipped: bool,
    cell: &mut Element,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(
    [f32; 4],
    [crate::style::values::value::Color; 4],
    [u8; 4],
    u32,
)> {
    // Наследуемые отступы и рамки берутся из слитого стиля `cm`,
    // где `style::cascade::inherit` уже разрешил значения родителя.
    if cell.style.padding_inherit || cell.style.padding_inherit_side.contains(&true) {
        cell.style.padding = cm.padding;
    }
    if cell.style.border_inherit
        || cell.style.border_inherit_w.contains(&true)
        || cell.style.border_inherit_s.contains(&true)
        || cell.style.border_inherit_c.contains(&true)
    {
        cell.style.border_width = cm.border_width;
        cell.style.border_visible = cm.border_visible;
        cell.style.border_side_styles = cm.border_side_styles;
        cell.style.border_colors = cm.border_colors.clone();
        cell.style.border_dashed = cm.border_dashed;
        cell.style.border_dotted = cm.border_dotted;
    }
    // Не держать внутри ячейки ПОЛОВИНУ её кромки прозрачной рамкой, а
    // внутри таблицы — половину своей (§17.6.2: «row-width = (0.5 *
    // border-width0) + padding-left1 + …», «the width of the table
    // includes half the table border»): половина берётся от ПОБЕДИВШЕЙ
    // кромки соседей (§17.6.2.1), а не от своей, и без разрешения ширин
    // по всей сетке модель не сходится (`fixed-table-layout-027`,
    // `collapsing-border-model-008`).
    //
    // Сросшиеся рамки (border-collapse): рамки С ЯЧЕЕК СНИМАЮТСЯ
    // целиком — их рисует отдельный слой кромок на линиях сетки
    // (см. layout::table::paint::edge_painter::EdgePainter): кромка соседей ОДНА, рисуется
    // поверх фонов, и «шире побеждает» решается наложением.
    let cell_edge = if collapse_cells {
        collapsed_cell_edge(cell, cm, win_edges, px_of)
    } else {
        None
    };
    if clipped {
        cell.style.overflow_x = None;
        cell.style.overflow_y = None;
    }
    // Ячейка схлопнутой колонки не рисуется: колонка выброшена
    // (css-tables-3 §visibility-collapse-cell-rendering), её дорожка
    // нулевая, а краска ячейки торчала бы поверх соседей.
    if (col_ix..col_ix + span_cols as usize)
        .all(|i| cols_collapsed.get(i).copied().unwrap_or(false))
    {
        cell.style.hidden = Some(true);
    }
    table_spanning_size::preserve(
        &mut cell.style,
        cm,
        span_cols,
        e.style.table_fixed == Some(true),
        table_is_vertical,
        spans_collapsed,
    );
    let orthogonal_inline =
        cell.style.wm().vertical() && !e.style.wm().vertical() && cell.style.width_from_inline;
    if !table_is_vertical
        && !orthogonal_inline
        // A mixed percentage `calc()` is `auto` for the column (`col_measure`,
        // Blink `table_layout_utils`: calculated lengths with percentages
        // are treated as auto); left on the box it widened the table alone
        // (`calc-width-table-auto-1` row 4).
        && matches!(
            cell.style.width,
            Some(Len::Px(_)) | Some(Len::Pct(_)) | Some(Len::Calc(_))
        )
    {
        cell.style.width = None;
    }
    // Кегльная ширина уходит с коробки, как Px и доля: колонка
    // разрешает её сама (Em-ветка col_widths), а на коробке она
    // падала в запасной кегль 16px — ячейка `width: 2em` при шрифте
    // 50px сжималась до 32 точек, и прижим строк оставался без места
    // (table-cell-valign-003-ref). Ортогональную ячейку не трогаем:
    // её width — логический inline-size, он ниже перекладывается в
    // высоту.
    if !(cell.style.wm().vertical()
        && !e.style.wm().vertical()
        && cell.style.width_from_inline
        // У ВЕРТИКАЛЬНОЙ таблицы width остаётся коробке: дорожку
        // задаёт высота (table-cell-align-001/002).
        || table_is_vertical)
        && matches!(
            cell.style.width,
            Some(Len::Em(_)) | Some(Len::Ch(_)) | Some(Len::Ex(_))
        )
    {
        cell.style.width = None;
    }
    // The row's logical block size is physical width in a vertical table.
    // Preserve authored cell width as a minimum, then stretch to the row.
    if table_is_vertical {
        let size = crate::layout::table::col_measure::cell_font_px(
            cell.style.font_size,
            row.style.font_size,
            table_font,
        );
        let family = cell
            .style
            .font_family
            .clone()
            .unwrap_or_else(|| table_family.to_string());
        super::cell_size::vertical_block_floor(&mut cell.style, size, &family, vcx);
    }
    // Письмо к рядам и группам рядов не применяется (css-writing-modes
    // §applies), а РАЗМЕЩЕНИЕ ячеек в решётке всегда ведёт письмо
    // таблицы — оно уже посчитано табличным кодом. Собственное письмо
    // ячейки остаётся: оно законно управляет её СОДЕРЖИМЫМ
    // (ортогональные ячейки, table-cell-align-002).
    cell_edge
}
