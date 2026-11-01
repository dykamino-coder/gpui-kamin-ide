//! Роли auto-коробок сохраняют intrinsic-вклады и формулу shrink-to-fit.

use super::*;
use crate::hooks::Hooks;
use crate::style::computed::{Computed, Display, Position};

struct IntrinsicBox;

#[gpui::test]
fn definite_axis_query_without_a_known_width_skips_block_intrinsics(cx: &mut gpui::TestAppContext) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            for fill in [true, false] {
                let mut context = block_cx();
                context.constraints.avail_w = fill.then_some(100.);
                let mut child = kid(Kind::Flow, Default::default(), Default::default());
                let intrinsic = child.intrinsic.clone();
                child.build = Rc::new(|_, _, _, _, _| IntrinsicBox.hooked());
                let flow = BandFlow::new(vec![child], true, &context);
                let plans = flow.plan.clone();
                let mut element = flow.hooked();
                let root = element.request_layout(window, app);
                let actual = window.measure_layout_size(
                    root,
                    size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::MaxContent,
                    ),
                    gpui::Axis::Horizontal,
                    app,
                );
                assert_eq!(actual.width, px(if fill { 100. } else { 60. }));
                assert_eq!(intrinsic.get().is_none(), fill);
                assert!(plans.borrow().get(f32::from(actual.width)).is_none());
            }
            div()
        },
    );
}

impl Hooks for IntrinsicBox {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _: &mut App) -> (LayoutId, LayoutId) {
        let id = window.request_measured_layout(Default::default(), |known, available, _, _| {
            let width = known.width.unwrap_or_else(|| match available.width {
                AvailableSpace::MinContent => px(20.),
                AvailableSpace::MaxContent => px(60.),
                AvailableSpace::Definite(w) => px(f32::from(w).clamp(20., 60.)),
            });
            size(width, px(10.))
        });
        (id, id)
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut LayoutId, _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[gpui::test]
fn unconstrained_final_layout_keeps_the_content_height(cx: &mut gpui::TestAppContext) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let mut child = kid(Kind::Flow, Default::default(), Default::default());
            child.build = Rc::new(|_, _, _, _, _| IntrinsicBox.hooked());
            let mut flow = BandFlow::new(vec![child], true, &block_cx()).hooked();
            let actual = flow.layout_as_root(
                size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                window,
                app,
            );
            assert_eq!(actual, size(px(60.), px(10.)));
            div()
        },
    );
}

#[gpui::test]
fn auto_width_roles_keep_fit_content_until_the_owner_supplies_a_width(
    cx: &mut gpui::TestAppContext,
) {
    let block = Computed::default();
    let float = Computed {
        float: Some(-1),
        ..Default::default()
    };
    let absolute = Computed {
        position: Some(Position::Absolute),
        ..Default::default()
    };
    let roles = [
        (block, true),
        (float, false),
        (
            Computed {
                floated_box: true,
                ..Default::default()
            },
            false,
        ),
        (absolute, false),
        (
            Computed {
                display: Some(Display::TableCell),
                ..Default::default()
            },
            false,
        ),
        (
            Computed {
                display: Some(Display::InlineBlock),
                ..Default::default()
            },
            false,
        ),
        (
            Computed {
                display: Some(Display::Flex),
                ..Default::default()
            },
            false,
        ),
        (
            Computed {
                display: Some(Display::Grid),
                ..Default::default()
            },
            false,
        ),
        (
            Computed {
                flex_item: true,
                ..Default::default()
            },
            false,
        ),
    ];
    for (style, stretch) in roles {
        for (available, expected) in [(100., if stretch { 100. } else { 60. }), (40., 40.)] {
            let bounds = Rc::new(Cell::new(None));
            let intrinsic;
            {
                let mut context = block_cx();
                context.constraints.avail_w = crate::layout::block::available_width::inner(
                    &style,
                    if style.flex_item { None } else { Some(100.) },
                );
                let mut child = kid(Kind::Flow, Default::default(), Default::default());
                intrinsic = child.intrinsic.clone();
                child.build = Rc::new(|_, _, _, _, _| IntrinsicBox.hooked());
                cx.add_empty_window().draw(
                    point(px(0.), px(0.)),
                    size(px(available), px(100.)),
                    |_, _| {
                        let owner = div().flex().flex_col();
                        let owner = if stretch {
                            owner.items_stretch()
                        } else {
                            owner.items_start()
                        };
                        owner.child(
                            Capture {
                                child: BandFlow::new(vec![child], true, &context).hooked(),
                                bounds: bounds.clone(),
                            }
                            .hooked(),
                        )
                    },
                );
            }
            assert_eq!(bounds.get().unwrap().size, size(px(expected), px(10.)));
            assert_eq!(intrinsic.get().is_some(), !stretch);
        }
    }
}
