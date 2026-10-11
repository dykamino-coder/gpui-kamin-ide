//! Колонки нулевой ширины: дорожки, которые не держат отступы и рамки ячеек.

use super::is_cell;
use crate::dom::{Element, Node};
use crate::layout::block::containing::CB_WIDTH;
use crate::style::values::value::Len;

pub(super) fn zero_width_cols(
    e: &Element,
    cols: u16,
    row_elements: &Vec<&Element>,
    from_cols: &[Option<f32>],
) -> Vec<bool> {
    let zero_cols: Vec<bool> = {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        // Ширина стола ДОЛЕЙ решается от содержащего блока: `width: 80%` в
        // шестистах сорока — это 512 (`fixed-table-layout-023`).
        let своя_ширина = match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
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
                let base = tw - side(tbz.left) - side(tbz.right) - gap * (f32::from(cols) + 1.0);
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
                        let своя = match cell.style.width {
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
                            *slot = Some(
                                w + side(cell.style.padding.left)
                                    + side(cell.style.padding.right)
                                    + side(b.left)
                                    + side(b.right),
                            );
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
