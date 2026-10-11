//! Дорожки колонок таблицы: ширины первого ряда и список дорожек сетки.

use crate::dom::{Element, Node};
use crate::layout::block::containing::CB_WIDTH;
use crate::layout::table::columns::track_list_collapsed;
use crate::layout::table::is_cell;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn table_tracks(
    e: &Element,
    inherited: &Computed,
    row_elements: &Vec<&Element>,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_font: f32,
    table_family: &str,
    spacing: (f32, f32),
    cols: u16,
    from_cols: &[Option<f32>],
    mut col_widths: Vec<(Option<f32>, Option<f32>)>,
    cols_collapsed: Vec<bool>,
    cols_pct: Vec<Option<f32>>,
) -> (Vec<Option<f32>>, Vec<gpui::GridTrack>, bool) {
    let first_row_widths = first_row_widths_of(
        e,
        row_elements,
        win_edges,
        table_font,
        table_family,
        spacing,
        cols,
    );
    // `<col>`-ширины старше ячеек первого ряда (§17.5.2.1) и действуют и в
    // авто-раскладке: колонка с шириной держит её (как ширина ячейки).
    for (i, w) in from_cols.iter().enumerate() {
        if let (Some(w), Some(slot)) = (w, col_widths.get_mut(i)) {
            slot.0 = Some(slot.0.map_or(*w, |old| old.max(*w)));
        }
    }
    let first_row_widths: Vec<Option<f32>> = (0..cols as usize)
        .map(|i| {
            from_cols
                .get(i)
                .copied()
                .flatten()
                .or_else(|| first_row_widths.get(i).copied().flatten())
        })
        .collect();
    let tracks = track_list_collapsed(
        cols,
        e.style.table_fixed == Some(true),
        &first_row_widths,
        &col_widths,
        &cols_collapsed,
        &cols_pct,
        // Место под КОЛОНКИ, а не вся ширина стола: §17.5.2.1 считает долю
        // от ширины таблицы БЕЗ её рамок и без зазоров между ячейками
        // (`fixed-table-layout-022` расписывает это прямо в тексте: 533 − 58
        // рамок − 75 зазоров = 400).
        match match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        } {
            Some(v) => {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(w)) => w,
                    _ => 0.0,
                };
                let bs = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                Some(v - side(bs.left) - side(bs.right) - gap * (cols as f32 + 1.0))
            }
            _ => None,
        },
    );
    // Таблица ЗАДАННОЙ высоты раздаёт лишнее место рядам БЕЗ своей высоты
    // (CSS 2.1 §17.5.3): ряд с высотой (своей или ячеек) держит её, остальные
    // делят остаток. Без этого средний ряд решётки 64/auto/64 в таблице 224px
    // схлопывался по содержимому, и вся середина уезжала.
    // Считается и от min-height: минимум так же растягивает таблицу, и
    // остаток обязан достаться безвысотным рядам.
    let table_tall = matches!(e.style.height, Some(Len::Px(_)))
        || matches!(e.style.min_height, Some(Len::Px(_)))
        // Доля высоты, которая РАЗРЕШАЕТСЯ (содержащий блок определён), —
        // такая же заданная высота: css-tables-3 «a 'height' property with a
        // value other than auto … will eventually be distributed to the height
        // of the rows». Неразрешимая доля — это `auto` (CSS 2.1 §10.5), её не
        // берём. Растянутую раскладкой коробку (элемент гибкого контейнера или
        // сетки со своей долей) тоже не берём: там доля в taffy падает в `auto`,
        // а ряды-доли в неопределённой высоте выравниваются по самому высокому.
        // Эталоны `fr-unit-ref`, `display-grid-ref`: стол `height:100%` в
        // абсолюте 400×100 держал ряды по тексту вместо 30/70.
        || (matches!(e.style.height, Some(Len::Pct(_)))
            && inherited.cb_height_def
            && !inherited.stretched);
    (first_row_widths, tracks, table_tall)
}

pub(super) fn first_row_widths_of(
    e: &Element,
    row_elements: &Vec<&Element>,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_font: f32,
    table_family: &str,
    spacing: (f32, f32),
    cols: u16,
) -> Vec<Option<f32>> {
    let first_row_widths: Vec<Option<f32>> = row_elements
        .first()
        .map(|row| {
            let mut out = vec![];
            for c in &row.children {
                if let Node::Element(cell) = c
                    && is_cell(cell)
                {
                    let span = cell
                        .attr("colspan")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    // Колонка = width + горизонтальные паддинги и рамки
                    // ячейки (§17.5.2.1, content-box); в сросшейся модели
                    // рамка входит ПОЛОВИНОЙ.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(p)) => p,
                        _ => 0.0,
                    };
                    let extra = if cell.style.border_box == Some(true) {
                        0.0
                    } else {
                        let b = cell.style.borders();
                        let border = side(b.left) + side(b.right);
                        side(cell.style.padding.left)
                            + side(cell.style.padding.right)
                            + if e.style.border_collapse == Some(true) {
                                // Половина ПОБЕДИВШЕЙ линии — та же, что
                                // легла в паддинг ячейки (§17.6.2.1).

                                win_edges
                                    .get(&cell.node_id)
                                    .map(|w| (w[1] + w[3]) / 2.0)
                                    .unwrap_or(border / 2.0)
                            } else {
                                border
                            }
                    };
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разрешать ширину ячейки в
                    // единицах ШРИФТА (`width: 1em` доезжает сюда
                    // неразрешённой — `resolve_em` живёт в наследовании, а
                    // ширины колонок считаются раньше). Срез из 10 пар
                    // `separated-border-model-*`: 8 зелёных до и после, ни
                    // один вердикт не сдвинулся. Одной этой половины мало:
                    // цель (`-004c/-004d`) требует ещё нижней грани ширины
                    // стола по сумме дорожек — возвращать вместе с ней.
                    match cell.style.width {
                        Some(Len::Px(v)) if span == 1 => out.push(Some(v + extra)),
                        // Ширина в единицах шрифта — кеглем САМОЙ ячейки
                        // (ряд → таблица): без неё колонка `width: 1em`
                        // уходила в безразмерные, и пол ширины стола ниже
                        // (§17.5.2.1) не складывался (`separated-border-model-004c`).
                        Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) if span == 1 => {
                            let size = super::super::col_measure::cell_font_px(
                                cell.style.font_size,
                                row.style.font_size,
                                table_font,
                            );
                            let family = cell
                                .style
                                .font_family
                                .clone()
                                .unwrap_or_else(|| table_family.to_string());
                            let v = crate::text::metrics::spacing_px(Some(l), &family, size);
                            out.push((v > 0.0).then_some(v + extra));
                        }
                        // Доля считается от места, отдаваемого дорожкам:
                        // ширина таблицы за вычетом зазоров (§17.5.2.1,
                        // «a percentage value ... of the table width»).
                        // Известна, только когда ширина таблицы в точках.
                        Some(Len::Pct(p)) if span == 1 => match e.style.width {
                            Some(Len::Px(tw)) => {
                                let gaps = spacing.0 * (f32::from(cols) + 1.0);
                                out.push(Some((tw - gaps).max(0.0) * p + extra));
                            }
                            _ => out.push(None),
                        },
                        _ => out.extend(std::iter::repeat_n(None, span)),
                    }
                }
            }
            out
        })
        .unwrap_or_default();
    first_row_widths
}
