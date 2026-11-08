//! KaminIDE patch: the inherited text style refinements as a shared persistent
//! list. A push links one refinement to the current scope; capturing a scope for
//! a deferred draw, a paint snapshot or a measure callback is one `Rc` clone.

use super::*;

/// The text style refinements in force, outermost first (see
/// [`Window::text_style`]). Cloning shares the list.
#[derive(Clone, Default)]
pub struct TextStyleScope(Option<Rc<Link>>);

struct Link {
    style: TextStyleRefinement,
    parent: TextStyleScope,
    depth: usize,
}

impl TextStyleScope {
    /// KaminIDE patch: the same scope (the same chain of links).
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }

    /// This scope with `style` applied innermost.
    pub(crate) fn push(&self, style: TextStyleRefinement) -> Self {
        Self(Some(Rc::new(Link {
            style,
            parent: self.clone(),
            depth: self.len() + 1,
        })))
    }

    /// The number of refinements.
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |link| link.depth)
    }

    /// Whether no refinement applies.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// Apply the refinements, outermost first, onto `style`.
    pub(crate) fn refine(&self, style: &mut TextStyle) {
        let mut links: SmallVec<[&TextStyleRefinement; 16]> = SmallVec::with_capacity(self.len());
        let mut at = self.0.as_deref();
        while let Some(link) = at {
            links.push(&link.style);
            at = link.parent.0.as_deref();
        }
        for refinement in links.into_iter().rev() {
            style.refine(refinement);
        }
    }
}

impl Drop for TextStyleScope {
    // Unlink iteratively: a deep document must not recurse once per level.
    fn drop(&mut self) {
        let mut next = self.0.take();
        while let Some(link) = next {
            next = match Rc::try_unwrap(link) {
                Ok(mut link) => link.parent.0.take(),
                Err(_) => None,
            };
        }
    }
}

/// The context a measure callback was requested in: the requesting element's
/// inherited text style and rem size. Taffy calls measure functions during
/// the root's layout, after those scopes were popped; elements a callback
/// builds for probing must still see them.
#[derive(Clone)]
pub(crate) struct MeasureScope {
    text: TextStyleScope,
    rem: Pixels,
}

impl Window {
    pub(crate) fn measure_scope(&self) -> MeasureScope {
        MeasureScope {
            text: self.text_style_stack.clone(),
            rem: self.rem_size(),
        }
    }
}

impl MeasureScope {
    /// Run `f` (one measure callback) in this scope, then restore the window's.
    pub(crate) fn enter<R>(&self, window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        let text = std::mem::replace(&mut window.text_style_stack, self.text.clone());
        let depth = window.rem_size_override_stack.len();
        window.rem_size_override_stack.push(self.rem);
        window
            .with_restore(
                (text, depth),
                |window, (text, depth)| {
                    window.text_style_stack = std::mem::take(text);
                    window.rem_size_override_stack.truncate(*depth);
                },
                f,
            )
            .0
    }
    pub(crate) async fn enter_async<R>(
        &self,
        window: &mut Window,
        f: impl AsyncFnOnce(&mut Window) -> R,
    ) -> R {
        let text = std::mem::replace(&mut window.text_style_stack, self.text.clone());
        let depth = window.rem_size_override_stack.len();
        window.rem_size_override_stack.push(self.rem);
        window
            .with_restore_async(
                (text, depth),
                |window, (text, depth)| {
                    window.text_style_stack = std::mem::take(text);
                    window.rem_size_override_stack.truncate(*depth);
                },
                f,
            )
            .await
            .0
    }
}
