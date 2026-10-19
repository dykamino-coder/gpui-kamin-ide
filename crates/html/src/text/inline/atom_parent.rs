//! Стиль строчного предка для атома: наследование идёт от ближайшей коробки.
//!
//! Атомарная строчная коробка (`inline-block`, `inline-table`, замещаемый
//! элемент) — ребёнок того `<span>`, внутри которого стоит, и наследует от
//! него (CSS Cascade 4 §7: «the inherited value of a property on an element
//! is the computed value of the property on the element's parent»). Атом
//! строится замыканием абзаца со стилем БЛОКА, поэтому сбор кусков держит
//! (`CollectCx::inline_parents`) стек вычисленных стилей открытых строчных предков (без служебных полей
//! окраски строчной коробки — только `inherit`): `<b>a <span
//! style="display:inline-block">b</span></b>` терял полужирный у «b»
//! (`run-in-contains-inline-block-001`).

use crate::document::state::ValueCx;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;

/// What the piece collection tells the builder of an atom: the style of its
/// nearest open inline ancestor (`None` — the paragraph block is the parent)
/// and whether that ancestor chain holds a positioned inline, which may be the
/// atom's containing block (CSS 2.1 §10.1 item 4, `InlineCbOffer`).
#[derive(Clone, Default)]
pub struct AtomCx {
    pub(crate) parent: Option<Computed>,
    pub(crate) in_inline_cb: bool,
}

/// Open an inline ancestor on the collection's stack: its style inherits from
/// the top of the stack (or from `base`, the paragraph's style, when empty).
pub(crate) fn open(stack: &mut Vec<Computed>, own: &Computed, base: &Computed, vcx: ValueCx<'_>) {
    let parent = stack.last().unwrap_or(base);
    let style = inherit(parent, own, vcx);
    stack.push(style);
}
