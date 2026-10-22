//! Поиск приемлемого якоря в сохранённом реестре прошлого кадра.

use super::super::AnchorRec;
use super::super::settle::DefaultAnchor;
use crate::document::state::ValueCx;

/// Якорь из реестра ПРОШЛОГО кадра: по имени — последняя запись с этим
/// именем, собранная РАНЬШЕ коробки (`seq`), — «the last element in tree
/// order» среди «laid out strictly before» (§target anchor element);
/// неявный — по `node_id` хозяина.
pub(super) fn last_lookup(
    name: Option<&str>,
    default: &Option<DefaultAnchor>,
    seq: u32,
    vcx: ValueCx<'_>,
) -> Option<AnchorRec> {
    match name {
        Some(n) => last_named(n, seq, vcx),
        None => match default {
            Some(DefaultAnchor::Named(n)) => last_named(n, seq, vcx),
            Some(DefaultAnchor::Implicit(id)) => {
                vcx.doc.anchor_frame.last_implicit.borrow().get(id).copied()
            }
            None => None,
        },
    }
}

/// Последняя запись прошлого кадра с этим именем, собранная раньше `seq`.
pub(in crate::layout::positioned::anchor) fn last_named(
    n: &str,
    seq: u32,
    vcx: ValueCx<'_>,
) -> Option<AnchorRec> {
    {
        let v = &vcx.doc.anchor_frame.last_named;
        v.borrow()
            .iter()
            .filter(|(k, s, _)| k == n && *s < seq)
            .max_by_key(|(_, s, _)| *s)
            .map(|(_, _, r)| *r)
    }
}
