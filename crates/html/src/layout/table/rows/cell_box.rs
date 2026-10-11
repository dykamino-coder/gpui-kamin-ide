//! Коробка ячейки из сырого стиля: inherit-отступы и рамки, полкромки сросшейся модели, cellpadding.

use crate::dom::Element;
use crate::layout::table::{collapsed_cell_edge, table_spanning_size};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
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
) -> Option<(
    [f32; 4],
    [crate::style::values::value::Color; 4],
    [u8; 4],
    u32,
)> {
    // `padding: inherit` и `border: inherit` решаются только в СЛИТОМ
    // стиле (`cm`, `inline.rs` ветки `padding_inherit`/`border_inherit*`),
    // а коробку ячейки, полкромки сросшейся модели и `cellpadding`
    // ниже строят из СЫРОГО стиля: там лежало умолчание `td {padding:
    // 1px}` вместо 5px ряда (CSS 2.1 §6.2.1 — значение родителя).
    // `row-margin-border-padding`, `row-group-margin-border-padding`:
    // все четыре стола с `.inherited` выходили меньше эталона.
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
        cell.style.border_colors = cm.border_colors;
        cell.style.border_color = cm.border_color;
        cell.style.border_dashed = cm.border_dashed;
        cell.style.border_dotted = cm.border_dotted;
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ: держать внутри ячейки ПОЛОВИНУ её кромки
    // прозрачной рамкой, а внутри таблицы — половину своей (§17.6.2:
    // «row-width = (0.5 * border-width0) + padding-left1 + …», «the
    // width of the table includes half the table border»), заодно сняв
    // поправку `shift` у проб. Проба по 39 парам семей
    // `table-backgrounds-b[cs]-*`, `collapsing-border-model-*`,
    // `border-conflict-style-10*`: флипов ноль, все шесть `bc-*`
    // подтянулись (13.73 -> 12.10, 5.00 -> 4.01, 1.00 -> 0.81), но
    // потеряны `fixed-table-layout-027` (0.00 -> «красное видно») и
    // `collapsing-border-model-008` (0.00 -> 1.36). Половина берётся от
    // ПОБЕДИВШЕЙ кромки соседей (§17.6.2.1), а не от своей: без
    // разрешения ширин по всей сетке модель не сходится.
    //
    // Сросшиеся рамки (border-collapse): рамки С ЯЧЕЕК СНИМАЮТСЯ
    // целиком — их рисует отдельный слой кромок на линиях сетки
    // (см. interact::EdgePainter): кромка соседей ОДНА, рисуется
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
    // Презентационный `cellpadding="N"` таблицы: хинт ниже авторского
    // padding, но выше умолчания браузера `td { padding: 1px }` —
    // применяется, только когда у ячейки ровно оно.
    if let Some(pad) = e
        .attr("cellpadding")
        .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    {
        let ua = |l: Option<Len>| matches!(l, Some(Len::Px(1.0)));
        let pd = &cell.style.padding;
        if ua(pd.top) && ua(pd.right) && ua(pd.bottom) && ua(pd.left) {
            let v = Some(Len::Px(pad));
            cell.style.padding = crate::style::computed::Sides {
                top: v,
                right: v,
                bottom: v,
                left: v,
            };
        }
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
    if matches!(cell.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
        cell.style.width = None;
    }
    // Кегльная ширина уходит с коробки, как Px и доля: колонка
    // разрешает её сама (Em-ветка col_widths), а на коробке она
    // падала в запасной кегль 16px — ячейка `width: 2em` при шрифте
    // 50px сжималась до 32 точек, и прижим строк оставался без места
    // (table-cell-valign-003-ref). Ортогональную ячейку не трогаем:
    // её width — логический inline-size, он ниже перекладывается в
    // высоту.
    if !(cell.style.vertical == Some(true)
        && e.style.vertical != Some(true)
        && cell.style.width_from_inline)
        // У ВЕРТИКАЛЬНОЙ таблицы width остаётся коробке: дорожку
        // задаёт высота (table-cell-align-001/002).
        && !table_is_vertical
        && matches!(
            cell.style.width,
            Some(Len::Em(_)) | Some(Len::Ch(_)) | Some(Len::Ex(_))
        )
    {
        cell.style.width = None;
    }
    // У ВЕРТИКАЛЬНОЙ таблицы кегльная ширина ячейки остаётся на
    // коробке, но в точки её никто не переводил: `apply` добавляет
    // отступы только к `Len::Px`, и коробка выходила у́же дорожки на
    // свои отступы и рамки. Перекладываем в минимум, зеркально
    // правилу `height` -> `min_height` ниже.
    if table_is_vertical
        && !cell.style.width_from_inline
        && let Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) = cell.style.width
    {
        let size = match cell.style.font_size.or(row.style.font_size) {
            Some(Len::Px(v)) => v,
            _ => table_font,
        };
        let family = cell
            .style
            .font_family
            .clone()
            .unwrap_or_else(|| table_family.to_string());
        let v = crate::text::metrics::spacing_px(Some(l), &family, size);
        if v > 0.0 {
            cell.style.width = None;
            cell.style.min_width = Some(Len::Px(v));
        }
    }
    // Письмо к рядам и группам рядов не применяется (css-writing-modes
    // §applies), а РАЗМЕЩЕНИЕ ячеек в решётке всегда ведёт письмо
    // таблицы — оно уже посчитано табличным кодом. Собственное письмо
    // ячейки остаётся: оно законно управляет её СОДЕРЖИМЫМ
    // (ортогональные ячейки, table-cell-align-002).
    cell_edge
}
