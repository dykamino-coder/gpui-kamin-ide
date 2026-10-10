//! Формы флекс-строк ряда: поперечный размер и вырезы элементов.

use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::fragment_size::shape_full;
use crate::style::values::value::Len;

pub(crate) fn shape_flex_lines(
    row_gap: f32,
    side: impl Fn(&Option<Len>) -> Option<f32>,
    lines: &mut [Vec<(
        f32,
        Element,
        (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
    )>],
) -> Option<()> {
    for (li, line) in lines.iter_mut().enumerate() {
        let cross = line
            .iter()
            .map(|x| x.2.0 + x.2.1 + x.2.2)
            .fold(0.0f32, f32::max);
        let (bf, ba) = (
            line.iter().any(|x| edge_break(&x.1, false)),
            line.iter().any(|x| edge_avoid(&x.1, false)),
        );
        let (af, aa) = (
            line.iter().any(|x| edge_break(&x.1, true)),
            line.iter().any(|x| edge_avoid(&x.1, true)),
        );
        let m = line.len();
        for (ii, (_, e, sh)) in line.iter_mut().enumerate() {
            // `height: auto` тянется на строку — полом.
            if matches!(e.style.height, None | Some(Len::Auto)) && sh.0 + sh.1 + sh.2 < cross - 0.01
            {
                let eb = e.style.borders();
                let edges = side(&e.style.padding.top).unwrap_or(0.0)
                    + side(&e.style.padding.bottom).unwrap_or(0.0)
                    + side(&eb.top).unwrap_or(0.0)
                    + side(&eb.bottom).unwrap_or(0.0);
                let content = (cross - sh.1 - sh.2 - edges).max(0.0);
                e.style.min_height = Some(Len::Px(content));
                *sh = shape_full(e, 4, ShapeCx::COLUMNS)?;
            }
            // Зазор между строками — к полю элементов следующей строки.
            if li > 0 {
                sh.1 += row_gap;
            }
            if ii == 0 {
                e.style.break_before_force |= bf;
                e.style.break_before_avoid |= ba && !bf;
            }
            if ii + 1 == m {
                e.style.break_after_force |= af;
                e.style.break_after_avoid |= aa && !af;
            }
        }
    }
    Some(())
}
