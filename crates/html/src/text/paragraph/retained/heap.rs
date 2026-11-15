//! Обход уникальных удерживаемых раскладок и позиций без повторного счёта глифов.

use super::Retained;
use kamin_heap::{HeapSize, Ops};
use std::{rc::Rc, sync::Arc};

impl HeapSize for Retained {
    fn heap_size(&self, ops: &mut Ops) -> usize {
        self.state.try_borrow().map_or(0, |state| {
            let shape = state.shape.as_ref().map_or(0, |(inputs, segs)| {
                inputs.heap_size(ops)
                    + ops.buffer(segs.as_ptr(), segs.capacity())
                    + segments(segs, ops)
            });
            let lines = state.lines.as_ref().map_or(0, |(inputs, entries)| {
                inputs.heap_size(ops)
                    + ops.buffer(entries.as_ptr(), entries.capacity())
                    + entries
                        .iter()
                        .map(|(_, lines)| {
                            let own = ops.shallow(Rc::as_ptr(lines));
                            if own == 0 {
                                0
                            } else {
                                own + ops.buffer(lines.as_ptr(), lines.capacity())
                            }
                        })
                        .sum::<usize>()
            });
            let paint = ops.buffer(state.paint.as_ptr(), state.paint.capacity())
                + state
                    .paint
                    .iter()
                    .map(|piece| {
                        ops.buffer(piece.fonts.as_ptr(), piece.fonts.capacity())
                            + piece.text.heap_size(ops)
                            + layout(&piece.layout, ops)
                    })
                    .sum::<usize>();
            shape + lines + paint
        })
    }
}

fn segments(segs: &[super::super::Seg], ops: &mut Ops) -> usize {
    let mut total = 0;
    for seg in segs {
        total += layout(&seg.layout, ops);
        let own = ops.shallow(Rc::as_ptr(&seg.positions));
        if own > 0 {
            total += own
                + seg
                    .positions
                    .get()
                    .map_or(0, |p| ops.buffer(p.as_ptr(), p.capacity()));
        }
    }
    total
}

fn layout(layout: &Arc<gpui::LineLayout>, ops: &mut Ops) -> usize {
    let own = ops.shallow(Arc::as_ptr(layout));
    if own == 0 {
        return 0;
    }
    own + ops.buffer(layout.runs.as_ptr(), layout.runs.capacity())
        + layout
            .runs
            .iter()
            .map(|run| ops.buffer(run.glyphs.as_ptr(), run.glyphs.capacity()))
            .sum::<usize>()
}
