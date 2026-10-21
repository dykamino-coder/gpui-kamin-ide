//! Заявленный блочный размер ячейки в вертикальной таблице — минимум ряда, независимый от
//! строчных дорожек: `width` переезжает в `min-width`. CSS Tables 3
//! §computing-row-measures; Blink `table_layout_utils.cc` берёт логический блочный размер ячейки.
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn vertical_block_floor(
    style: &mut Computed,
    font: f32,
    family: &str,
    vcx: crate::document::state::ValueCx<'_>,
) {
    if style.width_from_inline {
        return;
    }
    let width = match style.width {
        Some(Len::Px(width)) => width,
        Some(length @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
            crate::text::metrics::spacing_px(Some(length), family, font, vcx)
        }
        _ => return,
    };
    style.width = None;
    style.min_width = Some(Len::Px(match style.min_width {
        Some(Len::Px(minimum)) => width.max(minimum),
        _ => width,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_vertical_cells_preserve_declared_block_size_and_larger_minima() {
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
        let mut cell = Computed {
            width: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        vertical_block_floor(&mut cell, 16.0, "", vcx);
        assert_eq!(cell.width, None);
        assert_eq!(cell.min_width, Some(Len::Px(100.0)));
        cell.width = Some(Len::Px(50.0));
        vertical_block_floor(&mut cell, 16.0, "", vcx);
        assert_eq!(cell.min_width, Some(Len::Px(100.0)));
        cell.width = Some(Len::Px(150.0));
        cell.width_from_inline = true;
        vertical_block_floor(&mut cell, 16.0, "", vcx);
        assert_eq!(cell.width, Some(Len::Px(150.0)));
        assert_eq!(cell.min_width, Some(Len::Px(100.0)));
    }
}
