//! Keep inherited atomic formatting contexts physical inside a rotated text paragraph.
use crate::{
    computed::{Computed, Display, Sides},
    dom::Element,
};
use gpui::{IntoElement, ParentElement, Styled};

thread_local! {
    static HAS_TEXT_TURN: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub(super) fn without_text_turn<R>(build: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            HAS_TEXT_TURN.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(HAS_TEXT_TURN.with(|cell| cell.replace(false)));
    build()
}

/// The paragraph being built is a rotated line that carries text (not the
/// pure-atom branch of `without_text_turn`).
pub(super) fn text_turn() -> bool {
    HAS_TEXT_TURN.with(|cell| cell.get())
}

pub(super) fn physical(
    e: &Element,
    flow: &Computed,
    opts: &super::RenderOpts,
) -> Option<gpui::AnyElement> {
    if !HAS_TEXT_TURN.with(|cell| cell.get())
        || flow.rotated_line != Some(true)
        || e.style.vertical.is_some()
        || e.style.inline_display == Some(true)
        || !matches!(
            e.style.display,
            Some(
                Display::InlineBlock
                    | Display::InlineFlex
                    | Display::InlineGrid
                    | Display::InlineTable
            )
        )
        || matches!(
            e.style.position,
            Some(crate::computed::Position::Absolute | crate::computed::Position::Fixed)
        )
    {
        return None;
    }
    let mut context = flow.clone();
    context.vertical = Some(true);
    context.rotated_line = None;
    context.lines_reversed = None;
    context.para_vertical = None;
    let mut child = super::pct_resolved_against_block(e, flow).unwrap_or_else(|| e.clone());
    let margins = margin(&child.style, flow);
    child.style.margin = Sides::default();
    let central = flow.sideways != Some(true) && flow.text_sideways != Some(true);
    let ccw = flow.sideways == Some(true) && flow.vertical_rl != Some(true);
    let physical = super::element(&child, &context, opts);
    Some(
        crate::apply::margins(gpui::div().flex().flex_col().flex_shrink_0(), &margins)
            .child(super::physical_atomic::PhysicalAtomic::new(
                physical, ccw, central,
            ))
            .into_any_element(),
    )
}

pub(super) fn resolved(e: &Element, flow: &Computed) -> Option<Element> {
    (e.style.display == Some(Display::InlineBlock) && e.style.inline_display != Some(true))
        .then(|| super::pct_resolved_against_block(e, flow))
        .flatten()
}

pub(super) fn margin(style: &Computed, flow: &Computed) -> Sides {
    crate::inline::physical_sides::side_values(flow, style.margin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_atom_construction_restores_the_text_turn_even_after_unwinding() {
        assert!(HAS_TEXT_TURN.with(|cell| cell.get()));
        let result = std::panic::catch_unwind(|| {
            without_text_turn(|| {
                assert!(!HAS_TEXT_TURN.with(|cell| cell.get()));
                without_text_turn(|| assert!(!HAS_TEXT_TURN.with(|cell| cell.get())));
                panic!("aborted atom construction");
            })
        });
        assert!(result.is_err());
        assert!(HAS_TEXT_TURN.with(|cell| cell.get()));
    }
}
