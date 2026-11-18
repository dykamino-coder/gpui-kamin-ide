//! A spanner memo outlives its trial elements, not their captured contexts.

use super::{SpannerFlow, SpannerPass};
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, IntoElement, Styled, point, px, size};
use std::{cell::RefCell, rc::Rc};

struct Trial {
    child: AnyElement,
    _context: Rc<()>,
}

impl Wrap for Trial {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
}

#[gpui::test]
fn trial_contexts_expire_before_the_final_spanner_build(cx: &mut gpui::TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let trials = observed.clone();
    let final_builds = Rc::new(std::cell::Cell::new(0));
    let finals = final_builds.clone();
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(800.), px(600.)),
        move |_, _| {
            let trials = trials.clone();
            let finals = finals.clone();
            SpannerFlow::deferred(
                move |_, pass, _, _| {
                    let child = gpui::div().w(px(120.)).h(px(30.)).into_any_element();
                    if pass == SpannerPass::Measure {
                        let context = Rc::new(());
                        trials.borrow_mut().push(Rc::downgrade(&context));
                        Trial {
                            child,
                            _context: context,
                        }
                        .hooked()
                    } else {
                        assert!(!trials.borrow().is_empty(), "the trial did not run");
                        assert!(
                            trials.borrow().iter().all(|weak| weak.upgrade().is_none()),
                            "a trial element still retains its captured context"
                        );
                        finals.set(finals.get() + 1);
                        child
                    }
                },
                |_, _, _| 120.,
            )
        },
    );
    assert!(final_builds.get() > 0, "the final build did not run");
    assert!(
        observed
            .borrow()
            .iter()
            .all(|weak| weak.upgrade().is_none())
    );
}
