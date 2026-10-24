//! KaminIDE patch: использованные кромки родителя для фоновых слоёв.
//! Процентный padding уже разрешён раскладкой относительно containing block.

use super::*;

impl TaffyLayoutEngine {
    /// Used border and padding of the parent, in unrounded logical pixels.
    /// Absolute paint layers must not resolve CSS percentages a second time.
    pub fn parent_box_edges(
        &self,
        child: LayoutId,
        scale_factor: f32,
    ) -> Option<(Edges<Pixels>, Edges<Pixels>)> {
        let parent = self.taffy.parent(child.into())?;
        let layout = self.taffy.layout(parent).ok()?;
        let logical = |r: TaffyRect<f32>| Edges {
            top: Pixels(r.top / scale_factor),
            right: Pixels(r.right / scale_factor),
            bottom: Pixels(r.bottom / scale_factor),
            left: Pixels(r.left / scale_factor),
        };
        Some((logical(layout.border), logical(layout.padding)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;
    use taffy::prelude::TaffyMaxContent;

    #[test]
    fn overlay_reads_used_parent_edges_with_percentage_padding_and_scale() {
        use taffy::style_helpers::{length, percent};
        for scale in [1.0, 1.25, 2.0] {
            let mut engine = TaffyLayoutEngine::new();
            let overlay = engine
                .taffy
                .new_leaf(taffy::Style {
                    position: taffy::Position::Absolute,
                    ..Default::default()
                })
                .unwrap();
            let parent = engine
                .taffy
                .new_with_children(
                    taffy::Style {
                        box_sizing: taffy::BoxSizing::ContentBox,
                        size: taffy::Size {
                            width: length(100.0 * scale),
                            height: length(50.0 * scale),
                        },
                        padding: taffy::Rect {
                            top: percent(0.1),
                            right: percent(0.1),
                            bottom: percent(0.1),
                            left: percent(0.1),
                        },
                        border: taffy::Rect {
                            top: length(3.0 * scale),
                            right: length(3.0 * scale),
                            bottom: length(3.0 * scale),
                            left: length(3.0 * scale),
                        },
                        ..Default::default()
                    },
                    &[overlay],
                )
                .unwrap();
            let root = engine
                .taffy
                .new_with_children(
                    taffy::Style {
                        display: taffy::Display::Block,
                        size: taffy::Size {
                            width: length(400.0 * scale),
                            height: length(300.0 * scale),
                        },
                        ..Default::default()
                    },
                    &[parent],
                )
                .unwrap();
            engine
                .taffy
                .compute_layout(root, taffy::Size::MAX_CONTENT)
                .unwrap();
            let (border, padding) = engine.parent_box_edges(overlay.into(), scale).unwrap();
            assert_eq!(border, Edges::all(px(3.0)));
            assert_eq!(padding, Edges::all(px(40.0)));
            assert_eq!(engine.parent_box_edges(root.into(), scale), None);
        }
    }
}
