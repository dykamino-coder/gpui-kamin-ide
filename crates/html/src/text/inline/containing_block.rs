//! Содержащий блок внепоточных потомков при сборе кусков абзаца: предложение строчной
//! коробки содержащим блоком (`InlineCbOffer`) и пометка абсолютов с краями, чей содержащий
//! блок — строчная коробка (`InlineCb`), метками её краёв в списке кусков. Предикаты
//! `establishes_cb`/`backdrop_root` — в `style::computed::box_predicates`.

use super::{InlineCb, OverlayAt, Piece};
use crate::dom::Element;
use crate::style::values::value::px_lossy_zero;
use gpui::IntoElement;

/// A positioned inline ancestor offered as the containing block of the atom
/// being built (CSS 2.1 §10.1 item 4): the paragraph's atom builder makes the
/// offer (`LayoutCx::inline_cb`), the first absolute atom that can use it
/// takes it, and the builder learns whether it was taken.
#[derive(Debug)]
pub(crate) struct InlineCbOffer {
    offered: std::cell::Cell<bool>,
    taken: std::cell::Cell<bool>,
}

impl InlineCbOffer {
    pub(crate) fn offered() -> std::rc::Rc<Self> {
        std::rc::Rc::new(Self {
            offered: std::cell::Cell::new(true),
            taken: std::cell::Cell::new(false),
        })
    }

    /// Is the offer still open? Asking consumes it (one atom per offer).
    pub(crate) fn take(&self) -> bool {
        self.offered.replace(false)
    }

    /// The atom was built for the inline containing block.
    pub(crate) fn note_taken(&self) {
        self.taken.set(true);
    }

    pub(crate) fn taken(&self) -> bool {
        self.taken.get()
    }
}

/// Отметить куски-абсолюты с краями (`OverlayAt::edges`) содержимого
/// строчной коробки `e`, у которых содержащего блока ещё нет: ближайший
/// позиционированный предок — она.
pub(super) fn mark_inline_cb(pieces: Vec<Piece>, e: &Element) -> Vec<Piece> {
    if !pieces
        .iter()
        .any(|p| matches!(p, Piece::Overlay(_, how) if how.edges && how.cb.is_none()))
    {
        return pieces;
    }
    static CB_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = CB_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let px_of = px_lossy_zero;
    let pad = [
        px_of(e.style.used().padding.top),
        px_of(e.style.used().padding.right),
        px_of(e.style.used().padding.bottom),
        px_of(e.style.used().padding.left),
    ];
    let marker = |start: bool| {
        Piece::Overlay(
            gpui::Empty.into_any_element(),
            OverlayAt {
                cb_marker: Some((id, start)),
                ..Default::default()
            },
        )
    };
    let mut out = Vec::with_capacity(pieces.len() + 2);
    out.push(marker(true));
    out.extend(pieces.into_iter().map(|p| match p {
        Piece::Overlay(el, how) if how.edges && how.cb.is_none() => Piece::Overlay(
            el,
            OverlayAt {
                cb: Some(InlineCb {
                    id,
                    start: 0,
                    end: 0,
                    pad,
                    shift: (0.0, 0.0),
                }),
                ..how
            },
        ),
        other => other,
    }));
    out.push(marker(false));
    out
}
