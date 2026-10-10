//! Замер ширин колонок по ячейкам (min/max-content, проценты, охваты).

use super::{is_cell, row_span_in_group};
use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn measure_col_widths(
    e: &Element,
    inherited: &Computed,
    rows_left: &[usize],
    row_elements: &Vec<&Element>,
    col_widths: &mut [(Option<f32>, Option<f32>)],
    table_font: f32,
    table_family: String,
    mut busy: Vec<u16>,
    win_edges: &std::collections::HashMap<u64, [f32; 4]>,
) {
    for (ri, row) in row_elements.iter().enumerate() {
        let mut ix = 0usize;
        for slot in busy.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        for c in &row.children {
            let Node::Element(cell) = c else { continue };
            if !is_cell(cell) {
                continue;
            }
            while ix < busy.len() && busy[ix] > 0 {
                ix += 1;
            }
            let span = cell
                .attr("colspan")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            // Занятость колонок — по УРЕЗАННОМУ охвату (см. `rows_left`).
            let rspan = row_span_in_group(cell, rows_left.get(ri).copied().unwrap_or(1))
                .min(usize::from(u16::MAX)) as u16;
            for c2 in ix..(ix + span).min(busy.len()) {
                busy[c2] = rspan;
            }
            if span == 1 && ix < col_widths.len() {
                // Дорожку задаёт размер ячейки вдоль ИНЛАЙН-ОСИ ТАБЛИЦЫ.
                // Вертикальная таблица: ось вертикальна — дорожка из ВЫСОТЫ
                // ячейки (width остаётся её коробке, table-cell-align-002).
                // Горизонтальная: из ширины; у ортогональной ячейки
                // (вертикальное письмо в htb-таблице) `block-size` лёг в
                // height (размеры при вертикали не переставляются, см.
                // resolve_logical) — он и задаёт колонку.
                let table_vertical =
                    e.style.vertical == Some(true) || inherited.vertical == Some(true);
                let orthogonal = cell.style.vertical == Some(true) && !table_vertical;
                let source = if table_vertical || orthogonal {
                    // У ортогональной ячейки width несёт ЛОГИЧЕСКИЙ
                    // inline-size — физически это ВЫСОТА, не колонка
                    // (table-cell-align-005/006); колонку задаёт block-size,
                    // осевший в height.
                    cell.style.height
                } else {
                    cell.style.width
                };
                // ЗАМЕРЕНО: CSS2 5117 -> 5128, oldfront 2352 -> 2340. Двенадцать
                // потерянных — семья `css-writing-modes/table-progression-*`:
                // её эталон горизонтальный, а тест вертикальный, и слагаемые
                // ложатся по разным осям. Пробовали отсекать вертикальное
                // письмо (2339) и считать добавку только горизонтальной (2338)
                // — обе хуже. Возвращаться вместе с вертикальной табличной
                // раскладкой.
                //
                // Дорожка = `width` ячейки ПЛЮС её горизонтальные отступы и
                // рамки (§17.5.2.1, коробка содержимого); в сросшейся модели
                // рамка входит половиной. Та же формула стоит в ветке первого
                // ряда ниже; без неё колонка выходила у́же ячейки на её рамку
                // (`margin-applies-to-001..007`).
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(p)) => p,
                    _ => 0.0,
                };
                let extra = if cell.style.border_box == Some(true) {
                    0.0
                } else {
                    let b = cell.style.borders();
                    let border = if table_vertical || orthogonal {
                        side(b.top) + side(b.bottom)
                    } else {
                        side(b.left) + side(b.right)
                    };
                    let pad = if table_vertical || orthogonal {
                        side(cell.style.padding.top) + side(cell.style.padding.bottom)
                    } else {
                        side(cell.style.padding.left) + side(cell.style.padding.right)
                    };
                    // В сросшейся модели дорожка считает ПОЛОВИНУ победившей
                    // линии — ту же, что легла в паддинг ячейки; своя кромка
                    // могла быть у́же соседней.
                    pad + if e.style.border_collapse == Some(true) {
                        let w = win_edges.get(&cell.node_id).copied().unwrap_or([
                            side(cell.style.borders().top),
                            side(cell.style.borders().right),
                            side(cell.style.borders().bottom),
                            side(cell.style.borders().left),
                        ]);
                        if table_vertical || orthogonal {
                            (w[0] + w[2]) / 2.0
                        } else {
                            (w[1] + w[3]) / 2.0
                        }
                    } else {
                        border
                    }
                };
                match source {
                    Some(Len::Px(v)) => {
                        let v = v + extra;
                        let slot = &mut col_widths[ix].0;
                        *slot = Some(slot.map_or(v, |old| old.max(v)));
                    }
                    Some(Len::Pct(k)) => {
                        let slot = &mut col_widths[ix].1;
                        *slot = Some(slot.map_or(k, |old| old.max(k)));
                    }
                    // Шрифтовые единицы решаются кеглем САМОЙ ячейки
                    // (наследование row -> table): `td { width: 2em }` при
                    // `table { font: 50px }` — колонка 100px, не пропуск.
                    Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
                        let size = match cell
                            .style
                            .font_size
                            .or(row.style.font_size)
                            .or(inherited.font_size)
                        {
                            Some(Len::Px(v)) => v,
                            _ => table_font,
                        };
                        let family = cell
                            .style
                            .font_family
                            .clone()
                            .unwrap_or_else(|| table_family.clone());
                        let v = crate::text::metrics::spacing_px(Some(l), &family, size) + extra;
                        if v > 0.0 {
                            let slot = &mut col_widths[ix].0;
                            *slot = Some(slot.map_or(v, |old| old.max(v)));
                        }
                    }
                    _ => {}
                }
            }
            ix += span;
        }
    }
}
