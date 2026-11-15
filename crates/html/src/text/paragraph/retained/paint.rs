//! Храним геометрию нарисованных LTR-кусков, оформление строим из свежих красок.

use super::{Paragraph, Retained, inputs};
use gpui::{LineLayout, ShapedLine, SharedString, TextRun, Window};
use std::{ops::Range, sync::Arc};

pub(super) struct Piece {
    pub(super) range: Range<usize>,
    pub(super) fonts: Vec<u8>,
    pub(super) text: SharedString,
    pub(super) layout: Arc<LineLayout>,
}

impl Retained {
    pub(in crate::text::paragraph) fn painted(
        &self,
        p: &Paragraph,
        range: &Range<usize>,
        runs: &[TextRun],
        window: &Window,
    ) -> Option<ShapedLine> {
        let mut state = self.state.borrow_mut();
        state.validate(window.text_system());
        if !state.matches(p) {
            return None;
        }
        let hit = state
            .paint
            .iter()
            .find(|piece| piece.range == *range && inputs::matches_fonts(&piece.fonts, runs))?;
        let mut decorated = super::super::trim_runs(runs, &p.text[range.clone()]);
        super::super::controlled_shape::pad_runs(&mut decorated);
        Some(ShapedLine::from_retained_layout(
            hit.layout.clone(),
            hit.text.clone(),
            &decorated,
        ))
    }

    pub(in crate::text::paragraph) fn remember_paint(
        &self,
        p: &Paragraph,
        range: Range<usize>,
        fonts: Vec<u8>,
        shaped: &ShapedLine,
    ) {
        let mut state = self.state.borrow_mut();
        if !state.matches(p) {
            return;
        }
        if state
            .paint
            .iter()
            .any(|piece| piece.range == range && piece.fonts == fonts)
        {
            return;
        }
        // Ограничение на необычное число ширин/строк одного потока.
        if state.paint.len() == 64 {
            state.paint.remove(0);
        }
        state.paint.push(Piece {
            range,
            fonts,
            text: shaped.text.clone(),
            layout: shaped.retained_layout(),
        });
    }
}
