//! Финальные линии flex и координаты их элементов без округления устройства.
use super::{LayoutId, TaffyLayoutEngine};
use crate::{Pixels, Point};
use taffy::{DetailedLayoutInfo, Display, FlexDirection};

/// KaminIDE patch: завершённая раскладка flex в логических точках.
#[derive(Clone, Debug)]
pub struct FlexLayout {
    /// Whether the physical block axis is the main axis.
    pub is_column: bool,
    /// Final lines, in physical cross-axis order.
    pub lines: Vec<FlexLineLayout>,
}

/// Линия и её элементы в порядке финального размещения Taffy.
#[derive(Clone, Debug)]
pub struct FlexLineLayout {
    /// Physical cross-axis border-box position relative to the container.
    pub cross_offset: f32,
    /// Used cross-axis size.
    pub cross_size: f32,
    /// In-flow items assigned to this line by Taffy.
    pub items: Vec<FlexItemLayout>,
}

/// Поточная геометрия элемента: относительное позиционирование сюда не входит.
#[derive(Clone, Debug)]
pub struct FlexItemLayout {
    /// Layout node of the item.
    pub id: LayoutId,
    /// Physical main-axis border-box position relative to the container.
    pub main_offset: f32,
    /// Physical cross-axis border-box position relative to the container.
    pub cross_offset: f32,
    /// Used main-axis size.
    pub main_size: f32,
    /// Used cross-axis size.
    pub cross_size: f32,
    /// Resolved physical block-start (top) margin.
    pub margin_top: f32,
    /// Resolved physical block-end (bottom) margin.
    pub margin_bottom: f32,
}

impl TaffyLayoutEngine {
    /// KaminIDE patch: consumed cross extent of a row flex continuation.
    pub fn set_flex_item_fragmentation_consumed(&mut self, id: LayoutId, consumed: f32, scale: f32) -> bool {
        self.taffy.set_flex_item_fragmentation_consumed(id.into(), consumed * scale).is_ok()
    }

    /// KaminIDE patch: consumed block space before a column flex item.
    pub fn set_flex_item_fragmentation_offset(&mut self, id: LayoutId, offset: f32, scale: f32) -> bool {
        self.taffy.set_flex_item_fragmentation_offset(id.into(), offset * scale).is_ok()
    }
    /// KaminIDE patch: the item's flex line fixed before fragmentation.
    pub fn set_flex_item_fragmentation_line(&mut self, id: LayoutId, line: Option<usize>) -> bool {
        self.taffy.set_flex_item_fragmentation_line(id.into(), line).is_ok()
    }
    /// KaminIDE patch: atomic boxes cannot expand due to internal fragmentation.
    pub fn set_fragmentation_monolithic(&mut self, id: LayoutId, on: bool) -> bool {
        self.taffy.set_fragmentation_monolithic(id.into(), on).is_ok()
    }
    /// KaminIDE patch: the flex container is laid out in a fragmentation context.
    pub fn set_flex_fragmentation(&mut self, id: LayoutId, on: bool) -> bool {
        self.taffy.set_flex_fragmentation(id.into(), on).is_ok()
    }
    /// KaminIDE patch: consumed fragmentation space after physical row lines.
    pub fn set_flex_line_fragmentation_growth(&mut self, id: LayoutId, growth: &[f32], scale: f32) -> bool {
        self.taffy.set_flex_line_fragmentation_growth(id.into(), growth.iter().map(|v| v * scale).collect()).is_ok()
    }
    /// KaminIDE patch: реальные линии родительского flex, его начало и элементы.
    /// Грязное дерево не выдаёт метаданные предыдущего замера.
    pub fn parent_flex_layout(
        &mut self,
        child: LayoutId,
        scale: f32,
    ) -> Option<(Point<Pixels>, FlexLayout)> {
        let parent = self.taffy.parent(child.into())?;
        if self.taffy.style(parent).ok()?.display != Display::Flex {
            return None;
        }
        self.taffy.computed_layout_output(parent).ok()??;
        let DetailedLayoutInfo::Flex(info) = self.taffy.detailed_layout_info(parent) else {
            return None;
        };
        let layout = FlexLayout {
            is_column: matches!(info.direction, FlexDirection::Column | FlexDirection::ColumnReverse),
            lines: info.lines.iter().map(|line| FlexLineLayout {
                cross_offset: line.cross_offset / scale,
                cross_size: line.cross_size / scale,
                items: line.items.iter().map(|item| {
                    let margin = self.taffy.unrounded_layout(item.node).margin;
                    FlexItemLayout {
                        id: item.node.into(),
                        main_offset: item.main_offset / scale,
                        cross_offset: item.cross_offset / scale,
                        main_size: item.main_size / scale,
                        cross_size: item.cross_size / scale,
                        margin_top: margin.top / scale,
                        margin_bottom: margin.bottom / scale,
                    }
                }).collect(),
            }).collect(),
        };
        let origin = self.layout_origin_unrounded(parent.into(), scale);
        Some((origin, layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::{Dimension, FlexWrap, Style};
    use taffy::geometry::Size;
    use taffy::prelude::TaffyMaxContent;

    #[test]
    fn final_lines_are_scaled_and_dirty_geometry_is_rejected() {
        for scale in [1.0, 1.5] {
            let mut engine = TaffyLayoutEngine::new();
            let children: Vec<_> = (0..3).map(|_| engine.taffy.new_leaf(Style {
                size: Size::from_lengths(40.0 * scale, 20.0 * scale), flex_shrink: 0.0, ..Style::DEFAULT
            }).unwrap()).collect();
            let root = engine.taffy.new_with_children(Style {
                size: Size { width: Dimension::length(80.0 * scale), height: Dimension::auto() },
                flex_wrap: FlexWrap::Wrap, ..Style::DEFAULT
            }, &children).unwrap();
            engine.taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let child = children[0].into();
            let (_, lines) = engine.parent_flex_layout(child, scale).unwrap();
            assert!(!lines.is_column);
            assert_eq!(lines.lines.len(), 2);
            assert_eq!(lines.lines[0].items.len(), 2);
            assert_eq!(lines.lines[1].cross_offset, 20.0);
            assert!(engine.set_flex_line_fragmentation_growth(root.into(), &[30.0], scale));
            assert!(engine.parent_flex_layout(child, scale).is_none());
            engine.taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let (_, lines) = engine.parent_flex_layout(child, scale).unwrap();
            assert_eq!(lines.lines[1].cross_offset, 50.0);
            assert_eq!(lines.lines[0].items[0].cross_size, 20.0);
        }
    }
}
