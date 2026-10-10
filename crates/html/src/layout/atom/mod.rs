//! Строчные атомы: `atom_element`.
// owner: A

use crate::dom::Element;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::writing_mode::rotated_atom;
use crate::render::{RenderOpts, content_sized, content_sized_wraps, replaced_tag};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

mod own_box;
mod pct;
mod positioned;
mod replaced;
pub(super) use pct::pct_resolved_against_block;
pub(crate) use pct::pct_resolved_for_wrapper;
mod raw;
use raw::atom_element_raw;

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
/// Строчный атом с размером по ключевому слову (`width: min-content` и
/// родня): дорожка по содержимому ставится той же обёрткой, что у блочного
/// пути (`content_sized`). Без неё атом шёл в ряд строки голым, гибкий ряд
/// брал его основу по max-content, и `inline-grid`/`inline grid-lanes` с
/// `width: min-content` раскладывался по max-content: доли `1fr 2fr 1fr 1fr`
/// при базах по 2ch раздавались, вторая дорожка выходила 4ch
/// (`grid-lanes-intrinsic-sizing-cols-002-fr`; css-sizing-3 §5.1).
pub(crate) fn atom_element(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if let Some(physical) = rotated_atom::physical(e, inherited, opts) {
        return Some(physical);
    }
    let resolved = rotated_atom::resolved(e, inherited);
    let e = resolved.as_ref().unwrap_or(e);
    let el = atom_element_raw(e, inherited, opts)?;
    // CSS Sizing 3 §5.1: intrinsic keywords in the block axis behave as auto.
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    // Повёрнутый абзац вертикального письма (`rotated_line`) набирается
    // горизонтально, но ось строки там вертикальна.
    let inline_axis_only = inherited.vertical != Some(true)
        && inherited.rotated_line != Some(true)
        && e.style.vertical != Some(true)
        && keyword(e.style.width)
        && !keyword(e.style.height);
    let wraps = inline_axis_only
        && content_sized_wraps(e)
        && !replaced_tag(e)
        && !at_static_position(&e.style)
        && !matches!(e.tag.as_str(), "input" | "select" | "textarea" | "button");
    Some(if wraps {
        content_sized(el, &e.style, &inherit(inherited, &e.style), (None, None))
    } else {
        el
    })
}
