//! Static-position rect из тех же строк и advances, которые завершили normal layout.

use crate::layout::fragment::geometry::Node;
use crate::layout::positioned::containing_block::metrics::static_rect::LineGeometry;
use crate::text::{inline::OverlayAt, paragraph::Paragraph};
use gpui::{App, AvailableSpace, Pixels, Window, size};
mod geometry;

#[derive(Clone)]
pub(crate) struct NormalStatic {
    overlays: Vec<(usize, OverlayAt)>,
}

impl NormalStatic {
    pub(super) fn new(paragraph: &mut Paragraph, window: &mut Window, cx: &mut App) -> Self {
        for (_, el, how) in &mut paragraph.overlays {
            if how.normal_probe.is_some() {
                el.layout_as_root(
                    size(AvailableSpace::MaxContent, AvailableSpace::MaxContent),
                    window,
                    cx,
                );
            }
        }
        Self {
            overlays: paragraph
                .overlays
                .iter()
                .filter(|(_, _, h)| h.line_probe.is_some() || h.normal_probe.is_some())
                .map(|(at, _, h)| (*at, h.clone()))
                .collect(),
        }
    }

    pub(super) fn bind(&self, node: Node) {
        for (key, (_, how)) in self.overlays.iter().enumerate() {
            let geometry = LineGeometry::Paragraph { node, key };
            if let Some(slot) = &how.line_probe {
                slot.normal.set(Some(geometry));
            }
            if let Some(slot) = &how.normal_probe {
                slot.normal.set(Some(geometry));
            }
        }
    }

    pub(super) fn measure(
        &self,
        paragraph: &Paragraph,
        extent: gpui::Size<Pixels>,
        window: &mut Window,
    ) -> Vec<gpui::MeasuredStaticPosition> {
        if self.overlays.is_empty() || paragraph.vertical || paragraph.paint_context.is_rotated() {
            return Vec::new();
        }
        let mut measured = Vec::new();
        let segs = paragraph.measure(window);
        let pads = paragraph.line_padding();
        for (key, (at, how)) in self.overlays.iter().enumerate() {
            let past_break = !paragraph.wrap.rtl
                && paragraph.lines.last().is_some_and(|l| *at >= l.range.end)
                && paragraph.text.get(..*at).is_some_and(|t| {
                    t.trim_end_matches([' ', '\u{feff}', '\u{200b}'])
                        .ends_with('\n')
                });
            let kind = if how.next_line {
                geometry::Position::Block
            } else if past_break {
                geometry::Position::AfterBreak
            } else {
                geometry::Position::Inline
            };
            let (rect, line) = geometry::rect(paragraph, &segs, &pads, *at, extent.width, kind);
            measured.push(gpui::MeasuredStaticPosition { key, rect, line });
        }
        measured
    }
}
