//! KaminIDE patch: keep local deferred paint ranges in their containing scene.
//!
//! A retained root copies these commands as part of its own paint, without a
//! separate deferred paint pass. Its copied draws must follow the same mapping
//! as scroll nodes, including command removal during a scrolled replay.

use super::*;
use crate::scene::scroll_marks::SceneSpace;

impl Window {
    pub(super) fn local_deferred_edges(
        &self,
        scenes: &Range<usize>,
        space: SceneSpace,
    ) -> impl Iterator<Item = (usize, usize)> + '_ {
        let scenes = scenes.clone();
        self.next_frame
            .deferred_draws
            .iter()
            .filter(move |draw| {
                let range = &draw.paint_range;
                draw.local
                    && draw.paint_space == Some(space)
                    && range.start.scene_index >= scenes.start
                    && range.end.scene_index <= scenes.end
            })
            .flat_map(|draw| {
                [
                    (draw.paint_range.start.scene_index, 0),
                    (draw.paint_range.end.scene_index, 0),
                ]
            })
    }

    pub(super) fn land_local_deferred(
        &mut self,
        scenes: &Range<usize>,
        from: SceneSpace,
        here: SceneSpace,
        land: impl Fn(usize) -> usize,
    ) {
        let relabel = self.next_frame.scene.relabel as isize;
        for draw in &mut self.next_frame.deferred_draws {
            let range = &mut draw.paint_range;
            if !draw.local
                || draw.paint_space != Some(from)
                || range.start.scene_index < scenes.start
                || range.end.scene_index > scenes.end
            {
                continue;
            }
            range.start.scene_index = land(range.start.scene_index);
            range.end.scene_index = land(range.end.scene_index);
            draw.paint_space = Some(here);
            if !draw.groups.is_empty() {
                draw.groups = draw.groups.start.checked_add_signed(relabel).unwrap()
                    ..draw.groups.end.checked_add_signed(relabel).unwrap();
            }
        }
    }
}
