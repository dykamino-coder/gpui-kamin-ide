//! Условия сброса последнего успешного варианта (css-anchor-position-1 §fallback).

use super::owned::Owned;
use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Display, Position};
use crate::style::css::DeclBlock;
use crate::style::values::position_area::TryFallback;

#[derive(Clone, PartialEq)]
pub(super) struct Signature {
    accepted: Owned,
    position: Option<Position>,
    display: Option<Display>,
    cb: u64,
    order: u8,
    fallbacks: Vec<TryFallback>,
    rules: Vec<(String, Option<DeclBlock>)>,
}

impl Signature {
    pub(super) fn of(c: &Computed, vcx: ValueCx<'_>) -> Self {
        Self {
            accepted: Owned::of(c, vcx),
            position: c.position,
            display: c.display,
            cb: c.cb_node,
            order: c.position_try_order,
            fallbacks: c.position_try_fallbacks.clone(),
            rules: c
                .position_try_fallbacks
                .iter()
                .filter_map(|f| {
                    let n = f.name.as_ref()?;
                    Some((n.clone(), vcx.doc.try_rule(n)))
                })
                .collect(),
        }
    }
}
