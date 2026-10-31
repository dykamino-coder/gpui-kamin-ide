//! Роли auto-коробок сохраняют intrinsic-вклады и формулу shrink-to-fit.

use super::*;
use crate::hooks::Hooks;
use crate::style::computed::{Computed, Display, Position};

struct IntrinsicBox;

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
        (absolute, false),
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
    ];
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            for (style, stretch) in roles {
                let mut context = block_cx();
                context.constraints.avail_w =
                    crate::layout::block::available_width::inner(&style, Some(100.));
                let mut child = kid(Kind::Flow, Default::default(), Default::default());
                child.build = Rc::new(|_, _, _, _, _| IntrinsicBox.hooked());
                let mut element = BandFlow::new(vec![child], true, &context).hooked();
                for (available, expected) in [(100., if stretch { 100. } else { 60. }), (40., 40.)]
                {
                    let actual = element.layout_as_root(
                        size(
                            AvailableSpace::Definite(px(available)),
                            AvailableSpace::MaxContent,
                        ),
                        window,
                        app,
                    );
                    assert_eq!(actual, size(px(expected), px(10.)));
                }
            }
            div()
        },
    );
}
