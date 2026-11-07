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
        self.box_edges(parent.into(), scale_factor)
    }

    /// Used border and padding of `id` itself, in unrounded logical pixels.
    pub fn box_edges(
        &self,
        id: LayoutId,
        scale_factor: f32,
    ) -> Option<(Edges<Pixels>, Edges<Pixels>)> {
        let layout = self.taffy.layout(id.into()).ok()?;
        let logical = |r: TaffyRect<f32>| Edges {
            top: Pixels(r.top / scale_factor),
            right: Pixels(r.right / scale_factor),
            bottom: Pixels(r.bottom / scale_factor),
            left: Pixels(r.left / scale_factor),
        };
        Some((logical(layout.border), logical(layout.padding)))
    }

    /// The box an absolutely positioned child of `id` with `[top, right, bottom, left]`
    /// insets from the padding box would get, without creating that child: `(snapped,
    /// unrounded)` in logical pixels, snapped on absolute edges exactly as `layout_bounds`
    /// snaps a child. Lets a box paint its own background layers (CSS 2.1 Appendix E)
    /// at the geometry of a layer element, without the element and its layout node.
    pub fn inset_box(
        &mut self,
        id: LayoutId,
        [top, right, bottom, left]: [f32; 4],
        scale_factor: f32,
    ) -> Option<(Bounds<Pixels>, Bounds<Pixels>)> {
        let _ = self.layout_bounds(id, scale_factor);
        let (ax, ay) = self.absolute_unrounded.get(&id).copied()?;
        let layout = *self.taffy.layout(id.into()).ok()?;
        let (border, extent) = (layout.border, layout.size);
        // As `layout_bounds` places the child: parent origin (plus content shift) + the
        // child's location, which taffy computes as start inset + positioning-area offset.
        let x = ax + (left * scale_factor + border.left);
        let y = (ay + self.taffy.content_shift(id.into())) + (top * scale_factor + border.top);
        // The same f32 operations, in the same order, as taffy's stretch-fit size of an
        // absolute child: positioning area = border box - (start + end border), then the
        // inset-modified containing block (area - end inset) - start inset.
        let area_width = extent.width - (border.left + border.right);
        let area_height = extent.height - (border.top + border.bottom);
        let width = (area_width - right * scale_factor) - left * scale_factor;
        let height = (area_height - bottom * scale_factor) - top * scale_factor;
        let (width, height) = (width.max(0.0), height.max(0.0));
        let (rx, ry) = (x.round(), y.round());
        let snapped = Bounds {
            origin: point(Pixels(rx / scale_factor), Pixels(ry / scale_factor)),
            size: size(
                Pixels(((x + width).round() - rx) / scale_factor),
                Pixels(((y + height).round() - ry) / scale_factor),
            ),
        };
        let exact = Bounds {
            origin: point(Pixels(x / scale_factor), Pixels(y / scale_factor)),
            size: size(Pixels(width / scale_factor), Pixels(height / scale_factor)),
        };
        Some((snapped, exact))
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

    #[test]
    fn inset_box_matches_the_absolute_child_it_replaces() {
        use taffy::style_helpers::{length, percent};
        let insets = [[0.0; 4], [-1.5, -2.25, -0.75, -3.0], [0.4, 1.6, 2.5, 0.5]];
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for [t, r, b, l] in insets {
                let mut engine = TaffyLayoutEngine::new();
                let layer = engine
                    .taffy
                    .new_leaf(taffy::Style {
                        position: taffy::Position::Absolute,
                        inset: taffy::Rect {
                            top: length(t * scale),
                            right: length(r * scale),
                            bottom: length(b * scale),
                            left: length(l * scale),
                        },
                        ..Default::default()
                    })
                    .unwrap();
                let parent = engine
                    .taffy
                    .new_with_children(
                        taffy::Style {
                            // GPUI boxes are relative by default (gpui Style::default),
                            // so the box is its layer's containing block; taffy's own
                            // default is static.
                            position: taffy::Position::Relative,
                            size: taffy::Size {
                                width: percent(0.37),
                                height: length(20.3 * scale),
                            },
                            margin: taffy::Rect {
                                top: length(7.7 * scale),
                                right: length(0.0),
                                bottom: length(0.0),
                                left: length(3.3 * scale),
                            },
                            padding: taffy::Rect {
                                top: percent(0.013),
                                right: length(1.1 * scale),
                                bottom: length(0.0),
                                left: percent(0.021),
                            },
                            border: taffy::Rect {
                                top: length(1.0 * scale),
                                right: length(2.6 * scale),
                                bottom: length(1.4 * scale),
                                left: length(0.8 * scale),
                            },
                            ..Default::default()
                        },
                        &[layer],
                    )
                    .unwrap();
                let root = engine
                    .taffy
                    .new_with_children(
                        taffy::Style {
                            display: taffy::Display::Block,
                            size: taffy::Size {
                                width: length(401.7 * scale),
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
                let (snapped, exact) = engine
                    .inset_box(parent.into(), [t, r, b, l], scale)
                    .unwrap();
                assert_eq!(snapped, engine.layout_bounds(layer.into(), scale));
                assert_eq!(
                    exact.origin,
                    engine.layout_origin_unrounded(layer.into(), scale)
                );
                assert_eq!(
                    exact.size,
                    engine.layout_size_unrounded(layer.into(), scale)
                );
                assert_eq!(
                    engine.box_edges(parent.into(), scale),
                    engine.parent_box_edges(layer.into(), scale)
                );
            }
        }
    }
}
