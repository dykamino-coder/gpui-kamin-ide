//! Применение к GPUI: `align-content: baseline` у элемента сетки. Коробка
//! остаётся растянутой, а её содержимое сдвигается к общей базовой ряда тем же
//! контрактом сдвига содержимого, что и ячейки таблицы с `vertical-align:
//! baseline`. Реализован только физически-вертикальный первый baseline;
//! остальные предпочтения оставляют прежний запасной путь без сдвига коробки.

use crate::style::computed::{Align, Computed, Position};

pub(super) fn apply(style: &mut gpui::StyleRefinement, c: &Computed) {
    // Only the physical-y, first-baseline contract is currently implemented.
    // Other preferences keep their existing fallback rather than moving the box.
    if c.parent_grid == 1
        && !c.wm().vertical()
        && c.align_content_baseline == Some(false)
        && matches!(
            c.align_self.or(c.parent_grid_align_items),
            None | Some(Align::Stretch)
        )
        && !matches!(c.position, Some(Position::Absolute | Position::Fixed))
    {
        style.table_cell_baseline = Some(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_preference_survives_parsing_inheritance_and_reset() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("align-content", "first baseline", vcx);
        assert_eq!(parent.align_content_baseline, Some(false));
        let mut own = Computed::default();
        own.apply_one("align-content", "inherit", vcx);
        let inherited = crate::style::cascade::inherit::inherit(
            &crate::style::computed::BoxStyle::from(parent.clone()),
            &own,
            vcx,
        );
        assert_eq!(inherited.align_content_baseline, Some(false));
        parent.apply_one("align-content", "last baseline", vcx);
        assert_eq!(parent.align_content_baseline, Some(true));
        parent.apply_one("align-content", "normal", vcx);
        assert_eq!(parent.align_content_baseline, None);
    }

    #[test]
    fn inherited_item_alignment_must_keep_the_box_at_its_start_edge() {
        for (alignment, participates) in [
            (None, true),
            (Some(Align::Stretch), true),
            (Some(Align::Center), false),
            (Some(Align::End), false),
        ] {
            let c = Computed {
                parent_grid: 1,
                parent_grid_align_items: alignment,
                align_content_baseline: Some(false),
                ..Computed::default()
            };
            let mut style = gpui::StyleRefinement::default();
            apply(&mut style, &c);
            assert_eq!(style.table_cell_baseline == Some(true), participates);
        }
    }
}
