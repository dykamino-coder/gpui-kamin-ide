//! Владение проходом в прозрачных корнях без дополнительных layout-узлов.

use super::*;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, PaintTraversal, Pixels};

use super::state::PassOwner;

struct Root {
    child: AnyElement,
    owner: Rc<PassOwner>,
}

pub(crate) fn roots(children: Vec<AnyElement>) -> Vec<AnyElement> {
    let owner = Rc::new(PassOwner::default());
    children
        .into_iter()
        .map(|child| {
            Root {
                child,
                owner: owner.clone(),
            }
            .hooked()
        })
        .collect()
}

struct Abort(Rc<CssPaintPass>);
impl Drop for Abort {
    fn drop(&mut self) {
        if std::thread::panicking() || !self.0.id.is_live() {
            self.0.abort();
        }
    }
}

impl Wrap for Root {
    fn paint_boundary(&self) -> bool {
        false
    }
    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.child.paint_metadata()
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let outer = current(window);
        let pass = self.owner.select(window.paint_pass_id(), outer.as_ref());
        let _abort = Abort(pass.clone());
        if outer.is_some_and(|outer| Rc::ptr_eq(&outer, &pass)) {
            self.child.paint(window, cx);
            return;
        }
        let context = Rc::new(Current(Rc::downgrade(&pass)));
        let traversal = Rc::new(PaintTraversal {
            enter_boundary: Rc::new(boundary),
            children: children::default_painter(),
        });
        window.with_render_context(Some(context), |window| {
            window.with_render_context(Some(traversal), |window| self.child.paint(window, cx));
        });
    }
}
