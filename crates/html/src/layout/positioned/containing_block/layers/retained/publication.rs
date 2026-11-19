//! Publish only the selected subtree and prepare its hoisted layout before source prepaint.

use super::Escaped;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};

pub(super) struct Publication {
    escaped: Option<Escaped>,
    child: AnyElement,
}

impl Publication {
    pub(super) fn wrap(escaped: Escaped, child: AnyElement) -> AnyElement {
        Self {
            escaped: Some(escaped),
            child,
        }
        .hooked()
    }
}

impl Wrap for Publication {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.child.paint_metadata()
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if let Some(escaped) = self.escaped.take() {
            for (queue, destination) in escaped.0 {
                queue.commit_prepared(&destination, window, cx);
            }
        }
        self.child.prepaint(window, cx);
    }
}
