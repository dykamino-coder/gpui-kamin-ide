//! Адаптер вычисленного запроса к нейтральному matching; формулы живут в font_facts.

use crate::document::state::ValueCx;
pub(crate) use crate::font_facts::matching::{Face, range, slope};
use crate::style::computed::Computed;

#[cfg(test)]
fn select(family: &str, request: &Computed, vcx: ValueCx<'_>) -> Option<Face> {
    vcx.doc
        .font_snapshot()
        .matching_face(family, request.font_request(family, 0.0))
}

pub(crate) fn alias_for(family: &str, request: &Computed, vcx: ValueCx<'_>) -> Option<String> {
    vcx.doc
        .font_snapshot()
        .alias_for(family, request.font_request(family, 0.0))
}

#[cfg(test)]
fn features(family: &str, request: &Computed, vcx: ValueCx<'_>) -> Option<Vec<(String, u32)>> {
    let face = select(family, request, vcx)?;
    Some(crate::font_facts::matching::features(
        face,
        &request.font_request(family, 0.0),
    ))
}

#[cfg(test)]
mod tests;
