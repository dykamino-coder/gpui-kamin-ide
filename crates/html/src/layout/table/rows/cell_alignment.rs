//! CSS Align 3 block content alignment for the single table-cell alignment subject.
use crate::style::computed::{Computed, Justify};
use gpui::{Div, Styled};

pub(super) fn apply(cm: &Computed, mut cell: Div) -> Div {
    // CSS Align 3 section 5.1.1: normal uses vertical-align. For block
    // containers an omitted overflow position is safe (Blink's
    // block_layout_algorithm_utils.cc ComputeContentAlignment).
    let mut safe = !cm.align_content_unsafe;
    if cm.align_content_block {
        let last_baseline = cm.align_content_baseline == Some(true);
        let alignment = match cm.align_content {
            Some(Justify::Center | Justify::Around | Justify::Evenly) => {
                gpui::JustifyContent::Center
            }
            Some(Justify::End | Justify::WmEnd) => gpui::JustifyContent::End,
            _ if last_baseline => gpui::JustifyContent::End,
            _ => gpui::JustifyContent::Start,
        };
        // Distributed values have one alignment subject: use their fallback.
        safe |=
            last_baseline || matches!(cm.align_content, Some(Justify::Around | Justify::Evenly));
        let style = cell.style();
        style.justify_content = Some(alignment);
        style.table_cell_baseline =
            Some(cm.align_content_baseline == Some(false) && !cm.wm().vertical());
        style.flex_direction = Some(if cm.wm().vertical() {
            if cm.wm().block_rl() {
                gpui::FlexDirection::RowReverse
            } else {
                gpui::FlexDirection::Row
            }
        } else {
            gpui::FlexDirection::Column
        });
    }
    let style = cell.style();
    let mut overflow = style.safe_alignment.unwrap_or_default();
    overflow.3 = safe;
    style.safe_alignment = Some(overflow);
    cell
}

pub(super) fn subject(cm: &Computed, contents: Vec<gpui::AnyElement>) -> Vec<gpui::AnyElement> {
    use gpui::{IntoElement, ParentElement, div};
    if !cm.align_content_block {
        return contents;
    }
    let mut subject = div().flex().flex_shrink_0();
    subject = if cm.wm().vertical() {
        if cm.wm().block_rl() {
            subject.flex_row_reverse()
        } else {
            subject.flex_row()
        }
    } else {
        subject.flex_col().w_full()
    };
    vec![subject.children(contents).into_any_element()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{JustifyContent, div};

    #[test]
    fn normal_preserves_vertical_alignment_with_safe_overflow() {
        let cm = Computed::default();
        let mut cell = apply(&cm, div().justify_end());
        let style = cell.style();
        assert_eq!(style.justify_content, Some(JustifyContent::End));
        assert!(style.safe_alignment.unwrap().3);
    }

    #[test]
    fn block_positions_distinguish_default_safe_from_explicit_unsafe() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for (value, safe) in [
            ("center", true),
            ("safe end", true),
            ("unsafe center", false),
            ("unsafe end", false),
        ] {
            let mut cm = Computed::default();
            cm.apply_one("align-content", value, vcx);
            let mut cell = apply(&cm, div());
            assert_eq!(cell.style().safe_alignment.unwrap().3, safe, "{value}");
        }
    }

    #[test]
    fn one_subject_uses_distribution_and_baseline_fallbacks() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for (value, expected) in [
            ("space-between", JustifyContent::Start),
            ("stretch", JustifyContent::Start),
            ("space-around", JustifyContent::Center),
            ("space-evenly", JustifyContent::Center),
            ("last baseline", JustifyContent::End),
        ] {
            let mut cm = Computed::default();
            cm.apply_one("align-content", value, vcx);
            let mut cell = apply(&cm, div());
            assert_eq!(cell.style().justify_content, Some(expected), "{value}");
            assert!(cell.style().safe_alignment.unwrap().3);
        }
    }
}
