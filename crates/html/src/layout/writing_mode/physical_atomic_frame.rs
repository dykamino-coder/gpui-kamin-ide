//! Перевод плоской коробки атома в экранные координаты повёрнутой рамки текста (`TurnFrame`:
//! границы, фаза, направление поворота) и обратная матрица, отменяющая поворот текста:
//! внешние CSS-трансформации и нативные hitbox сохраняются, потому что поворот снимается
//! матрицей, а не заменой трансформаций. `PhysicalAtomic` на время подготовки поддерева
//! сам убирает рамку из контекста рисования.
use crate::text::vertical::TurnFrame;
use gpui::{Bounds, Pixels, TransformationMatrix, point, size};

pub(super) fn map(
    flat: Bounds<Pixels>,
    scale: f32,
    frame: Option<TurnFrame>,
) -> (Bounds<Pixels>, TransformationMatrix) {
    let Some(frame) = frame else {
        return (flat, TransformationMatrix::unit());
    };
    project(frame.bounds, frame.phase, flat, scale, frame.ccw)
}

fn project(
    frame: Bounds<Pixels>,
    phase: Pixels,
    flat: Bounds<Pixels>,
    scale: f32,
    ccw: bool,
) -> (Bounds<Pixels>, TransformationMatrix) {
    let (x, y) = (frame.origin.x, frame.origin.y);
    let along = flat.origin.x - x;
    // The clockwise frame lays flat content out from `y + phase` (the turn phase).
    let across = flat.origin.y - y - if ccw { Pixels::ZERO } else { phase };
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
            translation: [x - y, x + y + f32::from(frame.size.width + phase) * scale],
        }
    };
    (physical, inverse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

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
            let (physical, back) = project(frame, Pixels::ZERO, flat, 1.25, ccw);
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

    #[test]
    fn clockwise_phase_keeps_flat_offsets_relative_to_the_shifted_origin() {
        // Flat content starts `phase` below the frame top; the box sits 10 px
        // into the block axis, so its physical right edge is 10 px left of
        // the frame's unrounded block-end edge.
        let frame = Bounds {
            origin: point(px(8.0), px(8.0)),
            size: size(px(46.75), px(88.0)),
        };
        let flat = Bounds {
            origin: point(px(12.0), px(8.25 + 10.0)),
            size: size(px(20.0), px(23.0)),
        };
        let (physical, back) = project(frame, px(0.25), flat, 1.0, false);
        assert_eq!(
            physical.origin,
            point(px(8.0 + 46.75 - 10.0 - 23.0), px(12.0))
        );
        let turn = TransformationMatrix {
            rotation_scale: [[0.0, -1.0], [1.0, 0.0]],
            translation: [8.0 + 8.25 + 46.75, 0.0],
        };
        let screen = physical.origin;
        let result = back.compose(turn).apply(screen);
        assert!((f32::from(result.x - screen.x)).abs() < 0.0001);
        assert!((f32::from(result.y - screen.y)).abs() < 0.0001);
    }
}
