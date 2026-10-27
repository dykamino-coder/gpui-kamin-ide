//! Перенос исходного CB/static anchor в normal fragment; повтору fixed нужен CB своего листа.

use super::*;
use crate::layout::fragment::flow::coordinates::block_offset;
use crate::layout::fragment::plan::oof::stream::Input;
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use gpui::{Bounds, Pixels};

type Sources = Vec<(usize, Source)>;

impl PageStack {
    pub(super) fn oof_inputs(
        &self,
        normal: &Plan,
        pages: &Pages<'_>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(Sources, Vec<Input>), Error> {
        let mut sources = Vec::new();
        let mut inputs = Vec::new();
        for (owner, kid) in self.kids.iter().enumerate() {
            let Some(build) = &kid.oof else { continue };
            let first = normal.iter().find(|f| f.kid == owner).map_or(0, |f| f.col);
            let geom = pages.geometry(first);
            let snapshot = build(geom.area, window, cx);
            self.oof
                .snapshots
                .borrow_mut()
                .push((owner, snapshot.clone()));
            for source in &snapshot.flows {
                inputs.push(self.oof_input(source, owner, normal)?);
                sources.push((owner, source.clone()));
            }
        }
        Ok((sources, inputs))
    }

    fn oof_input(&self, source: &Source, owner: usize, normal: &Plan) -> Result<Input, Error> {
        let offset = |rect: Bounds<Pixels>| {
            block_offset(
                source.owner_rect.origin,
                source.owner_rect.size,
                rect.origin,
                rect.size,
                self.axis,
            )
        };
        let root_cb = source.metrics.cb_id == 0;
        let cb_start = offset(source.metrics.original_cb_rect);
        let anchor = normal
            .iter()
            .filter(|f| f.kid == owner)
            .find(|f| cb_start < f.from + f.h || f.from + f.h >= self.kids[owner].end.get());
        let (first, start) = if root_cb && !source.static_block {
            (
                0,
                block_offset(
                    source.metrics.original_cb_rect.origin,
                    source.metrics.original_cb_rect.size,
                    source.metrics.normal_rect().origin,
                    source.metrics.normal_rect().size,
                    self.axis,
                ),
            )
        } else if let Some(anchor) = anchor {
            (
                anchor.col,
                anchor.y + offset(source.metrics.normal_rect()) - anchor.from,
            )
        } else if source.owner == 0 {
            // Прямой OOF не занимает normal fragment: static cursor стоит
            // после предыдущей normal-коробки либо в начале пустого ICB.
            let cursor = normal
                .iter()
                .filter(|f| f.kid < owner)
                .max_by(|a, b| a.kid.cmp(&b.kid).then(a.copy.cmp(&b.copy)));
            (
                cursor.map_or(0, |f| f.col),
                cursor.map_or(0., |f| f.y + f.h) + offset(source.metrics.normal_rect()),
            )
        } else {
            return Err(Error::InvalidGeometry);
        };
        let repeat_fixed = source.policy == FixedPolicy::RepeatPages;
        Ok(Input {
            kid: source.kid.clone(),
            inline: source.inline.clone(),
            first: if repeat_fixed { 0 } else { first },
            block_start: start,
            inline_start: f32::from(if self.axis.is_vertical() {
                source.metrics.normal_rect().origin.y - source.metrics.original_cb_rect.origin.y
            } else {
                source.metrics.normal_rect().origin.x - source.metrics.original_cb_rect.origin.x
            }),
            repeat_fixed,
            fixed_content: source.fixed || source.inside_fixed,
        })
    }

    pub(super) fn refresh_fixed(
        &self,
        sources: &[(usize, Source)],
        context: &Context<'_>,
        index: usize,
        streams: &mut Streams,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), Error> {
        for (owner, kid) in self.kids.iter().enumerate() {
            if !sources
                .iter()
                .any(|(o, s)| *o == owner && s.policy == FixedPolicy::RepeatPages)
            {
                continue;
            }
            let build = kid.oof.as_ref().ok_or(Error::InvalidGeometry)?;
            let snapshot = build(context.pages.geometry(index).area, window, cx);
            for (id, (_, old)) in sources
                .iter()
                .enumerate()
                .filter(|(_, (o, s))| *o == owner && s.policy == FixedPolicy::RepeatPages)
            {
                let source = snapshot
                    .flows
                    .iter()
                    .find(|s| s.id == old.id)
                    .ok_or(Error::InvalidGeometry)?;
                streams.refresh_fixed(id, self.oof_input(source, owner, context.normal)?)?;
                self.oof
                    .fixed
                    .borrow_mut()
                    .push((index, id, source.clone()));
            }
            self.oof
                .repeat_snapshots
                .borrow_mut()
                .push((index, owner, snapshot.clone()));
        }
        Ok(())
    }
}
