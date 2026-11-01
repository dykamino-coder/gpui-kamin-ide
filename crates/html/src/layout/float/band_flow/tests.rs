//! Проверки сохранения измеренных детей и независимости intrinsic-запросов от планов.

use super::*;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AvailableSpace, Bounds, ParentElement, Pixels, Styled, point, size};

mod lifetime;
mod painting;
mod persistent;
mod sizing;
mod text_context;

fn block_cx() -> crate::layout::cx::LayoutCx {
    let mut cx = crate::layout::cx::LayoutCx::default();
    cx.constraints.avail_w = Some(100.);
    cx
}

fn kid(kind: Kind, builds: Rc<Cell<usize>>, bounds: Rc<Cell<Option<Bounds<Pixels>>>>) -> Kid {
    Kid {
        intrinsic: Default::default(),
        flow: None,
        watched: None,
        flow_key: 0,
        kind,
        node_id: 1,
        pagination_offset: 0.,
        float_floor: None,
        monolithic: false,
        clear: None,
        margin: [Edge::Px(0.); 4],
        build: Rc::new(move |_, _, _, _, _| {
            builds.set(builds.get() + 1);
            Capture {
                child: div().w(px(20.)).h(px(10.)).into_any_element(),
                bounds: bounds.clone(),
            }
            .hooked()
        }),
        nest: None,
        anon: false,
        lead: None,
        lead_base: None,
        margin_offset: 0.,
        start_open: true,
        shape: None,
        rtl: false,
    }
}

struct Capture {
    child: AnyElement,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Wrap for Capture {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.bounds.set(Some(bounds));
        self.child.prepaint(window, cx);
    }
}

#[gpui::test]
fn definite_block_prepares_each_child_once_through_prepaint(cx: &mut gpui::TestAppContext) {
    for kind in [
        Kind::Flow,
        Kind::Piece {
            table: false,
            rtl: false,
        },
        Kind::Float {
            side: 1,
            clear: None,
            shrink: false,
            letter: false,
            own_layout: false,
        },
    ] {
        let builds = Rc::new(Cell::new(0));
        let bounds = Rc::new(Cell::new(None));
        let window = cx.add_empty_window();
        window.simulate_scale_factor_change(1.);
        window.draw(
            point(px(0.25), px(0.25)),
            size(px(100.), px(100.)),
            |_, _| {
                BandFlow::new(
                    vec![kid(kind, builds.clone(), bounds.clone())],
                    true,
                    &block_cx(),
                )
                .hooked()
            },
        );
        assert_eq!(builds.get(), 1, "{kind:?}");
        let actual = bounds.get().expect("prepared child reaches prepaint");
        assert_eq!(actual.size, size(px(20.), px(10.)));
        let x = if matches!(kind, Kind::Float { side: 1, .. }) {
            80.
        } else {
            0.
        };
        assert_eq!(actual.origin, point(px(x), px(0.)), "{kind:?}");
    }
}

#[gpui::test]
fn intrinsic_pair_does_not_place_children_or_evict_width_plans(cx: &mut gpui::TestAppContext) {
    let builds = Rc::new(Cell::new(0));
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let flow = BandFlow::new(
                vec![kid(Kind::Flow, builds.clone(), Default::default())],
                true,
                &block_cx(),
            );
            let plans = flow.plan.clone();
            let mut el = flow.hooked();
            let root = el.request_layout(window, app);
            let mut layout = |width| {
                let available = size(width, AvailableSpace::MaxContent);
                if matches!(width, AvailableSpace::Definite(_)) {
                    el.layout_as_root(available, window, app)
                } else {
                    window.measure_layout_size(root, available, gpui::Axis::Horizontal, app)
                }
            };
            assert_eq!(layout(AvailableSpace::MinContent).width, px(20.));
            assert_eq!(layout(AvailableSpace::MaxContent).width, px(20.));
            assert_eq!(builds.get(), 1, "min/max share one child preparation");
            assert!(plans.get(100.).is_none());
            layout(AvailableSpace::Definite(px(100.)));
            layout(AvailableSpace::Definite(px(200.)));
            assert_eq!(builds.get(), 3);
            assert!(plans.get(100.).is_some());
            assert!(plans.get(200.).is_some());
            layout(AvailableSpace::MinContent);
            layout(AvailableSpace::Definite(px(100.)));
            assert_eq!(
                builds.get(),
                3,
                "intrinsic query preserves both placement plans"
            );
            div()
        },
    );
}

#[gpui::test]
fn replacement_element_invalidates_intrinsic_sizes(cx: &mut gpui::TestAppContext) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let builds = Rc::new(Cell::new(0));
            for width in [20., 35.] {
                let mut child = kid(Kind::Flow, builds.clone(), Default::default());
                let count = builds.clone();
                child.build = Rc::new(move |_, _, _, _, _| {
                    count.set(count.get() + 1);
                    div().w(px(width)).h(px(10.)).into_any_element()
                });
                let mut element = BandFlow::new(vec![child], true, &block_cx()).hooked();
                let root = element.request_layout(window, app);
                for available in [AvailableSpace::MinContent, AvailableSpace::MaxContent] {
                    let actual = window.measure_layout_size(
                        root,
                        size(available, AvailableSpace::MaxContent),
                        gpui::Axis::Horizontal,
                        app,
                    );
                    assert_eq!(actual.width, px(width));
                }
            }
            assert_eq!(builds.get(), 2, "each immutable generation measures once");
            div()
        },
    );
}

#[gpui::test]
fn nested_shell_reuses_its_clamp_probe(cx: &mut gpui::TestAppContext) {
    for measure in [None, Some((1, Default::default()))] {
        let builds = Rc::new(Cell::new(0));
        let bounds = Rc::new(Cell::new(None));
        cx.add_empty_window()
            .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                let mut shell = kid(Kind::Nest, builds.clone(), Default::default());
                shell.nest = Some(Nest {
                    kids: vec![kid(Kind::Flow, builds.clone(), bounds.clone())],
                    inset: [Edge::Px(0.); 4],
                    height: None,
                    width: None,
                    group: None,
                });
                BandFlow::new(vec![shell], true, &block_cx())
                    .clamp_measure(measure, None)
                    .hooked()
            });
        assert_eq!(builds.get(), 2, "shell and descendant are each built once");
        assert_eq!(bounds.get().unwrap().size, size(px(20.), px(10.)));
    }
}

#[gpui::test]
fn anonymous_host_follows_the_owners_stretch_and_intrinsic_size(cx: &mut gpui::TestAppContext) {
    for stretch in [true, false] {
        let bounds = Rc::new(Cell::new(None));
        let output = bounds.clone();
        cx.add_empty_window()
            .draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
                let flow = BandFlow::new(
                    vec![kid(Kind::Flow, Default::default(), Default::default())],
                    true,
                    &block_cx(),
                );
                let mut flow = flow;
                flow.fill_inline = stretch;
                let owner = div().flex().flex_col();
                let owner = if stretch {
                    owner.items_stretch()
                } else {
                    owner.items_start()
                };
                owner.child(
                    Capture {
                        child: flow.hooked(),
                        bounds: output,
                    }
                    .hooked(),
                )
            });
        let actual = bounds.get().unwrap().size;
        assert_eq!(actual, size(px(if stretch { 100. } else { 20. }), px(10.)));
    }
}
