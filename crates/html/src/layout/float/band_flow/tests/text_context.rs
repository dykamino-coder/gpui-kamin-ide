//! Deferred construction inherits the same font and rem scope as ordinary children.

use super::*;
use gpui::{AbsoluteLength, SharedString, TextStyleRefinement, rems};
use std::cell::RefCell;

type Seen = Rc<RefCell<Vec<(SharedString, Pixels, Pixels)>>>;

struct TextInputs {
    child: AnyElement,
    seen: Seen,
}

impl Wrap for TextInputs {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> gpui::LayoutId {
        let text = window.text_style();
        self.seen.borrow_mut().push((
            text.font_family,
            text.font_size.to_pixels(window.rem_size()),
            window.rem_size(),
        ));
        self.child.request_layout(window, cx)
    }
}

#[gpui::test]
fn intrinsic_and_final_preparation_restore_inherited_text_inputs(cx: &mut gpui::TestAppContext) {
    for explicit in [false, true] {
        let seen = Seen::default();
        cx.add_empty_window().draw(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
            |window, app| {
                let mut child = kid(Kind::Flow, Default::default(), Default::default());
                let output = seen.clone();
                child.build = Rc::new(move |_, _, _, _, _| {
                    let wrapper = div().child(
                        TextInputs {
                            child: div().w(px(20.)).h(px(10.)).into_any_element(),
                            seen: output.clone(),
                        }
                        .hooked(),
                    );
                    if explicit {
                        wrapper
                            .font_family("child")
                            .text_size(px(13.))
                            .into_any_element()
                    } else {
                        wrapper.into_any_element()
                    }
                });
                let mut flow = BandFlow::new(vec![child], true, &block_cx()).hooked();
                let root = window.with_rem_size(Some(px(24.)), |window| {
                    window.with_text_style(
                        Some(TextStyleRefinement {
                            font_family: Some("ancestor".into()),
                            font_size: Some(AbsoluteLength::from(rems(2.))),
                            ..Default::default()
                        }),
                        |window| flow.request_layout(window, app),
                    )
                });
                // Taffy evaluates the callback after request_layout has left the
                // ancestor. Neither this ambient font nor rem size may leak in.
                let outer = window.text_style();
                let rem_size = window.rem_size();
                for width in [AvailableSpace::MinContent, AvailableSpace::MaxContent] {
                    window.measure_layout_size(
                        root,
                        size(width, AvailableSpace::MaxContent),
                        gpui::Axis::Horizontal,
                        app,
                    );
                }
                flow.layout_as_root(
                    size(px(100.).into(), AvailableSpace::MaxContent),
                    window,
                    app,
                );
                assert_eq!(window.text_style(), outer);
                assert_eq!(window.rem_size(), rem_size);
                div()
            },
        );
        let expected = (
            SharedString::from(if explicit { "child" } else { "ancestor" }),
            px(if explicit { 13. } else { 48. }),
            px(24.),
        );
        assert!(seen.borrow().len() >= 2, "intrinsic and final layout ran");
        assert!(
            seen.borrow().iter().all(|value| *value == expected),
            "{:?}",
            seen.borrow()
        );
    }
}
