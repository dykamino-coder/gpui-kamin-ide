//! Проценты в вертикальном containing block относятся к его INLINE-размеру, физической высоте.
//!
//! CSS 2.1 §8.3/§8.4 и css-box-4: процентные margin и padding относятся к inline-размеру
//! containing block; css-writing-modes-4 §7.1 отображает процентную `height` бокса в
//! вертикальном потоке на ту же inline-ось. Как ширина горизонтального блока, высота
//! вертикального блока определённа, когда он заполняет inline-ось родителя (CSS 2.1
//! §10.3.3 в логических терминах), вплоть до начального containing block (Blink
//! `ComputeInlineSizeForFragment` / fill-available; Servo `containing_block.size.inline`).
//! taffy разрешает эти проценты относительно ФИЗИЧЕСКОЙ ширины и считает растянутую высоту
//! неопределённой, поэтому они разрешаются здесь, когда известен inline-размер содержимого
//! родителя.
use crate::style::computed::{BoxStyle, Computed, Display, Overflow, Position, Sides};
use crate::style::values::value::Len;

fn px(value: Option<Len>) -> Option<f32> {
    match value {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        _ => None,
    }
}

fn out_of_flow(style: &Computed) -> bool {
    matches!(style.position, Some(Position::Absolute | Position::Fixed))
}

/// Resolve the box's own inline-axis percentages against its vertical containing block.
pub(crate) fn resolve_percentages(style: &mut Computed, parent: &BoxStyle) {
    let Some(cb) = parent.inline_definite else {
        return;
    };
    if !parent.wm().vertical() || out_of_flow(style) {
        return;
    }
    // A float-band piece (`band_flow_host::build_kid`) is laid out in a frame as large as its
    // band window, and the host resolves its percentages against the containing block
    // itself (CSS 2.1 §10.2, §10.5); resolving them here as well put the piece's inline
    // size out of step with the host's measure (`slr-alongside-vlr-floats`: the BFC beside
    // the floats was placed in its window but painted under the first float).
    if style.band_kid {
        return;
    }
    let at = |value: &mut Option<Len>| {
        if let Some(Len::Pct(k)) = *value {
            *value = Some(Len::Px(k * cb));
        }
    };
    at(&mut style.height);
    at(&mut style.min_height);
    at(&mut style.max_height);
    for sides in [&mut style.margin, &mut style.padding] {
        let Sides {
            top,
            right,
            bottom,
            left,
        } = sides;
        for side in [top, right, bottom, left] {
            at(side);
        }
    }
}

/// The box's own content inline size when it is definite and its inline axis is vertical.
pub(crate) fn own(style: &Computed, parent: &BoxStyle, root: bool, icb_height: f32) -> Option<f32> {
    if !style.wm().vertical() || style.contains_height() {
        return None;
    }
    let b = style.borders();
    // A scroll container's scrollbars are only known after a layout pass, so its
    // content inline size is left to the layout engine.
    if style.scroller
        || [style.overflow_x, style.overflow_y]
            .into_iter()
            .any(|o| matches!(o, Some(Overflow::Scroll | Overflow::Hidden)))
    {
        return None;
    }
    let edges = px(b.top)? + px(b.bottom)? + px(style.padding.top)? + px(style.padding.bottom)?;
    match style.height {
        Some(Len::Px(h)) => {
            return Some(if style.border_box == Some(true) {
                (h - edges).max(0.0)
            } else {
                h
            });
        }
        None | Some(Len::Auto) => {}
        _ => return None,
    }
    // An automatic inline size fills the parent's content inline size only for an in-flow
    // block-level box of the same (parallel) flow; floats, atomic inlines and out-of-flow
    // boxes shrink to fit instead.
    if out_of_flow(style)
        || style.float.is_some_and(|side| side != 0)
        || !matches!(
            style.display,
            None | Some(Display::Block | Display::ListItem)
        )
    {
        return None;
    }
    let space = if root {
        Some(icb_height)
    } else if parent.wm().vertical() {
        parent.inline_definite
    } else {
        stretched_flex_item(style, parent)
    }?;
    let margins = px(style.margin.top)? + px(style.margin.bottom)?;
    Some((space - margins - edges).max(0.0))
}

/// A stretched item of a horizontal single-line flex row with a definite height: its
/// physical height is definite (css-flexbox-1 §9.8 item 11, Blink
/// `IsStretchedAlongBlockAxis`), and for a vertical item that is its inline size.
fn stretched_flex_item(style: &Computed, parent: &Computed) -> Option<f32> {
    use crate::style::computed::Align;
    let stretch = |a: Option<Align>| matches!(a, None | Some(Align::Stretch));
    if !crate::layout::box_kind::flex_row_nowrap(parent)
        || !stretch(style.align_self)
        || !stretch(parent.align_items)
        || parent.contains_height()
    {
        return None;
    }
    let Some(Len::Px(height)) = parent.height else {
        return None;
    };
    let pb = parent.borders();
    let parent_edges = if parent.border_box == Some(true) {
        px(pb.top)? + px(pb.bottom)? + px(parent.padding.top)? + px(parent.padding.bottom)?
    } else {
        0.0
    };
    Some(height - parent_edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertical() -> BoxStyle {
        let mut style = BoxStyle::default();
        style.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true, false, false,
        ));
        style
    }

    #[test]
    fn stretched_vertical_blocks_carry_the_icb_height_down() {
        let mut html = vertical();
        html.inline_definite = own(&html, &BoxStyle::default(), true, 600.0);
        assert_eq!(html.inline_definite, Some(600.0));
        let mut body = vertical();
        body.margin.top = Some(Len::Px(8.0));
        body.margin.bottom = Some(Len::Px(8.0));
        assert_eq!(own(&body, &html, false, 600.0), Some(584.0));
        let mut float = vertical();
        float.float = Some(-1);
        assert_eq!(own(&float, &html, false, 600.0), None);
    }

    #[test]
    fn percentages_refer_to_the_vertical_inline_size() {
        let mut parent = vertical();
        parent.inline_definite = Some(200.0);
        let mut child = vertical();
        child.height = Some(Len::Pct(0.25));
        child.padding.left = Some(Len::Pct(0.5));
        resolve_percentages(&mut child, &parent);
        assert_eq!(child.height, Some(Len::Px(50.0)));
        assert_eq!(child.padding.left, Some(Len::Px(100.0)));
        let mut abs = vertical();
        abs.position = Some(Position::Absolute);
        abs.height = Some(Len::Pct(0.25));
        resolve_percentages(&mut abs, &parent);
        assert_eq!(abs.height, Some(Len::Pct(0.25)));
    }

    #[test]
    fn a_stretched_vertical_flex_item_takes_the_row_height() {
        let row = Computed {
            display: Some(Display::Flex),
            height: Some(Len::Px(100.0)),
            ..Computed::default()
        };
        let mut item = vertical();
        item.margin.top = Some(Len::Px(4.0));
        assert_eq!(
            own(&item, &BoxStyle::from(row.clone()), false, 600.0),
            Some(96.0)
        );
        item.overflow_y = Some(Overflow::Scroll);
        assert_eq!(own(&item, &BoxStyle::from(row), false, 600.0), None);
    }
}
