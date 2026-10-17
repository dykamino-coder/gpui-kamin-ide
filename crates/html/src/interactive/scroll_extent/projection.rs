//! CSS Overflow 3 §2.3: project negative-origin scrolling onto GPUI's negative offsets.
//! Reverse axes move content toward the physical end; preserve normal overflow geometry.

use super::{CURRENT, exact_probe::sticky_geometry};
use crate::hooks::{IntoHooked, Wrap};
use crate::style::computed::{Computed, Position};
use gpui::{AnyElement, App, Bounds, Pixels, ScrollHandle, Window, point, px};
pub(crate) struct Projection {
    handle: ScrollHandle,
    reverse: [bool; 2],
}
pub(crate) fn projection(parent: u64, c: Option<&Computed>) -> Option<Projection> {
    let frame = CURRENT.with(|slot| slot.borrow().clone())?;
    if frame.root.get() != Some(parent) || frame.reverse.get() == [false; 2] {
        return None;
    }
    if c.is_some_and(|c| {
        c.position == Some(Position::Fixed) && !c.transform_ancestor
            || c.position == Some(Position::Absolute) && c.cb_node != parent
    }) {
        return None;
    }
    let handle = frame.handle.borrow().clone()?;
    Some(Projection {
        handle,
        reverse: frame.reverse.get(),
    })
}
pub(crate) fn project(child: AnyElement, projection: Option<Projection>) -> AnyElement {
    match projection {
        Some(projection) => Projected { child, projection }.hooked(),
        None => child,
    }
}
struct Projected {
    child: AnyElement,
    projection: Projection,
}
impl Wrap for Projected {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let native = self.projection.handle.offset();
        let shift = point(
            if self.projection.reverse[0] {
                -native.x * 2.0
            } else {
                px(0.0)
            },
            if self.projection.reverse[1] {
                -native.y * 2.0
            } else {
                px(0.0)
            },
        );
        sticky_geometry(shift, || {
            window.with_exact_element_offset(shift, |window| self.child.prepaint(window, cx))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Overflow;
    use crate::{
        dom::Node,
        interactive::scroll_extent::{Frame, classic},
        layout::table::anon::anon_element,
    };
    use gpui::size;
    #[test]
    fn negative_origin_projection_stays_inside_its_containing_scrollport() {
        let mut root = anon_element("div", vec![]);
        root.node_id = 7;
        let frame = Frame::default();
        frame.set_handle(ScrollHandle::default());
        frame.within(Some(&root), || {
            let parent = Computed {
                self_node: 7,
                rtl: Some(true),
                position: Some(Position::Relative),
                ..Computed::default()
            };
            classic::model(7, &parent);
            let absolute = crate::style::cascade::inherit::inherit(
                &parent,
                &Computed {
                    position: Some(Position::Absolute),
                    ..Computed::default()
                },
            );
            assert!(projection(7, Some(&absolute)).is_some());
            let fixed = crate::style::cascade::inherit::inherit(
                &parent,
                &Computed {
                    position: Some(Position::Fixed),
                    ..Computed::default()
                },
            );
            assert!(projection(7, Some(&fixed)).is_none());
            assert!(projection(8, None).is_none());
        });
    }
    #[test]
    fn ancestor_projection_does_not_create_overflow_in_a_nested_scrollport() {
        let mut child = anon_element("div", vec![]);
        child.node_id = 8;
        let mut root = anon_element("div", vec![Node::Element(child)]);
        root.node_id = 7;
        let frame = Frame::default();
        let handle = ScrollHandle::default();
        let c = Computed {
            self_node: 7,
            rtl: Some(true),
            overflow_x: Some(Overflow::Scroll),
            overflow_y: Some(Overflow::Scroll),
            ..Computed::default()
        };
        frame.within(Some(&root), || {
            classic::model(7, &c);
            frame.marker.set(true);
            // Original root x=80, child x=110; the ancestor adds 40px.
            sticky_geometry(point(px(40.0), px(0.0)), || {
                frame.record_border_box(
                    8,
                    &Computed::default(),
                    Bounds::new(point(px(150.0), px(0.0)), size(px(20.0), px(20.0))),
                );
                frame.settle(
                    &root,
                    &c,
                    &handle,
                    Bounds::new(point(px(120.0), px(0.0)), size(px(50.0), px(50.0))),
                );
            });
            assert_eq!(frame.range(), [0.0; 2]);
            assert_eq!(frame.bars(), [false; 2]);
        });
    }
}
