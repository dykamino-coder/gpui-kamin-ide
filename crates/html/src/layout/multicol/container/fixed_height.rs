//! Заданный размер содержимого ряда колонок многоколоночника.

use crate::layout::block::available_height::content_block_size;
use crate::layout::cx::BoxConstraints;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn fixed_column_height(
    style: &Computed,
    col_vert: bool,
    col_h: Option<f32>,
    nest_rows: Option<f32>,
    parent: &BoxConstraints,
) -> Option<f32> {
    if style.column_fill_auto != Some(true) {
        return None;
    }
    if let Some(height) = nest_rows.or(col_h) {
        return Some(height);
    }
    let (preferred, maximum, outer, inline) = if col_vert {
        (style.width, style.max_width, parent.avail_w, parent.avail_h)
    } else {
        (style.height, style.max_height, parent.avail_h, parent.cb_width)
    };
    // При auto потолок колонок задаёт max-height (css-multicol-1
    // §column-fill; Blink ConstrainColumnBlockSize). Оба значения имеют
    // авторский box-sizing: fragmentainer занимает только content box.
    let preferred = match preferred {
        None | Some(Len::Auto) => maximum,
        value => value,
    };
    content_block_size(style, col_vert, preferred, outer, inline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmentainer_uses_content_box_and_minimum_wins_over_maximum() {
        let mut style = Computed {
            height: Some(Len::Px(100.0)),
            width: Some(Len::Px(140.0)),
            column_fill_auto: Some(true),
            border_box: Some(true),
            ..Default::default()
        };
        style.padding.top = Some(Len::Px(4.0));
        style.padding.bottom = Some(Len::Px(10.0));
        style.border_visible = [Some(true), None, Some(true), None];
        style.border_width.top = Some(Len::Px(2.0));
        style.border_width.bottom = Some(Len::Px(4.0));
        style.padding.left = Some(Len::Px(10.0));
        style.padding.right = Some(Len::Px(30.0));
        let height = |style: &Computed, vertical| {
            fixed_column_height(style, vertical, None, None, &BoxConstraints::default())
        };
        assert_eq!(height(&style, false), Some(80.0));
        assert_eq!(height(&style, true), Some(100.0));
        style.border_box = Some(false);
        assert_eq!(height(&style, false), Some(100.0));
        style.border_box = Some(true);
        style.max_height = Some(Len::Px(70.0));
        assert_eq!(height(&style, false), Some(50.0));
        style.min_height = Some(Len::Px(90.0));
        assert_eq!(height(&style, false), Some(70.0));
        style.min_height = None;
        style.max_height = None;
        style.height = Some(Len::Px(10.0));
        assert_eq!(height(&style, false), Some(0.0));
        style.height = None;
        style.max_height = Some(Len::Px(60.0));
        assert_eq!(height(&style, false), Some(40.0));
        style.max_height = None;
        assert_eq!(height(&style, false), None);
    }
}
