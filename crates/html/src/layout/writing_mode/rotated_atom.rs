//! Keep inherited atomic formatting contexts physical inside a rotated text paragraph.
use crate::dom::Element;
use crate::style::computed::{Computed, Display, Sides};
use gpui::{IntoElement, ParentElement, Styled};

thread_local! {
    static HAS_TEXT_TURN: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub(crate) fn without_text_turn<R>(build: impl FnOnce() -> R) -> R {
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
pub(crate) fn text_turn() -> bool {
    HAS_TEXT_TURN.with(|cell| cell.get())
}

pub(crate) fn physical(
    e: &Element,
    flow: &Computed,
    opts: &crate::render::RenderOpts,
) -> Option<gpui::AnyElement> {
    // Atomic formatting contexts keep physical dimensions and their own
    // writing mode, including an explicitly orthogonal mode. Only the
    // enclosing paragraph's text is turned (CSS Writing Modes 4, section 4.3).
    if !HAS_TEXT_TURN.with(|cell| cell.get())
        || flow.rotated_line != Some(true)
        || e.style.inline_display == Some(true)
        || !(matches!(
            e.style.display,
            Some(
                Display::InlineBlock
                    | Display::InlineFlex
                    | Display::InlineGrid
                    | Display::InlineTable
            )
        ) || physical_replaced(e))
        || matches!(
            e.style.position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        )
    {
        return None;
    }
    let mut context = flow.clone();
    context.vertical = Some(true);
    context.rotated_line = None;
    context.lines_reversed = None;
    context.para_vertical = None;
    let mut child =
        crate::layout::atom::pct_resolved_against_block(e, flow).unwrap_or_else(|| e.clone());
    let margins = margin(&child.style, flow);
    child.style.margin = Sides::default();
    // The atom is its own layout root inside the counter-rotation, where a
    // relative offset has nothing to shift against. Offsets are physical
    // (CSS 2.1 §9.4.3), so they move the wrapper in the turned line's frame.
    let (dx, dy) = crate::layout::positioned::relative::relative_shift(&child, None);
    if (dx, dy) != (0.0, 0.0) {
        child.style.inset = Default::default();
    }
    let (sx, sy) = crate::text::inline::to_rotated_frame(flow, (dx, dy));
    let central = flow.sideways != Some(true) && flow.text_sideways != Some(true);
    let ccw = flow.sideways == Some(true) && flow.vertical_rl != Some(true);
    let physical = crate::render::element(&child, &context, opts);
    let mut wrapper =
        crate::style::apply::margins(gpui::div().flex().flex_col().flex_shrink_0(), &margins);
    if (sx, sy) != (0.0, 0.0) {
        wrapper = wrapper.relative().left(gpui::px(sx)).top(gpui::px(sy));
    }
    Some(
        wrapper
            .child(
                crate::layout::writing_mode::physical_atomic::PhysicalAtomic::new(
                    physical, ccw, central,
                ),
            )
            .into_any_element(),
    )
}

/// Replaced content is never turned by a vertical writing mode (css-writing-modes-4 §7.4:
/// only typographic character units are rotated); in the turned paragraph it is laid out
/// in its physical size and counter-rotated like any other atomic inline.
fn physical_replaced(e: &Element) -> bool {
    matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
    ) && e.style.display.is_none()
}

pub(crate) fn resolved(e: &Element, flow: &Computed) -> Option<Element> {
    (e.style.display == Some(Display::InlineBlock) && e.style.inline_display != Some(true))
        .then(|| crate::layout::atom::pct_resolved_against_block(e, flow))
        .flatten()
}

pub(crate) fn margin(style: &Computed, flow: &Computed) -> Sides {
    crate::text::inline::physical_sides::side_values(flow, style.margin)
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
