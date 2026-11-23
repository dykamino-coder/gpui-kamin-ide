//! Отложенная краска хранит не историю правок, а не более двух областей:
//! по одной на причину. Объединение консервативно и ограничивает память
//! независимо от числа невидимых мутаций до следующей отрисовки.

use super::Wait;
use gpui::{Bounds, ScaledPixels};

type Frame = (Bounds<ScaledPixels>, Wait);

pub(super) fn coalesce(old: Vec<Frame>, new: Vec<Frame>) -> Vec<Frame> {
    let mut areas: [Option<Bounds<ScaledPixels>>; 2] = [None, None];
    for (area, why) in old.into_iter().chain(new) {
        let index = match why {
            Wait::Inline => 0,
            Wait::Decoration => 1,
        };
        areas[index] = Some(areas[index].map_or(area, |kept| kept.union(&area)));
    }
    areas
        .into_iter()
        .zip([Wait::Inline, Wait::Decoration])
        .filter_map(|(area, why)| area.map(|area| (area, why)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_edits_keep_bounded_storage_and_all_ink() {
        let mut frames = Vec::new();
        for i in 0..10_000 {
            let area = Bounds {
                origin: gpui::point(ScaledPixels(i as f32), ScaledPixels(500.)),
                size: gpui::size(ScaledPixels(10.), ScaledPixels(10.)),
            };
            frames = coalesce(frames, vec![(area, Wait::Inline), (area, Wait::Decoration)]);
            assert_eq!(frames.len(), 2);
            assert!(frames.capacity() <= 4);
        }
        assert_eq!(frames[0].0.left(), ScaledPixels(0.));
        assert_eq!(frames[0].0.right(), ScaledPixels(10_009.));
        assert_eq!(frames[0].0, frames[1].0);
    }
}
