//! Flex-линии читаются из final metadata, позиции — из normal geometry того же прохода.
use super::{Watched, axis::BlockAxis};
use crate::layout::fragment::flow::blocks::{FlexFrame, FlexLine};
use gpui::{Window, point, px, size};
use std::collections::HashMap;

pub(super) struct FlexFrames {
    pub(super) flexes: HashMap<u64, FlexFrame>,
    pub(super) margins: HashMap<u64, (f32, f32)>,
}

impl Watched {
    pub(super) fn flex_frames(
        &self,
        projection: BlockAxis,
        window: &mut Window,
        boxes: &mut HashMap<u64, (f32, f32)>,
    ) -> FlexFrames {
        let mut containers = Vec::new();
        if let Some(b) = &self.blocks {
            b.flexes(&mut containers);
        }
        let seen = self.seen.borrow();
        let mut out = HashMap::new();
        let mut margins = HashMap::new();
        for (id, items) in containers {
            let Some(lid) = items.iter().find_map(|id| seen.get(id)).copied() else {
                continue;
            };
            let Some(child) = lid.read(window) else {
                continue;
            };
            let Some(parent) = window.parent_layout_id(lid.id) else {
                continue;
            };
            let Some(normal) =
                crate::layout::fragment::geometry::Node::capture(parent, window).read(window)
            else {
                continue;
            };
            if child.space != normal.space {
                continue;
            }
            let Some((_, flex)) = window.parent_flex_layout(lid.id) else {
                continue;
            };
            let o = normal.normal.origin;
            let column = flex.is_column != projection.axis.is_vertical();
            let mut lines = Vec::new();
            for (index, line) in flex.lines.into_iter().enumerate() {
                let mut members = Vec::new();
                let mut order = Vec::new();
                let mut top = f32::MAX;
                let mut bottom = f32::MIN;
                for item in line.items {
                    let id = items
                        .iter()
                        .find(|id| seen.get(id).is_some_and(|node| node.id == item.id))
                        .copied();
                    order.push((item.order, id));
                    let (x, y, width, height) = if flex.is_column {
                        (
                            item.cross_offset,
                            item.main_offset,
                            item.cross_size,
                            item.main_size,
                        )
                    } else {
                        (
                            item.main_offset,
                            item.cross_offset,
                            item.main_size,
                            item.cross_size,
                        )
                    };
                    let (start, end) = projection
                        .span(point(o.x + px(x), o.y + px(y)), size(px(width), px(height)));
                    top = top.min(start);
                    bottom = bottom.max(end);
                    let Some(id) = id else { continue };
                    boxes.insert(id, (start, end));
                    let margin = match projection.axis {
                        crate::layout::fragment::types::StackAxis::Horizontal => {
                            (item.margin_top, item.margin_bottom)
                        }
                        crate::layout::fragment::types::StackAxis::VerticalLr => {
                            (item.margin_left, item.margin_right)
                        }
                        crate::layout::fragment::types::StackAxis::VerticalRl => {
                            (item.margin_right, item.margin_left)
                        }
                    };
                    margins.insert(id, margin);
                    members.push(id);
                }
                if order.is_empty() {
                    continue;
                }
                order.sort_unstable_by_key(|item| item.0);
                if !column {
                    let (a, b) = projection.relative_span(o, line.cross_offset, line.cross_size);
                    top = a;
                    bottom = b;
                }
                lines.push(FlexLine {
                    index,
                    start: top,
                    end: bottom,
                    items: members,
                    order,
                });
            }
            if !column {
                lines.sort_by(|a, b| a.start.total_cmp(&b.start));
            }
            out.insert(id, FlexFrame { column, lines });
        }
        FlexFrames {
            flexes: out,
            margins,
        }
    }
}
