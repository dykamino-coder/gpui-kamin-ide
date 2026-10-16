//! Map static-position probes out of continuous fragment copies.
//!
//! The positioned box keeps the viewport as its containing block. Only its
//! hypothetical flow position follows the fragmentainer placement plan.

use gpui::{Bounds, Pixels, Point};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(crate) struct Place {
    pub(crate) kid: usize,
    pub(crate) from: f32,
    pub(crate) origin: Point<Pixels>,
}

pub(crate) type Places = Rc<RefCell<Vec<Place>>>;

pub(crate) struct StaticCoordinates;

struct Mapping {
    kid: usize,
    origin: Point<Pixels>,
    places: Places,
}

thread_local! {
    static STACK: RefCell<Vec<Mapping>> = const { RefCell::new(Vec::new()) };
}

impl StaticCoordinates {
    pub(crate) fn enter(kid: usize, origin: Point<Pixels>, places: Places) -> Self {
        STACK.with(|stack| {
            stack.borrow_mut().push(Mapping {
                kid,
                origin,
                places,
            })
        });
        Self
    }
}

impl Drop for StaticCoordinates {
    fn drop(&mut self) {
        STACK.with(|stack| stack.borrow_mut().pop());
    }
}

pub(crate) fn map(mut hole: Bounds<Pixels>) -> Bounds<Pixels> {
    STACK.with(|stack| {
        for mapping in stack.borrow().iter().rev() {
            let offset = f32::from(hole.origin.y - mapping.origin.y);
            let place = mapping
                .places
                .borrow()
                .iter()
                .filter(|place| place.kid == mapping.kid && place.from <= offset + 0.01)
                .max_by(|a, b| a.from.total_cmp(&b.from))
                .copied();
            if let Some(place) = place {
                hole.origin.x += place.origin.x - mapping.origin.x;
                hole.origin.y += place.origin.y - mapping.origin.y;
            }
        }
    });
    hole
}

/// Allocate the plan only for multicol trees with viewport-positioned probes.
impl crate::layout::multicol::column_stack::ColumnStack {
    pub(crate) fn with_static_coordinates(mut self, needed: bool) -> Self {
        self.static_places = needed.then(Default::default);
        self
    }
}

pub(crate) fn needed(e: &crate::dom::Element) -> bool {
    !crate::paint::stacking::fixed_cb_layer_box(&e.style)
        && (e.style.position == Some(crate::style::computed::Position::Fixed)
            || e.children.iter().any(|node| match node {
                crate::dom::Node::Element(child) => needed(child),
                _ => false,
            }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn static_position_follows_later_fragments_and_keeps_the_terminal_edge() {
        let places = Rc::new(RefCell::new(vec![
            Place {
                kid: 0,
                from: 0.0,
                origin: point(px(10.0), px(20.0)),
            },
            Place {
                kid: 0,
                from: 100.0,
                origin: point(px(35.0), px(-80.0)),
            },
            Place {
                kid: 0,
                from: 200.0,
                origin: point(px(60.0), px(-180.0)),
            },
        ]));
        let _scope = StaticCoordinates::enter(0, point(px(10.0), px(20.0)), places);
        let probe = |y: f32| Bounds {
            origin: point(px(10.0), px(20.0 + y)),
            size: size(px(25.0), px(0.0)),
        };
        assert_eq!(map(probe(150.0)).origin, point(px(35.0), px(70.0)));
        assert_eq!(map(probe(300.0)).origin, point(px(60.0), px(120.0)));
    }
    #[test]
    fn nested_fragmentation_maps_static_points_from_inner_to_outer() {
        let entries = |origins: [(f32, f32); 2]| {
            Rc::new(RefCell::new(
                origins
                    .into_iter()
                    .enumerate()
                    .map(|(i, (x, y))| Place {
                        kid: 0,
                        from: i as f32 * 100.0,
                        origin: point(px(x), px(y)),
                    })
                    .collect(),
            ))
        };
        let _outer = StaticCoordinates::enter(
            0,
            point(px(0.0), px(0.0)),
            entries([(0.0, 0.0), (100.0, -100.0)]),
        );
        let _inner = StaticCoordinates::enter(
            0,
            point(px(0.0), px(100.0)),
            entries([(0.0, 100.0), (25.0, 0.0)]),
        );
        let probe = Bounds {
            origin: point(px(0.0), px(250.0)),
            size: size(px(25.0), px(0.0)),
        };
        assert_eq!(map(probe).origin, point(px(125.0), px(50.0)));
    }
}
