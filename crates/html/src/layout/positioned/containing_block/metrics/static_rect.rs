//! Статическое место из завершённой строки; intrinsic callbacks не публикуют геометрию.

use super::super::Spot;
use crate::layout::fragment::geometry::Node;
use gpui::{LayoutSpace, Window};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LineGeometry {
    Paragraph { node: Node, key: usize },
    Row(Node),
}

impl LineGeometry {
    pub(crate) fn read(self, mut spot: Spot, window: &Window) -> Option<(Spot, LayoutSpace)> {
        let (node, key) = match self {
            Self::Row(node) => {
                let layout = node.read(window)?;
                let line = window.final_flex_line(node.id_in(window)?, layout.space.epoch.tree)?;
                spot.hole = Some(gpui::Bounds {
                    origin: gpui::point(layout.normal.origin.x, line.origin.y),
                    size: gpui::size(layout.normal.size.width, line.size.height),
                });
                spot.line_box = Some((line.origin.y, line.size.height, line.origin.x));
                return Some((spot, layout.space));
            }
            Self::Paragraph { node, key } => (node, key),
        };
        let layout = node.read(window)?;
        let measured =
            window.final_static_position(node.id_in(window)?, layout.space.epoch.tree, key)?;
        let origin = layout.normal.origin;
        // Paragraph metadata уже разрешила hypothetical block на границе
        // реальной строки; поздний consumer не должен прибавлять её шаг ещё раз.
        if spot.next_line.take().is_some() {
            spot.block_strut = true;
        }
        spot.hole = Some(gpui::Bounds {
            origin: origin + measured.rect.origin,
            size: measured.rect.size,
        });
        spot.line_box = Some((
            origin.y + measured.line.origin.y,
            measured.line.size.height,
            origin.x + measured.line.origin.x,
        ));
        Some((spot, layout.space))
    }
}

impl Spot {
    pub(crate) fn normal(mut self, window: &Window) -> Option<(Self, LayoutSpace)> {
        let layout = self.normal_node?.read(window)?;
        self.hole = Some(layout.normal);
        Some((self, layout.space))
    }
}
