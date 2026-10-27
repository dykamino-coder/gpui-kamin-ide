//! Исходный CB/static anchor связывается с первым фрагментом normal-владельца.

use super::*;
use crate::layout::fragment::flow::coordinates::block_offset;

impl ColumnStack {
    pub(super) fn oof_inputs(&self, normal: &Plan) -> Result<(Vec<u64>, Vec<Input>), Error> {
        let mut ids = Vec::new();
        let mut inputs = Vec::new();
        for source in &self.oof.geometry.flows {
            if source.policy
                == crate::layout::fragment::token::resume::oof::FixedPolicy::RepeatPages
            {
                continue;
            }
            // CB/static anchor принадлежит исходному normal layout, а не копии.
            let Some(owner) = self.children.iter().position(|c| {
                c.watched
                    .as_ref()
                    .and_then(|w| w.blocks())
                    .is_some_and(|b| b.id == source.owner)
            }) else {
                return Err(Error::InvalidGeometry);
            };
            let offset = |rect: Bounds<Pixels>| {
                block_offset(
                    source.owner_rect.origin,
                    source.owner_rect.size,
                    rect.origin,
                    rect.size,
                    self.axis,
                )
            };
            let cb_start = offset(source.metrics.original_cb_rect);
            let Some(anchor) = normal
                .iter()
                .filter(|f| f.kid == owner)
                .find(|f| cb_start < f.from + f.h || f.from + f.h >= self.children[owner].h)
            else {
                return Err(Error::InvalidGeometry);
            };
            ids.push(source.id);
            inputs.push(Input {
                kid: source.kid.clone(),
                inline: source.inline.clone(),
                first: anchor.col,
                block_start: anchor.y + offset(source.metrics.normal_rect()) - anchor.from,
                inline_start: f32::from(if self.axis.is_vertical() {
                    source.metrics.normal_rect().origin.y - source.owner_rect.origin.y
                } else {
                    source.metrics.normal_rect().origin.x - source.owner_rect.origin.x
                }),
                repeat_fixed: false,
                fixed_content: source.fixed || source.inside_fixed,
            });
        }
        Ok((ids, inputs))
    }
}
