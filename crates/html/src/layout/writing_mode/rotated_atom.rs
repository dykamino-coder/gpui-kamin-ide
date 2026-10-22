//! Атомарные контексты форматирования внутри повёрнутого текстового абзаца остаются
//! физическими (css-writing-modes-4 §4.3, §7.4): строчные блоки, flex, grid, таблицы и
//! замещаемые строятся в своём письме и оборачиваются в `PhysicalAtomic` с физическими
//! полями и относительным сдвигом; флаг `text_turn` отличает абзац с текстом от чистого атома.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::build_contracts::Styling;
use crate::style::computed::{Computed, Display, Sides};
use gpui::{IntoElement, ParentElement, Styled};

/// Whether the paragraph's text line is turned around physical atoms.
pub(crate) fn text_turn(cx: &crate::layout::cx::LayoutCx) -> bool {
    !cx.pure_atom_paragraph
}

pub(crate) fn physical(
    e: &Element,
    flow: &Computed,
    opts: &crate::layout::options::RenderOpts,
) -> Option<gpui::AnyElement> {
    // Atomic formatting contexts keep physical dimensions and their own
    // writing mode, including an explicitly orthogonal mode. Only the
    // enclosing paragraph's text is turned (CSS Writing Modes 4, section 4.3).
    if !text_turn(&opts.cx)
        || !flow.wm().turned()
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
    context.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
        true,
        context.wm().block_rl(),
        context.wm().sideways(),
    ));
    context.turned_line = false;
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
    let central = !flow.wm().alphabetic_baseline();
    let ccw = flow.wm().line().line_turn()
        == crate::style::computed::writing_mode::Turn::CounterClockwise;
    let physical = opts.cx.build.element(&child, &context, opts);
    let mut wrapper = opts
        .cx
        .build
        .styled_div(Styling::Margins(&margins), &opts.cx)
        .flex()
        .flex_col()
        .flex_shrink_0();
    if (sx, sy) != (0.0, 0.0) {
        wrapper = wrapper.relative().left(gpui::px(sx)).top(gpui::px(sy));
    }
    Some(
        wrapper
            .child(
                crate::layout::writing_mode::physical_atomic::PhysicalAtomic::new(
                    physical,
                    ccw,
                    central,
                    opts.cx.vertical_paint.clone(),
                )
                .into_hooked(),
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
    fn atom_build_context_is_local_to_its_branch() {
        let parent = crate::layout::cx::LayoutCx::default();
        let mut atoms = parent.clone();
        atoms.pure_atom_paragraph = true;
        let sibling = parent.clone();
        assert!(text_turn(&parent));
        assert!(!text_turn(&atoms));
        assert!(text_turn(&sibling));
    }
}
