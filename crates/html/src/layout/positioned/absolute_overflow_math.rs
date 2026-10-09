//! CSS Align 3 abspos safety uses the union of the IMCB and original CB.
//! https://drafts.csswg.org/css-align-3/#auto-safety-position

#[derive(Clone, Copy)]
pub(super) struct Span {
    pub start: f32,
    pub end: f32,
}

pub(super) fn place(extent: f32, desired: f32, imcb: Span, cb: Span, from_end: bool) -> f32 {
    if extent <= imcb.end - imcb.start {
        return desired.clamp(imcb.start, imcb.end - extent);
    }
    let start = imcb.start.min(cb.start);
    let end = imcb.end.max(cb.end);
    if extent <= end - start {
        return desired.clamp((imcb.end - extent).max(start), imcb.start.min(end - extent));
    }
    if from_end { end - extent } else { start }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CB: Span = Span {
        start: 0.0,
        end: 200.0,
    };
    #[test]
    fn overflowing_static_rect_can_preserve_center_inside_original_cb() {
        assert_eq!(
            place(
                64.0,
                118.0,
                Span {
                    start: 125.0,
                    end: 175.0
                },
                CB,
                false
            ),
            118.0
        );
        assert_eq!(
            place(
                64.0,
                -2.0,
                Span {
                    start: 5.0,
                    end: 55.0
                },
                CB,
                false
            ),
            0.0
        );
    }
    #[test]
    fn outside_imcb_expands_the_limit_and_larger_subject_falls_to_logical_start() {
        assert_eq!(
            place(
                60.0,
                195.0,
                Span {
                    start: 200.0,
                    end: 250.0
                },
                CB,
                false
            ),
            190.0
        );
        let imcb = Span {
            start: 125.0,
            end: 175.0,
        };
        assert_eq!(place(240.0, 100.0, imcb, CB, false), 0.0);
        assert_eq!(place(240.0, 100.0, imcb, CB, true), -40.0);
        assert_eq!(place(40.0, 130.0, imcb, CB, false), 130.0);
    }
}
