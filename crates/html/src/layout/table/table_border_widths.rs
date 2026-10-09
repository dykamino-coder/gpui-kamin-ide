//! Resolve collapsed border widths before measuring table cells and tracks.
//! CSS 2.1 §17.6.2 centers each winning edge on a grid line.

use crate::dom::{Element, Node};
use crate::layout::table::{is_cell, row_span_in_group};
use crate::style::values::value::Len;
mod structural;
use std::collections::HashMap;

pub(crate) fn resolve(
    e: &Element,
    row_elements: &[&Element],
    groups: &[Option<&Element>],
    rows_left: &[usize],
    cols: u16,
    table_font: f32,
    table_family: &str,
) -> (HashMap<u64, [f32; 4]>, [f32; 4]) {
    let collapse_cells_pre = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    let px_of_pre = |l: Option<Len>| crate::text::metrics::spacing_px(l, &table_family, table_font);
    let tb = e.style.borders();
    let bw_pre = [
        px_of_pre(tb.top),
        px_of_pre(tb.right),
        px_of_pre(tb.bottom),
        px_of_pre(tb.left),
    ];
    let mut win_edges: std::collections::HashMap<u64, [f32; 4]> = Default::default();
    // Наружные полуширины таблицы: победитель на ЕЁ линиях. Копится прямо в
    // условиях края — свод по всем клеткам затягивал сюда и внутренние линии,
    // и таблица без рамки получала паддинг от кромок середины
    // (`fixed-table-layout-027`: крайние дорожки схлопывались в ноль).
    let mut outer_win = [0.0f32; 4];
    let structural = structural::Edges::new(
        e,
        row_elements,
        groups,
        cols as usize,
        table_font,
        table_family,
    );
    if collapse_cells_pre {
        struct Cel {
            r: usize,
            c: usize,
            sr: usize,
            sc: usize,
            w: [f32; 4],
            id: u64,
        }
        let mut cels: Vec<Cel> = vec![];
        let mut occ: Vec<u16> = vec![0; cols as usize];
        let mut r = 0usize;
        for row in row_elements {
            for slot in occ.iter_mut() {
                *slot = slot.saturating_sub(1);
            }
            let mut c = 0usize;
            for child in &row.children {
                let Node::Element(cell) = child else { continue };
                if !is_cell(cell) {
                    continue;
                }
                while c < occ.len() && occ[c] > 0 {
                    c += 1;
                }
                let sc = cell
                    .attr("colspan")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                // Охват урезан до конца группы — тот же, что ляжет в сетку
                // (см. `rows_left`), иначе полуширины разбирались бы с
                // соседом, которого у ячейки нет.
                let sr = row_span_in_group(cell, rows_left.get(r).copied().unwrap_or(1));
                let b = cell.style.borders();
                let own = [
                    px_of_pre(b.top),
                    px_of_pre(b.right),
                    px_of_pre(b.bottom),
                    px_of_pre(b.left),
                ];
                let extra = structural.cell(r, c, sr, sc);
                cels.push(Cel {
                    r,
                    c,
                    sr,
                    sc,
                    w: std::array::from_fn(|i| own[i].max(extra[i])),
                    id: cell.node_id,
                });
                for k in c..(c + sc).min(occ.len()) {
                    occ[k] = occ[k].max(sr as u16);
                }
                c += sc;
            }
            r += 1;
        }
        let rows_n = r;
        for a in cels.iter() {
            let mut w = a.w;
            for b in cels.iter() {
                if b.id == a.id {
                    continue;
                }
                let cols_over = a.c < b.c + b.sc && b.c < a.c + a.sc;
                let rows_over = a.r < b.r + b.sr && b.r < a.r + a.sr;
                if cols_over && b.r + b.sr == a.r {
                    w[0] = w[0].max(b.w[2]);
                }
                if cols_over && a.r + a.sr == b.r {
                    w[2] = w[2].max(b.w[0]);
                }
                if rows_over && b.c + b.sc == a.c {
                    w[3] = w[3].max(b.w[1]);
                }
                if rows_over && a.c + a.sc == b.c {
                    w[1] = w[1].max(b.w[3]);
                }
            }
            // Внешние линии спорят с рамкой самой таблицы.
            if a.r == 0 {
                w[0] = w[0].max(bw_pre[0]);
                outer_win[0] = outer_win[0].max(w[0]);
            }
            if a.c == 0 {
                w[3] = w[3].max(bw_pre[3]);
                outer_win[3] = outer_win[3].max(w[3]);
            }
            if a.c + a.sc >= cols as usize {
                w[1] = w[1].max(bw_pre[1]);
                outer_win[1] = outer_win[1].max(w[1]);
            }
            if a.r + a.sr >= rows_n {
                w[2] = w[2].max(bw_pre[2]);
                outer_win[2] = outer_win[2].max(w[2]);
            }
            win_edges.insert(a.id, w);
        }
    }
    let outer_win = [
        outer_win[0].max(bw_pre[0]),
        outer_win[1].max(bw_pre[1]),
        outer_win[2].max(bw_pre[2]),
        outer_win[3].max(bw_pre[3]),
    ];
    (win_edges, outer_win)
}
