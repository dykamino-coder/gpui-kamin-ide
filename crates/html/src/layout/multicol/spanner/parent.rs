//! Продолжение предка на пути к спаннеру. Высота содержимого расходуется
//! отдельно от кромок и полей; спаннер не расходует block-size своего предка.

use crate::layout::fragment::token::{
    Token,
    fc::{BlockState, FcState},
};

/// Численная геометрия native-пробы; LayoutId остаются в её layout tree.
#[derive(Clone, Copy)]
pub(crate) struct SpannerMetrics {
    pub(crate) block_size: f32,
    pub(crate) content_width: f32,
    pub(crate) content_size: Option<f32>,
    pub(crate) start_edge: f32,
    pub(crate) end_edge: f32,
}

pub(super) struct ParentResume {
    size: Option<f32>,
    token: Token,
    first_edge: f32,
    cloned_edges: Option<(f32, f32)>,
}

impl ParentResume {
    pub(super) fn new(metrics: SpannerMetrics, clone: bool) -> Self {
        let state = BlockState {
            content_block_size: metrics.content_size,
            ..Default::default()
        };
        Self {
            size: metrics.content_size,
            token: Token::new(FcState::Block(state)),
            first_edge: metrics.start_edge,
            cloned_edges: clone.then_some((metrics.start_edge, metrics.end_edge)),
        }
    }

    /// `block_size` — border box реально уложенного фрагмента, без поля.
    /// Только последний фрагмент забирает ещё не израсходованный размер.
    pub(super) fn resume(&mut self, block_size: f32, first: bool, last: bool) -> Option<f32> {
        let remaining = self.size.map(|size| (size - self.token.consumed).max(0.));
        if last {
            return remaining;
        }
        let edges = self
            .cloned_edges
            .map_or(if first { self.first_edge } else { 0. }, |(start, end)| {
                start + end
            });
        let content = (block_size - edges).max(0.);
        self.token = self
            .token
            .advance(
                remaining.map_or(content, |r| content.min(r)),
                self.token.fc.clone(),
            )
            .expect("finite native spanner fragment geometry");
        self.token.flags.by_spanner = true;
        remaining.filter(|r| content > *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_consumes_content_once_across_repeated_spanners() {
        let mut state = ParentResume::new(
            SpannerMetrics {
                block_size: 270.,
                content_width: 200.,
                content_size: Some(250.),
                start_edge: 20.,
                end_edge: 10.,
            },
            false,
        );
        assert_eq!(state.resume(120., true, false), None);
        assert_eq!(state.resume(100., false, false), None);
        assert_eq!(state.resume(0., false, true), Some(50.));
    }

    #[test]
    fn cloned_edges_do_not_consume_parent_content() {
        let mut state = ParentResume::new(
            SpannerMetrics {
                block_size: 280.,
                content_width: 200.,
                content_size: Some(250.),
                start_edge: 20.,
                end_edge: 10.,
            },
            true,
        );
        assert_eq!(state.resume(130., true, false), None);
        assert_eq!(state.resume(130., false, false), None);
        assert_eq!(state.resume(0., false, true), Some(50.));
    }
}
