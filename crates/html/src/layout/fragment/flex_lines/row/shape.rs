//! Формы флекс-строк ряда: поперечный размер и вырезы элементов.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::fragment_size::shape_full;
use crate::style::values::value::Len;

// The shape result pairs each line with its height and break flags.
#[allow(clippy::type_complexity)]
pub(crate) fn shape_flex_lines(
    row_gap: f32,
    side: impl Fn(&Option<Len>) -> Option<f32>,
    lines: &mut [Vec<(f32, Element, crate::layout::fragment::Shape)>],
    vcx: ValueCx<'_>,
) -> Option<()> {
    for (li, line) in lines.iter_mut().enumerate() {
        let cross = line
            .iter()
            .map(|x| x.2.height + x.2.margin_top + x.2.margin_bottom)
            .fold(0.0f32, f32::max);
        let (bf, ba) = (
            line.iter().any(|x| edge_break(&x.1, false, vcx)),
            line.iter().any(|x| edge_avoid(&x.1, false, vcx)),
        );
        let (af, aa) = (
            line.iter().any(|x| edge_break(&x.1, true, vcx)),
            line.iter().any(|x| edge_avoid(&x.1, true, vcx)),
        );
        let m = line.len();
        for (ii, (_, e, sh)) in line.iter_mut().enumerate() {
            // `height: auto` тянется на строку — полом.
            if matches!(e.style.cascaded().height, None | Some(Len::Auto))
                && sh.height + sh.margin_top + sh.margin_bottom < cross - 0.01
            {
                let eb = e.style.cascaded().borders();
                let edges = side(&e.style.cascaded().padding.top).unwrap_or(0.0)
                    + side(&e.style.cascaded().padding.bottom).unwrap_or(0.0)
                    + side(&eb.top).unwrap_or(0.0)
                    + side(&eb.bottom).unwrap_or(0.0);
                let content = (cross - sh.margin_top - sh.margin_bottom - edges).max(0.0);
                e.style.cascaded_mut().min_height = Some(Len::Px(content));
                *sh = shape_full(e, 4, ShapeCx::COLUMNS, vcx)?;
            }
            // Зазор между строками — к полю элементов следующей строки.
            if li > 0 {
                sh.margin_top += row_gap;
            }
            if ii == 0 {
                e.style.cascaded_mut().break_before_force |= bf;
                e.style.cascaded_mut().break_before_avoid |= ba && !bf;
            }
            if ii + 1 == m {
                e.style.cascaded_mut().break_after_force |= af;
                e.style.cascaded_mut().break_after_avoid |= aa && !af;
            }
        }
    }
    Some(())
}
