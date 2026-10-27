//! Остаток после завершённой фазы спаннера: consumed получен из native layout
//! ряда вместе с полями спаннера, а не из гипотетической формы его поддерева.

use crate::layout::fragment::token::{
    Error, Token,
    fc::{FcState, MulticolState},
};

/// Завершённая фаза спаннера. Курсор детей переживает смену constraints,
/// consumed относится к ряду, а не к content-box разрезанного предка.
pub(crate) struct SpannerRow(Token);

impl SpannerRow {
    pub(crate) fn after_spanner(index: usize, block_end: f32) -> Result<Self, Error> {
        let index = u32::try_from(index).map_err(|_| Error::ResourceExhausted)?;
        let state = FcState::Multicol(MulticolState {
            row: 0,
            spanner: Some(index),
            flow: None,
        });
        let mut token = Token::new(state.clone()).advance(block_end, state)?;
        token.flags.by_spanner = true;
        Ok(Self(token))
    }
}

/// Поля уже схлопнуты native layout; повторное сложение нарушило бы
/// CSS 2 §8.3.1. Вход — завершённая фаза, а не оценка формы поддерева.
pub(crate) fn tail_rest_height(block_size: f32, phase: &SpannerRow) -> f32 {
    (block_size - phase.0.consumed).max(0.)
}

/// `column-height` резервирует всю последнюю строку сетки. Внешний
/// фрагментаинер ограничивает колонки, но auto-высота коробки — по содержимому.
#[derive(Clone, Copy, Default)]
pub(crate) enum RowExtent {
    #[default]
    ColumnHeight,
    Content,
}

impl RowExtent {
    pub(crate) fn block_size(
        self,
        grid: f32,
        lines: &[(f32, f32)],
        spans: &[(usize, f32)],
        plan: &crate::layout::fragment::plan::Plan,
        count: usize,
        kids: &[crate::layout::fragment::types::Kid],
    ) -> f32 {
        match self {
            Self::ColumnHeight => grid,
            Self::Content => plan
                .iter()
                .map(|f| lines.get(f.col / count).map_or(0., |l| l.0) + f.y + f.h)
                .chain(spans.iter().map(|&(i, at)| at + kids[i].h + kids[i].mb))
                .fold(0., f32::max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_spanner_phase_constrains_tail_without_negative_budget() {
        let phase = SpannerRow::after_spanner(3, 70.).unwrap();
        assert_eq!(tail_rest_height(100., &phase), 30.);
        assert_eq!(tail_rest_height(120., &phase), 50.);
        assert_eq!(tail_rest_height(50., &phase), 0.);
        assert!(SpannerRow::after_spanner(3, f32::NAN).is_err());
    }
}
