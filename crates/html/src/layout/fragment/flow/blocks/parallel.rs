//! Проекция ограничений flex-item на его место в последовательной колонке.
//! Переполнение за этим местом — параллельный поток (css-break-3 §2.1),
//! его собственную цель переноса сохраняет `push::flex_flow`.
use super::BlockBreaks;
use crate::layout::fragment::flow::FlowGeometry;
use crate::tolerance::COORD_EPS;

impl BlockBreaks {
    pub(super) fn append_within(&mut self, inner: Self, top: f32, bottom: f32) {
        // Pending разрешаются внутри своего потока: ограничения соседа,
        // перекрытого переполнением, не должны менять выбор раннего разрыва.
        let inner = inner.finish(FlowGeometry::default(), bottom);
        self.cuts.extend(
            inner
                .cuts
                .into_iter()
                .filter(|&(need, _)| need >= top - COORD_EPS)
                .map(|(need, resume)| (need, resume.clamp(top, bottom))),
        );
        self.forced.extend(
            inner
                .forced
                .into_iter()
                .filter(|&at| at >= top - COORD_EPS && at < bottom - COORD_EPS),
        );
        let clip = |(a, b): (f32, f32)| {
            let (a, b) = (a.max(top), b.min(bottom));
            (b > a + COORD_EPS).then_some((a, b))
        };
        self.solid.extend(inner.solid.into_iter().filter_map(clip));
        self.keep.extend(inner.keep.into_iter().filter_map(clip));
        self.avoid.extend(inner.avoid.into_iter().filter_map(clip));
    }
}
