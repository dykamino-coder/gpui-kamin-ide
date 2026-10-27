//! Единственная native-фаза сохраняется после независимой раскладки и replay.

use super::*;
use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AvailableSpace, IntoElement, LayoutId, PaintPlacement, PaintPlan, PaintSegmentId, canvas,
};
use std::cell::Cell;

type Counts = Rc<[Cell<usize>; 3]>;
struct Probe(Counts);

impl Hooks for Probe {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.0[0].set(self.0[0].get() + 1);
        (window.request_layout(gpui::Style::default(), [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {
        self.0[1].set(self.0[1].get() + 1);
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.0[2].set(self.0[2].get() + 1);
        window.paint_quad(gpui::fill(
            Bounds::new(point(px(0.), px(0.)), gpui::size(px(10.), px(10.))),
            gpui::red(),
        ));
    }
}

#[test]
fn source_is_prepainted_and_painted_once_and_replay_keeps_independent_roots_valid() {
    let counts: Counts = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0)]);
    let observed = counts.clone();
    let mut cx = gpui::TestAppContext::single();
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        gpui::size(px(100.), px(100.)),
        |_, _| {
            let counts = counts.clone();
            crate::hooks::paint_root(
                canvas(
                    move |_, window, cx| {
                        let mut root = Probe(counts).hooked().into_any_element();
                        let id = root.request_layout(window, cx);
                        let layout = Node::capture(id, window);
                        let constraints = gpui::size(
                            AvailableSpace::Definite(px(20.)),
                            AvailableSpace::Definite(px(20.)),
                        );
                        root.layout_as_root(constraints, window, cx);
                        let native = Native::new(
                            root,
                            layout,
                            Bounds::new(point(px(0.), px(0.)), gpui::size(px(20.), px(20.))),
                        );
                        let routes: Rc<[PaintSegment]> = Rc::from([PaintSegment {
                            id: PaintSegmentId(1),
                            ordinal: 0,
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                        }]);
                        native
                            .prepaint(routes.clone(), SourceSlices::default(), window, cx)
                            .unwrap();
                        assert_eq!(
                            native.prepaint(routes, SourceSlices::default(), window, cx),
                            Err(PaintRecordError::InvalidGeometry)
                        );
                        let mut other = gpui::div().into_any_element();
                        let other_id = other.request_layout(window, cx);
                        other.layout_as_root(constraints, window, cx);
                        let other_node = Node::capture(other_id, window);
                        assert!(layout.read(window).is_some());
                        (native, other, other_node)
                    },
                    |_, state, window, cx| {
                        let (native, _other, other_node) = state;
                        let records = native.record(window, cx).unwrap();
                        assert_eq!(
                            native.record(window, cx).err(),
                            Some(PaintRecordError::InvalidGeometry)
                        );
                        let plan =
                            PaintPlan::record(records.get(PaintSegmentId(1)).unwrap().clone());
                        for x in [0., 30.] {
                            window
                                .paint_plan(
                                    &plan,
                                    PaintPlacement {
                                        source: point(px(0.), px(0.)),
                                        destination: point(px(x), px(0.)),
                                        clip: Bounds::new(
                                            point(px(0.), px(0.)),
                                            gpui::size(px(100.), px(100.)),
                                        ),
                                    },
                                )
                                .unwrap();
                        }
                        assert!(other_node.read(window).is_some());
                    },
                )
                .into_any_element(),
            )
        },
    );
    cx.quit();
    assert_eq!(
        observed.iter().map(Cell::get).collect::<Vec<_>>(),
        [1, 1, 1]
    );
}
