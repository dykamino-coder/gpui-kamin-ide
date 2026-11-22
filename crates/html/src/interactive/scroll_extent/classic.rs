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
    let mut cached = frame.as_ref().map_or([false; 2], |f| f.bars.get());
    // Before its first measured range the viewport guesses a vertical bar,
    // as Gecko's `ScrollContainerFrame::GuessVScrollbarNeeded` does for the
    // root: most documents scroll, and a short one is the cheaper document
    // to lay out again when the guess was wrong (`update` reports it).
    if frame.is_some_and(|f| f.viewport.get() && !f.settling.settled.get()) {
        cached[1] = true;
    }
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

/// The `auto` axes whose bars layout decides from the content (Taffy
/// `ScrollbarLayout`), as Blink `BlockNode::Layout` lays a box out again when
/// `ComputeScrollbars` changes. Layout reserves a bar's gutter where Taffy
/// does: the horizontal bar at the bottom always. A vertical bar whose gutter
/// lies elsewhere (`scrollbar-gutter: stable`, the left side) and every bar of
/// a reversed or vertical flow keep the bars of the last measured range, and
/// the frame records why.
pub(crate) fn layout_axes(frame: Option<&Frame>, e: &Element, c: &Computed) -> [bool; 2] {
    let Some(frame) = owner(frame, e, c) else {
        return [false; 2];
    };
    let overflow = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y];
    let auto: [bool; 2] = std::array::from_fn(|axis| {
        overflow[axis] == Some(Overflow::Scroll) && !c.reset_box_layout.scroll.forced[axis]
    });
    let gutters = |bars| {
        let model = super::super::scrollbar_gutter::Model::new(c, bars);
        model.used == model.native
    };
    let mut auto = auto;
    let refusal = if !auto.iter().any(|&a| a) {
        None
    } else if c.wm().vertical() || super::super::scrollbar_gutter::reversed(c) != [false; 2] {
        auto = [false; 2];
        Some("the scroll origin is not the top left corner")
    } else if !gutters([false; 2]) || !gutters([true; 2]) {
        // The vertical bar's gutter is not the one Taffy reserves on the
        // right; the horizontal bar's is, at the bottom: layout decides it.
        let refusal = (auto[1]).then_some(if !gutters([false; 2]) {
            "scrollbar-gutter reserves the vertical bar's space without it"
        } else {
            "the vertical bar lies on the side layout does not reserve"
        });
        auto[1] = false;
        refusal
    } else {
        None
    };
    frame.settling.refusal.set(refusal);
    frame.settling.decided.set(auto);
    auto
}

/// Take the bars layout decided for this scrollport before its content is
/// painted: its gutters and insets follow them. Whether they differ from the
/// bars it was built with.
pub(crate) fn adopt(frame: &Frame, c: &Computed, bars: [bool; 2]) -> bool {
    if frame.bars.get() == bars {
        return false;
    }
    let model = super::super::scrollbar_gutter::Model::new(c, bars);
    frame.reserved.set(model.total());
    frame.insets.set(model.used);
    frame.native_reserved.set(model.native_total());
    frame.bars.set(bars);
    true
}

/// Why layout does not decide this scrollport's bars, if it does not.
pub(super) fn refusal(frame: &Frame) -> Option<&'static str> {
    frame.settling.refusal.get()
}

/// The bars of the measured range. A bar layout decided (layout_axes) is final
/// for this layout, as Blink keeps the bars BlockNode::Layout settled: the range
/// only checks it and the why journal notes a disagreement, so no later frame
/// lays the box out again for it. Whether the bars changed.
pub(super) fn update(frame: &Frame, c: &Computed, view: [f32; 2]) -> bool {
    let overflow = [c.reset_box_layout.overflow_x, c.reset_box_layout.overflow_y];
    let measured: [bool; 2] = std::array::from_fn(|axis| {
        overflow[axis] == Some(Overflow::Scroll)
            && (c.reset_box_layout.scroll.forced[axis]
                || view.into_iter().all(|v| v > 0.0) && frame.range.get()[axis] > 0.0)
    });
    let current = frame.bars.get();
    let decided = frame.settling.decided.get();
    let bars = std::array::from_fn(|axis| {
        if decided[axis] {
            current[axis]
        } else {
            measured[axis]
        }
    });
    if bars != measured {
        crate::why::note("scroll_extent.bars_disagree", || {
            format!(
                "layout decided bars {bars:?}, the measured range {measured:?}: range {:?} view {view:?}",
                frame.range.get()
            )
        });
    }
    frame.settling.settled.set(true);
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
    fn a_bar_layout_decided_holds_against_the_measured_range() {
        // Layout decided both bars of an `auto` scrollport; its measured range
        // finds only the vertical one. The range checks, it does not lay the box
        // out again: a disagreement repeated every frame never goes quiet.
        let mut owner = anon_element("div", vec![]);
        owner.node_id = 7;
        let frame = Frame::default().begin(Some(&owner)).unwrap();
        let style = scrolling(Overflow::Scroll);
        frame.settling.decided.set([true, true]);
        assert!(adopt(&frame, &style, [true, true]));
        frame.range.set([0.0, 15.0]);
        assert!(
            !update(&frame, &style, [751.0, 124.0]),
            "the range changed bars layout decided"
        );
        assert_eq!(frame.bars(), [true, true]);
        // An axis layout does not decide still takes the measured range.
        frame.settling.decided.set([true, false]);
        frame.range.set([0.0, 0.0]);
        assert!(update(&frame, &style, [751.0, 124.0]));
        assert_eq!(frame.bars(), [true, false]);
    }
}
