//! Native atomic subtrees prepare in screen coordinates while surrounding text stays flat.
//! Cancelling the text frame preserves enclosing CSS transforms and native hitboxes.
use crate::text::vertical::{VT_CCW, VT_FRAME};
use gpui::{Bounds, Pixels, TransformationMatrix, point, px, size};

pub(super) fn map(flat: Bounds<Pixels>, scale: f32) -> (Bounds<Pixels>, TransformationMatrix) {
    let Some(frame) = VT_FRAME.with(|frame| frame.get()) else {
        return (flat, TransformationMatrix::unit());
    };
    project(frame, flat, scale, VT_CCW.with(|direction| direction.get()))
}

fn project(
    frame: Bounds<Pixels>,
    flat: Bounds<Pixels>,
    scale: f32,
    ccw: bool,
) -> (Bounds<Pixels>, TransformationMatrix) {
    let (x, y) = (frame.origin.x, frame.origin.y);
    let (along, across) = (flat.origin.x - x, flat.origin.y - y);
    let physical = Bounds {
        origin: if ccw {
            point(x + across, y + frame.size.height - along - flat.size.width)
        } else {
            point(x + frame.size.width - across - flat.size.height, y + along)
        },
        size: size(flat.size.height, flat.size.width),
    };
    let (x, y) = (f32::from(x) * scale, f32::from(y) * scale);
    let inverse = if ccw {
        TransformationMatrix {
            rotation_scale: [[0.0, -1.0], [1.0, 0.0]],
            translation: [x + y + f32::from(frame.size.height) * scale, y - x],
        }
    } else {
        TransformationMatrix {
            rotation_scale: [[0.0, 1.0], [-1.0, 0.0]],
            translation: [x - y, x + y + f32::from(frame.size.width) * scale],
        }
    };
    (physical, inverse)
}

pub(super) fn without_frame<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Bounds<Pixels>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            VT_FRAME.with(|frame| frame.set(self.0));
        }
    }
    let _restore = Restore(VT_FRAME.with(|frame| frame.replace(None)));
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_nonsquare_box_matches_turned_flat_corners_at_fractional_origins() {
        let frame = Bounds {
            origin: point(px(3.2), px(5.6)),
            size: size(px(125.0), px(295.0)),
        };
        let flat = Bounds {
            origin: point(px(13.2), px(25.6)),
            size: size(px(50.0), px(100.0)),
        };
        for ccw in [false, true] {
            let (physical, back) = project(frame, flat, 1.25, ccw);
            let expected = if ccw { (23.2, 240.6) } else { (8.2, 15.6) };
            assert!((f32::from(physical.origin.x) - expected.0).abs() < 0.0001);
            assert!((f32::from(physical.origin.y) - expected.1).abs() < 0.0001);
            assert_eq!(physical.size, size(px(100.0), px(50.0)));
            // Points already prepared in physical coordinates must survive
            // the enclosing text turn followed by its cancellation.
            let turn = if ccw {
                TransformationMatrix {
                    rotation_scale: [[0.0, 1.0], [-1.0, 0.0]],
                    translation: [(3.2 - 5.6) * 1.25, (3.2 + 5.6 + 295.0) * 1.25],
                }
            } else {
                TransformationMatrix {
                    rotation_scale: [[0.0, -1.0], [1.0, 0.0]],
                    translation: [(3.2 + 5.6 + 125.0) * 1.25, (5.6 - 3.2) * 1.25],
                }
            };
            let screen = physical.origin * 1.25;
            let result = back.compose(turn).apply(screen);
            assert!((f32::from(result.x - screen.x)).abs() < 0.0001);
            assert!((f32::from(result.y - screen.y)).abs() < 0.0001);
        }
    }
}
