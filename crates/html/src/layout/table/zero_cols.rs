//! Колонки нулевой ширины: дорожки, которые не держат отступы и рамки ячеек.

use super::is_cell;
use crate::dom::{Element, Node};
use crate::style::values::value::{Len, px_strict};

pub(super) fn zero_width_cols(
    e: &Element,
    cols: u16,
    row_elements: &Vec<&Element>,
    from_cols: &[Option<f32>],
    cb_px: Option<f32>,
) -> Vec<bool> {
    let vertical = e.style.vertical == Some(true);
    let axes = super::axes::Axes::new(&e.style);
    let zero_cols: Vec<bool> = {
        let px_of = px_strict;
        // Ширина стола ДОЛЕЙ решается от содержащего блока: `width: 80%` в
        // шестистах сорока — это 512 (`fixed-table-layout-023`).
        let своя_ширина = match if vertical {
            e.style.height
        } else {
            e.style.width
        } {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) if !vertical => cb_px.filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        };
        match (e.style.table_fixed == Some(true), своя_ширина) {
            (true, Some(tw)) => {
                let side = |l: Option<Len>| px_of(l).unwrap_or(0.0);
                let tbz = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                let tb = axes.logical([
                    side(tbz.top),
                    side(tbz.right),
                    side(tbz.bottom),
                    side(tbz.left),
                ]);
                let base = tw - tb[1] - tb[3] - gap * (f32::from(cols) + 1.0);
                let mut declared = vec![None::<f32>; cols as usize];
                for (i, w) in from_cols.iter().enumerate() {
                    if let (Some(w), Some(slot)) = (w, declared.get_mut(i)) {
                        *slot = Some(*w);
                    }
                }
                if let Some(row) = row_elements.first() {
                    let mut i = 0usize;
                    for c in &row.children {
                        let Node::Element(cell) = c else { continue };
                        if !is_cell(cell) {
                            continue;
                        }
                        let span = cell
                            .attr("colspan")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1);
                        // Доля ячейки первого ряда — тоже ЗАЯВЛЕННАЯ
                        // ширина: `width: 50%` при столе в сто точках это
                        // пятьдесят, и вместе со своим отступом дорожка
                        // забирает всё место (`fixed-table-layout-025`).
                        let своя = match if vertical {
                            cell.style.height
                        } else {
                            cell.style.width
                        } {
                            Some(Len::Px(v)) => Some(v),
                            Some(Len::Pct(k)) => Some(base.max(0.0) * k),
                            _ => None,
                        };
                        if span == 1
                            && let Some(w) = своя
                            && let Some(slot) = declared.get_mut(i)
                            && slot.is_none()
                        {
                            let b = cell.style.borders();
                            let p = &cell.style.padding;
                            let extra = axes.logical([
                                side(p.top) + side(b.top),
                                side(p.right) + side(b.right),
                                side(p.bottom) + side(b.bottom),
                                side(p.left) + side(b.left),
                            ]);
                            *slot = Some(w + extra[1] + extra[3]);
                        }
                        i += span;
                    }
                }
                let sum: f32 = declared.iter().flatten().sum();
                let свободно = base - sum;
                (0..cols as usize)
                    .map(|i| declared.get(i).copied().flatten().is_none() && свободно <= 0.5)
                    .collect()
            }
            _ => vec![false; cols as usize],
        }
    };
    zero_cols
}
