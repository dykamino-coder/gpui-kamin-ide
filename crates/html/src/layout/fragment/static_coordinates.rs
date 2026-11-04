//! Перевод щупов статической позиции из непрерывной копии фрагмента в размещение по
//! фрагментаинерам: `fixed`-коробка сохраняет область просмотра содержащим блоком, и лишь её
//! гипотетическое место в потоке следует плану стопки колонок (`Place` по детям и
//! смещениям, стек `Mapping` в `Deferred::coordinates` для вложенной фрагментации). План
//! выделяется только деревьям с такими щупами (`needed`,
//! `ColumnStack::with_static_coordinates`).

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

pub(crate) struct Mapping {
    kid: usize,
    origin: Point<Pixels>,
    places: Places,
}

pub(crate) fn with<R>(
    owner: &crate::layout::cx::deferred::Deferred,
    mapping: Option<(usize, Point<Pixels>, Places)>,
    f: impl FnOnce() -> R,
) -> R {
    let Some((kid, origin, places)) = mapping else {
        return f();
    };
    owner.coordinates.borrow_mut().push(Mapping {
        kid,
        origin,
        places,
    });
    let result = f();
    owner.coordinates.borrow_mut().pop();
    result
}

pub(crate) fn map(
    owner: &crate::layout::cx::deferred::Deferred,
    mut hole: Bounds<Pixels>,
) -> Bounds<Pixels> {
    {
        for mapping in owner.coordinates.borrow().iter().rev() {
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
    }
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
    let mut next = Some(e);
    let mut ancestors = Vec::new();
    loop {
        if let Some(element) = next.take() {
            let style = element.style.used();
            if !crate::style::computed::stacking::fixed_cb_layer_box(&style) {
                if style.reset_positioned.position == Some(crate::style::computed::Position::Fixed)
                {
                    return true;
                }
                ancestors.push(element.children.iter());
            }
        }
        let Some(children) = ancestors.last_mut() else {
            return false;
        };
        next = children.find_map(|node| match node {
            crate::dom::Node::Element(child) => Some(child),
            _ => None,
        });
        if next.is_none() {
            ancestors.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn static_probe_search_handles_deep_trees_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                use crate::dom::{Element, Node};
                let doc = crate::document::state::DocState::new(Default::default());
                let mut style = crate::style::computed::Computed::default();
                style.apply_decls(
                    &crate::style::css::parse_decls("position:fixed"),
                    doc.values(),
                );
                let mut root = Element::synthetic("div", style, Default::default(), vec![]);
                assert!(needed(&root));
                for _ in 0..20_000 {
                    root = Element::synthetic(
                        "div",
                        Default::default(),
                        Default::default(),
                        vec![Node::Element(root)],
                    );
                }
                assert!(needed(&root));
                let empty =
                    Element::synthetic("div", Default::default(), Default::default(), vec![]);
                assert!(!needed(&empty));
            })
            .unwrap()
            .join()
            .unwrap();
    }

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
        let owner = crate::layout::cx::deferred::Deferred::default();
        with(&owner, Some((0, point(px(10.0), px(20.0)), places)), || {
            let probe = |y: f32| Bounds {
                origin: point(px(10.0), px(20.0 + y)),
                size: size(px(25.0), px(0.0)),
            };
            assert_eq!(map(&owner, probe(150.0)).origin, point(px(35.0), px(70.0)));
            assert_eq!(map(&owner, probe(300.0)).origin, point(px(60.0), px(120.0)));
        });
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
        let owner = crate::layout::cx::deferred::Deferred::default();
        with(
            &owner,
            Some((
                0,
                point(px(0.0), px(0.0)),
                entries([(0.0, 0.0), (100.0, -100.0)]),
            )),
            || {
                with(
                    &owner,
                    Some((
                        0,
                        point(px(0.0), px(100.0)),
                        entries([(0.0, 100.0), (25.0, 0.0)]),
                    )),
                    || {
                        let probe = Bounds {
                            origin: point(px(0.0), px(250.0)),
                            size: size(px(25.0), px(0.0)),
                        };
                        assert_eq!(map(&owner, probe).origin, point(px(125.0), px(50.0)));
                    },
                );
            },
        );
    }
}
