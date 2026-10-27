//! Табличные полосы P18 выдаются в исходном CSS-слоте, внутри эффектов предка.

use super::*;
use crate::hooks::SourceSlot;
use crate::layout::fragment::{fragment_mask, types::Repeat};
use std::cell::RefCell;

#[cfg(test)]
mod tests;

struct Band {
    fragment: Frag,
    origin: gpui::Point<Pixels>,
    slice: PaintSegmentSlice,
    tail: Option<(Bounds<Pixels>, gpui::Hsla)>,
}

struct Slot {
    repeat: Option<Repeat>,
    bands: Vec<Band>,
    width: f32,
    spill: Pixels,
}

impl Slot {
    fn paint(&mut self, window: &mut Window, cx: &mut App) -> Result<(), PaintRecordError> {
        for band in &self.bands {
            let scale = window.scale_factor();
            window.with_paint_segment_slices(Rc::from([band.slice]), |window| {
                if let Some(repeat) = &mut self.repeat {
                    repeat.paint_bands(&band.fragment, |el, top, height| {
                        let mask = fragment_mask::snap(
                            Bounds::new(
                                band.origin + point(-self.spill, px(top)),
                                size(px(self.width) + self.spill + self.spill, px(height)),
                            ),
                            scale,
                        );
                        window.with_content_mask(Some(mask), |window| el.paint(window, cx));
                    });
                }
                if let Some((bounds, color)) = band.tail {
                    window.paint_quad(gpui::fill(fragment_mask::snap(bounds, scale).bounds, color));
                }
            })?;
        }
        Ok(())
    }
}

impl ColumnStack {
    pub(super) fn table_slots(
        &mut self,
        bounds: Bounds<Pixels>,
        plan: &[Frag],
        routes: &mut Vec<PaintSegment>,
        ordinals: &mut HashMap<usize, u32>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<HashMap<u64, SourceSlot>, PaintRecordError> {
        let mut slots = HashMap::new();
        let width = self.col_w.get();
        let spill = window.viewport_size().width.max(bounds.size.width);
        for index in 0..self.children.len() {
            if self.children[index].repeat.is_none() && self.children[index].slack.is_none() {
                continue;
            }
            let id = self.children[index]
                .watched
                .as_ref()
                .and_then(|w| w.blocks())
                .map(|b| b.id)
                .ok_or(PaintRecordError::InvalidGeometry)?;
            let mut bands = Vec::new();
            for f in plan.iter().filter(|f| f.kid == index) {
                let (column, row) = self.place(f.col);
                let kid = &mut self.children[index];
                let origin = bounds.origin
                    + point(
                        px(column as f32 * (width + self.gap) + kid.rel.0 + kid.par.dx),
                        px(row + f.y + kid.rel.1),
                    );
                if let Some(repeat) = &mut kid.repeat {
                    repeat.place_bands(f, |el, at| {
                        el.layout_as_root_at(
                            origin + point(px(0.), px(at)),
                            size(
                                gpui::AvailableSpace::Definite(px(width)),
                                gpui::AvailableSpace::Definite(px(kid.h)),
                            ),
                            window,
                            cx,
                        );
                        el.prepaint_at(point(px(0.), px(0.)), window, cx);
                    });
                }
                let ordinal = ordinals.entry(f.col).or_default();
                let placement = PaintSegment {
                    id: PaintSegmentId(f.col as u64),
                    ordinal: *ordinal,
                    source: point(px(0.), px(0.)),
                    destination: point(px(0.), px(0.)),
                };
                *ordinal = ordinal
                    .checked_add(1)
                    .ok_or(PaintRecordError::InvalidGeometry)?;
                routes.push(placement);
                let tail = kid
                    .slack
                    .filter(|_| plan.iter().any(|g| g.kid == index && g.copy == f.copy + 1))
                    .and_then(|color| {
                        let line = if self.rows.is_some_and(|r| r.wrap) {
                            f.col / self.count
                        } else {
                            0
                        };
                        let line_height = self.lines_plan.borrow().get(line).map_or(0., |l| l.1);
                        let footer = kid.repeat.as_ref().and_then(|r| r.foot).map_or(0., |b| b.1);
                        let height = if f.foot > crate::tolerance::COORD_EPS {
                            f.foot - footer
                        } else {
                            line_height - f.y - f.h
                        };
                        let width = self
                            .oof
                            .geometry
                            .normal
                            .get(&id)
                            .map_or(0., |b| f32::from(b.size.width))
                            .min(width);
                        (height > crate::tolerance::COORD_EPS
                            && width > crate::tolerance::COORD_EPS)
                            .then_some((
                                Bounds::new(
                                    origin + point(px(0.), px(f.h)),
                                    size(px(width), px(height)),
                                ),
                                color,
                            ))
                    });
                let clip = Bounds::new(
                    origin - point(spill, px(f.head)),
                    size(
                        px(width) + spill + spill,
                        px(f.head + f.h + f.foot)
                            .max(tail.map_or(px(0.), |(b, _)| b.bottom() - origin.y + px(f.head))),
                    ),
                );
                bands.push(Band {
                    fragment: *f,
                    origin,
                    slice: PaintSegmentSlice {
                        placement,
                        source_clip: clip,
                    },
                    tail,
                });
            }
            let slot: Rc<RefCell<Slot>> = Rc::new(RefCell::new(Slot {
                repeat: self.children[index].repeat.take(),
                bands,
                width,
                spill,
            }));
            let callback: SourceSlot =
                Rc::new(move |window, cx| slot.borrow_mut().paint(window, cx));
            slots.insert(id, callback);
        }
        Ok(slots)
    }
}
