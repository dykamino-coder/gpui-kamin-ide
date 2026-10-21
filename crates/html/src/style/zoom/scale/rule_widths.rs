//! Масштаб `zoom`: толщины линеек `column-rule-width`/`row-rule-width` и всех
//! элементов их списков. Начальное `medium` вычисляется в абсолютную длину
//! даже при `none`/`hidden` (CSS Multicol 1), поэтому материализуется до
//! умножения, как Blink `SetZoomedInitialLineWidths`, а не поздним запасом
//! 3px в раскладке; шрифтовые единицы не трогаются.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn scale(c: &mut Computed, k: f32) {
    let length = |l: &mut Len| match l {
        Len::Px(v) | Len::EmPx(_, v) | Len::LhPx(_, v) => *v *= k,
        _ => {}
    };
    for (width, list) in [
        (&mut c.column_rule_width, &mut c.column_rule_widths),
        (&mut c.row_rule_width, &mut c.row_rule_widths),
    ] {
        // CSS Multicol 1: medium computes to an absolute length even for
        // none/hidden rule styles. Materialize it before zoom, like Blink's
        // SetZoomedInitialLineWidths, rather than the late 3px layout fallback.
        length(width.get_or_insert(Len::Px(3.0)));
        if let Some(list) = list {
            for value in list
                .lead
                .iter_mut()
                .chain(&mut list.auto)
                .chain(&mut list.tail)
            {
                length(value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::GapList;

    #[test]
    fn initial_medium_is_zoomed_without_requiring_a_visible_rule() {
        let mut style = Computed::default();
        super::super::scale_own(&mut style, 2.0);
        assert_eq!(style.column_rule_width, Some(Len::Px(6.0)));
        assert_eq!(style.row_rule_width, Some(Len::Px(6.0)));
    }

    #[test]
    fn lists_scale_absolute_lengths_once_and_keep_font_units() {
        let mut style = Computed {
            column_rule_width: Some(Len::Px(2.0)),
            column_rule_widths: Some(GapList {
                lead: vec![Len::Px(2.0)],
                auto: vec![Len::Em(3.0)],
                tail: vec![Len::Px(4.0)],
            }),
            row_rule_width: Some(Len::Px(0.0)),
            ..Computed::default()
        };
        super::super::scale_own(&mut style, 2.0);
        assert_eq!(style.column_rule_width, Some(Len::Px(4.0)));
        assert_eq!(style.row_rule_width, Some(Len::Px(0.0)));
        let widths = style.column_rule_widths.expect("rule list");
        assert_eq!(widths.lead, vec![Len::Px(4.0)]);
        assert_eq!(widths.auto, vec![Len::Em(3.0)]);
        assert_eq!(widths.tail, vec![Len::Px(8.0)]);
    }
}
