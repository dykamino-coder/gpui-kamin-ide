//! Fragment clipping and Taffy box edges use the same device-pixel grid.

use gpui::{Bounds, ContentMask, Pixels, point, px, size};

pub(super) fn snap(bounds: Bounds<Pixels>, scale: f32) -> ContentMask<Pixels> {
    let scale = scale.max(0.01);
    let edge = |value: Pixels| px((f32::from(value) * scale).round() / scale);
    let left = edge(bounds.origin.x);
    let top = edge(bounds.origin.y);
    let right = edge(bounds.origin.x + bounds.size.width);
    let bottom = edge(bounds.origin.y + bounds.size.height);
    ContentMask {
        bounds: Bounds {
            origin: point(left, top),
            size: size(right - left, bottom - top),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_fragment_masks_share_an_edge_at_fractional_scales() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut previous = None;
            for i in 0..4 {
                let mask = snap(
                    Bounds {
                        origin: point(px(8.0 + i as f32 * 25.0), px(17.3)),
                        size: size(px(25.0), px(100.0)),
                    },
                    scale,
                );
                if let Some(previous) = previous {
                    assert_eq!(previous, mask.bounds.origin.x);
                }
                previous = Some(mask.bounds.origin.x + mask.bounds.size.width);
            }
        }
    }

    #[test]
    fn clipping_uses_absolute_edges_instead_of_rounding_the_size() {
        let mask = snap(
            Bounds {
                origin: point(px(58.0), px(-23.2)),
                size: size(px(50.0), px(0.6)),
            },
            1.25,
        );
        assert_eq!(mask.bounds.origin.x, px(58.4));
        assert_eq!(mask.bounds.origin.x + mask.bounds.size.width, px(108.0));
        assert_eq!(mask.bounds.origin.y, px(-23.2));
        assert_eq!((f32::from(mask.bounds.size.height) * 1.25).round(), 1.0);
    }
}
