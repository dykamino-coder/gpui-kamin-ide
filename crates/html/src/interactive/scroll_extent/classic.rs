//! Видимость классических полос прокрутки считается из CSS `overflow` и измеренного диапазона,
//! а не из границ декораций: `scroll` всегда, `auto` — когда есть переполнение. Смена набора
//! полос перекладывает коробку, как Blink `PaintLayerScrollableArea::UpdateAfterLayout`.
//! Модель gutter записывается в `Frame` для раскладки и покраски.
use super::Frame;
use crate::dom::Element;
use crate::style::computed::{Computed, Overflow};
use gpui::{Bounds, Pixels};

/// The frame's bars belong to the box that owns its scrollport: the tree root,
/// and only while it is a scroll container (CSS Overflow 3 §3). A node id alone
/// does not name that box: the anonymous viewport and every other anonymous box
/// or pseudo-element share id 0. Blink keeps the state on the `LayoutBox`'s own
/// `PaintLayerScrollableArea`, and Gecko on the box's scroll container frame.
fn owner<'a>(frame: Option<&'a Frame>, e: &Element, c: &Computed) -> Option<&'a Frame> {
    let scrolls = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y]
        .iter()
        .any(|o| matches!(o, Some(Overflow::Scroll | Overflow::Hidden)));
    frame.filter(|f| {
        scrolls && f.root.get() == Some(e.node_id) && f.viewport.get() == (e.tag == "html-viewport")
    })
}

pub(crate) fn model(
    frame: Option<&Frame>,
    e: &Element,
    c: &Computed,
) -> super::super::scrollbar_gutter::Model {
    let frame = owner(frame, e, c);
    let cached = frame.as_ref().map_or([false; 2], |f| f.bars.get());
    let overflow = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y];
    let bars = std::array::from_fn(|axis| {
        frame.is_some()
            && overflow[axis] == Some(Overflow::Scroll)
            && (c.reset_box_layout.scroll.forced[axis] || cached[axis])
    });
    let model = super::super::scrollbar_gutter::Model::new(c, bars);
    if let Some(frame) = frame {
        frame
            .reverse
            .set(super::super::scrollbar_gutter::reversed(c));
        frame.reserved.set(model.total());
        frame.insets.set(model.used);
        frame.native_reserved.set(model.native_total());
        frame.bars.set(bars);
    }
    model
}
pub(crate) fn reserve(frame: Option<&Frame>, e: &Element, c: &Computed) -> [bool; 2] {
    model(frame, e, c).bars
}

pub(super) fn update(frame: &Frame, c: &Computed, view: [f32; 2]) -> bool {
    let overflow = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y];
    let bars = std::array::from_fn(|axis| {
        overflow[axis] == Some(Overflow::Scroll)
            && (c.reset_box_layout.scroll.forced[axis]
                || view.into_iter().all(|v| v > 0.0) && frame.range.get()[axis] > 0.0)
    });
    frame.bars.replace(bars) != bars
}

pub(super) fn decoration(child: Bounds<Pixels>, owner: Bounds<Pixels>, gutter: [f32; 2]) -> bool {
    let near = |a: Pixels, b: Pixels| (a - b).abs() <= gpui::px(0.81);
    near(child.left(), owner.left())
        && near(child.top(), owner.top())
        && near(child.right(), owner.right() - gpui::px(gutter[0]))
        && near(child.bottom(), owner.bottom() - gpui::px(gutter[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;
    use crate::style::computed::{ResetBoxLayout, StyleGroup};

    fn scrolling(overflow: Overflow) -> Computed {
        Computed {
            reset_box_layout: StyleGroup::new(ResetBoxLayout {
                overflow_x: Some(overflow),
                overflow_y: Some(overflow),
                ..Default::default()
            }),
            ..Computed::default()
        }
    }

    /// The root frame with a measured vertical range, as `classic::update`
    /// leaves it after a settled frame.
    fn settled(owner: &Element) -> Frame {
        let frame = Frame::default().begin(Some(owner)).unwrap();
        frame.range.set([0.0, 500.0]);
        assert!(update(&frame, &scrolling(Overflow::Scroll), [400.0, 300.0]));
        frame
    }

    #[test]
    fn anonymous_boxes_keep_the_viewport_bars() {
        let viewport = anon_element("html-viewport", vec![]);
        let frame = settled(&viewport);
        let style = scrolling(Overflow::Scroll);
        assert_eq!(model(Some(&frame), &viewport, &style).bars, [false, true]);
        // An anonymous table box and a pseudo-element share the viewport's id 0.
        for tag in ["tr", "::before"] {
            let anon = anon_element(tag, vec![]);
            assert_eq!(
                model(Some(&frame), &anon, &Computed::default()).bars,
                [false; 2]
            );
            assert_eq!(model(Some(&frame), &anon, &style).bars, [false; 2]);
        }
        assert_eq!(
            frame.bars(),
            [false, true],
            "a box that is not the owner reset the bars"
        );
        assert!(
            !update(&frame, &style, [400.0, 300.0]),
            "the next frame sees new bars"
        );
    }

    #[test]
    fn an_owner_that_stops_scrolling_writes_nothing() {
        let mut owner = anon_element("div", vec![]);
        owner.node_id = 7;
        let frame = settled(&owner);
        assert_eq!(
            model(Some(&frame), &owner, &Computed::default()).bars,
            [false; 2]
        );
        assert_eq!(frame.bars(), [false, true]);
        assert_eq!(
            model(Some(&frame), &owner, &scrolling(Overflow::Hidden)).bars,
            [false; 2]
        );
        assert_eq!(
            frame.bars(),
            [false; 2],
            "a hidden scroll container has no bars"
        );
    }

    #[test]
    fn a_nested_owner_keeps_its_frame_next_to_the_viewport() {
        let viewport = anon_element("html-viewport", vec![]);
        let root = settled(&viewport);
        let mut nested = anon_element("div", vec![]);
        nested.node_id = 7;
        let own = settled(&nested);
        let style = scrolling(Overflow::Scroll);
        assert_eq!(model(Some(&own), &nested, &style).bars, [false, true]);
        assert_eq!(model(Some(&own), &viewport, &style).bars, [false; 2]);
        assert_eq!(model(Some(&root), &nested, &style).bars, [false; 2]);
        assert_eq!((root.bars(), own.bars()), ([false, true], [false, true]));
    }

    /// A pseudo-element can be a scroll container: its frame is matched by id 0,
    /// not the viewport kind, and only while it scrolls. An anonymous box inside
    /// that is not a scroll container leaves it alone.
    #[test]
    fn a_scrolling_pseudo_element_owns_its_frame() {
        let pseudo = anon_element("::before", vec![]);
        let frame = settled(&pseudo);
        let style = scrolling(Overflow::Scroll);
        assert_eq!(model(Some(&frame), &pseudo, &style).bars, [false, true]);
        let anon = anon_element("tr", vec![]);
        assert_eq!(
            model(Some(&frame), &anon, &Computed::default()).bars,
            [false; 2]
        );
        assert_eq!(frame.bars(), [false, true]);
    }
}
