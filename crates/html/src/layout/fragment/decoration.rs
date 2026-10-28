//! Геометрия собственной краски фрагмента отделена от допуска его содержимого.

use super::types::{StackAxis, axis_box};
use gpui::{Bounds, PaintSegmentSlice, Pixels};

/// CSS Break 3 §5.4: clone получает коробку фрагмента, slice — общую коробку.
/// Размещение и допуск принадлежат тому же плану, что и содержимое узла.
#[derive(Clone, Copy)]
pub(crate) struct Decoration {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) slice: PaintSegmentSlice,
}

impl Decoration {
    /// Clone оборачивает содержимое каждого фрагмента заново; slice сохраняет
    /// общую коробку, а обрезка остаётся частью исходного допуска команд.
    pub(crate) fn for_slice(
        bounds: Bounds<Pixels>,
        slice: PaintSegmentSlice,
        edges: Option<(StackAxis, f32, f32)>,
    ) -> Option<Self> {
        let Some((axis, head, tail)) = edges else {
            return Some(Self { bounds, slice });
        };
        let block = if axis.is_vertical() {
            bounds.size.width
        } else {
            bounds.size.height
        };
        let inline = if axis.is_vertical() {
            bounds.size.height
        } else {
            bounds.size.width
        };
        let (low, high) = match axis {
            StackAxis::Horizontal => (
                slice.source_clip.top() - bounds.top(),
                slice.source_clip.bottom() - bounds.top(),
            ),
            StackAxis::VerticalLr => (
                slice.source_clip.left() - bounds.left(),
                slice.source_clip.right() - bounds.left(),
            ),
            StackAxis::VerticalRl => (
                bounds.right() - slice.source_clip.right(),
                bounds.right() - slice.source_clip.left(),
            ),
        };
        let start = gpui::px(head).max(low);
        let end = (block - gpui::px(tail)).min(high);
        if end <= start {
            return None;
        }
        let own = axis_box(
            axis,
            bounds.origin,
            block.into(),
            0.,
            inline.into(),
            (start - gpui::px(head)).into(),
            (end - start + gpui::px(head + tail)).into(),
        );
        let mut admission: PaintSegmentSlice = slice;
        // Декорации имеют собственное ink overflow, содержимое — свой срез.
        let spill = if axis.is_vertical() {
            slice.source_clip.size.height
        } else {
            slice.source_clip.size.width
        };
        if axis.is_vertical() {
            admission.source_clip = Bounds::new(
                gpui::point(own.left() - spill, slice.source_clip.top()),
                gpui::size(
                    own.size.width + spill + spill,
                    slice.source_clip.size.height,
                ),
            );
        } else {
            admission.source_clip = Bounds::new(
                gpui::point(slice.source_clip.left(), own.top() - spill),
                gpui::size(
                    slice.source_clip.size.width,
                    own.size.height + spill + spill,
                ),
            );
        }
        Some(Self {
            bounds: own,
            slice: admission,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{PaintSegment, PaintSegmentId, point, px, size};

    fn rect(y: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(10.), px(y)), size(px(60.), px(h)))
    }

    fn part(y: f32, h: f32) -> PaintSegmentSlice {
        PaintSegmentSlice {
            placement: PaintSegment {
                id: PaintSegmentId(2),
                ordinal: 3,
                source: point(px(10.), px(y - 5.)),
                destination: point(px(110.), px(0.)),
            },
            source_clip: rect(y, h),
        }
    }

    #[test]
    fn clone_wraps_each_content_fragment_while_slice_keeps_the_common_box() {
        let whole = rect(0., 100.);
        for (y, h) in [(5., 40.), (45., 40.), (85., 10.)] {
            let slice = part(y, h);
            let cloned =
                Decoration::for_slice(whole, slice, Some((StackAxis::Horizontal, 5., 5.))).unwrap();
            assert_eq!(cloned.bounds, rect(y - 5., h + 10.));
            assert_eq!(cloned.slice.placement.id, slice.placement.id);
            assert_eq!(cloned.slice.placement.ordinal, 3);
            assert_eq!(cloned.slice.placement.source, slice.placement.source);
            assert_eq!(
                cloned.slice.placement.destination,
                slice.placement.destination
            );
            assert!(cloned.slice.source_clip.top() < cloned.bounds.top());
            assert!(cloned.slice.source_clip.bottom() > cloned.bounds.bottom());
            let sliced = Decoration::for_slice(whole, slice, None).unwrap();
            assert_eq!(sliced.bounds, whole);
            assert_eq!(sliced.slice.source_clip, slice.source_clip);
        }
    }

    #[test]
    fn descendant_clone_intersects_its_content_before_repeating_edges() {
        let own = rect(20., 40.);
        let first =
            Decoration::for_slice(own, part(5., 40.), Some((StackAxis::Horizontal, 5., 5.)))
                .unwrap();
        let last =
            Decoration::for_slice(own, part(45., 40.), Some((StackAxis::Horizontal, 5., 5.)))
                .unwrap();
        assert_eq!(first.bounds, rect(20., 30.));
        assert_eq!(last.bounds, rect(40., 20.));
        assert!(
            Decoration::for_slice(own, part(85., 10.), Some((StackAxis::Horizontal, 5., 5.)))
                .is_none()
        );
    }

    #[test]
    fn vertical_clone_wraps_content_at_both_logical_block_edges() {
        let whole = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(60.)));
        for axis in [StackAxis::VerticalRl, StackAxis::VerticalLr] {
            let slice = PaintSegmentSlice {
                placement: part(0., 1.).placement,
                source_clip: axis_box(axis, whole.origin, 100., 0., 60., 45., 40.),
            };
            let cloned = Decoration::for_slice(whole, slice, Some((axis, 5., 5.))).unwrap();
            assert_eq!(
                cloned.bounds,
                axis_box(axis, whole.origin, 100., 0., 60., 40., 50.)
            );
            assert_eq!(cloned.slice.placement.id, slice.placement.id);
            assert_eq!(cloned.slice.placement.ordinal, slice.placement.ordinal);
            assert_eq!(cloned.slice.placement.source, slice.placement.source);
            assert_eq!(
                cloned.slice.placement.destination,
                slice.placement.destination
            );
            assert_eq!(
                Decoration::for_slice(whole, slice, None).unwrap().bounds,
                whole
            );
        }
    }
}
