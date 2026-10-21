//! Наследование: flow-относительные слова выравнивания `flow-start` /
//! `flow-end` (css-align-3 в расширении для grid lanes, css-grid-3 §alignment).
//!
//! В контейнере grid lanes ось сетки идёт в порядке дорожек, ось стопки — в
//! порядке заполнения: `track-reverse` и `fill-reverse` обращают эти потоки,
//! `flow-start`/`flow-end` следуют за ними, а `start`/`end` остаются
//! относительными к письму. Вне grid lanes поток — поток письма, и слова
//! ведут себя как `start`/`end`. `flex-start`/`flex-end` в grid lanes следуют
//! тем же потокам. Разборщик хранит слова как `start`/`end` и помнит, какое
//! свойство несло flow-слово (`Computed::align_flow`); этот проход обращает
//! значение, когда направление lanes контейнера известно: content/items — по
//! своему направлению, self — по родительскому (контейнеру выравнивания).

use crate::style::computed::{Align, Computed, Display, Justify};

/// `align_flow` bits, one per alignment property.
pub(crate) const JUSTIFY_CONTENT: u8 = 1;
pub(crate) const ALIGN_CONTENT: u8 = 2;
pub(crate) const JUSTIFY_ITEMS: u8 = 4;
pub(crate) const ALIGN_ITEMS: u8 = 8;
pub(crate) const JUSTIFY_SELF: u8 = 16;
pub(crate) const ALIGN_SELF: u8 = 32;

/// Records whether the declaration `key: v` used a flow-relative keyword.
pub(crate) fn note(c: &mut Computed, key: &str, v: &str) {
    let bit = match key {
        "justify-content" => JUSTIFY_CONTENT,
        "align-content" => ALIGN_CONTENT,
        "justify-items" => JUSTIFY_ITEMS,
        "align-items" => ALIGN_ITEMS,
        "justify-self" => JUSTIFY_SELF,
        "align-self" => ALIGN_SELF,
        _ => return,
    };
    // `flex-start`/`flex-end` are flow relative in grid lanes as well
    // (`column-fill-reverse-align-content-002`: `flex-start` aligns to the
    // fill start, i.e. the bottom under `fill-reverse`).
    let flow = matches!(
        v.split_whitespace().last(),
        Some("flow-start" | "flow-end" | "flex-start" | "flex-end")
    );
    if flow {
        c.align_flow |= bit;
    } else {
        c.align_flow &= !bit;
    }
}

/// Whether the inline (`justify`) and block (`align`) axes of grid lanes
/// container `s` run against the writing mode's start-to-end direction.
fn reversed_axes(s: &Computed) -> Option<(bool, bool)> {
    if s.display != Some(Display::GridLanes) && !s.lanes_taffy {
        return None;
    }
    let (grid, stack) = (s.lanes_track_reverse, s.lanes_fill_reverse);
    // Column lanes: the grid axis is the inline axis.
    Some(if crate::dom::lanes_row_dir(s) {
        (stack, grid)
    } else {
        (grid, stack)
    })
}

fn flip_align(a: &mut Option<Align>) {
    *a = match *a {
        Some(Align::Start) => Some(Align::End),
        Some(Align::End) => Some(Align::Start),
        other => other,
    };
}

fn flip_justify(j: &mut Option<Justify>) {
    *j = match *j {
        Some(Justify::WmStart) => Some(Justify::WmEnd),
        Some(Justify::WmEnd) => Some(Justify::WmStart),
        Some(Justify::Start) => Some(Justify::End),
        Some(Justify::End) => Some(Justify::Start),
        other => other,
    };
}

/// Resolves the flow keywords of `c` against its own and its parent's lanes
/// direction.
pub(super) fn resolve(parent: &Computed, c: &mut Computed) {
    if c.align_flow == 0 {
        return;
    }
    let has = |bit: u8| c.align_flow & bit != 0;
    if let Some((inline_rev, block_rev)) = reversed_axes(c) {
        let (jc, ac, ji, ai) = (
            has(JUSTIFY_CONTENT) && inline_rev,
            has(ALIGN_CONTENT) && block_rev,
            has(JUSTIFY_ITEMS) && inline_rev,
            has(ALIGN_ITEMS) && block_rev,
        );
        if jc {
            flip_justify(&mut c.justify_content);
        }
        if ac {
            flip_justify(&mut c.align_content);
        }
        if ji {
            flip_align(&mut c.justify_items);
        }
        if ai {
            flip_align(&mut c.align_items);
        }
    }
    if let Some((inline_rev, block_rev)) = reversed_axes(parent) {
        if has(JUSTIFY_SELF) && inline_rev {
            flip_align(&mut c.justify_self);
        }
        if has(ALIGN_SELF) && block_rev {
            flip_align(&mut c.align_self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lanes(track_reverse: bool, fill_reverse: bool) -> Computed {
        Computed {
            display: Some(Display::GridLanes),
            lanes_track_reverse: track_reverse,
            lanes_fill_reverse: fill_reverse,
            ..Default::default()
        }
    }

    #[test]
    fn track_reverse_flips_inline_flow_keywords_of_column_lanes() {
        let mut c = lanes(true, false);
        note(&mut c, "justify-content", "flow-start");
        note(&mut c, "align-content", "flow-start");
        c.justify_content = Some(Justify::WmStart);
        c.align_content = Some(Justify::WmStart);
        resolve(&Computed::default(), &mut c);
        assert_eq!(c.justify_content, Some(Justify::WmEnd));
        assert_eq!(c.align_content, Some(Justify::WmStart));
    }

    #[test]
    fn self_alignment_follows_the_parent_lanes() {
        let parent = lanes(false, true);
        let mut c = Computed::default();
        note(&mut c, "align-self", "flow-end");
        c.align_self = Some(Align::End);
        resolve(&parent, &mut c);
        assert_eq!(c.align_self, Some(Align::Start));
        // `start` is not flow relative.
        note(&mut c, "align-self", "start");
        c.align_self = Some(Align::Start);
        resolve(&parent, &mut c);
        assert_eq!(c.align_self, Some(Align::Start));
    }
}
