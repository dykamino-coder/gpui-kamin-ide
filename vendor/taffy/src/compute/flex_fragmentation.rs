//! KaminIDE patch: CSS Flexbox 1 §10 fragmentation in the writing-mode block axis.
//! Constraints consume space after native flexible sizing; they cannot add flex items or margins.
use super::*;

/// Largest consumed main-axis space among parallel native lines.
pub(super) fn main_growth(tree: &impl LayoutFlexboxContainer, lines: &[FlexLine]) -> f32 {
    lines
        .iter()
        .map(|line| {
            line.items
                .iter()
                .map(|item| tree.flex_item_fragmentation_offset(item.node))
                .sum::<f32>()
        })
        // Подавленный промежуток уменьшает auto-размер; параллельные линии
        // выбирают наибольший подписанный расход, а не минимум в нуле.
        .reduce(f32::max)
        .unwrap_or(0.0)
}

/// Line membership fixed by the fragmenter, in `DetailedFlexInfo` line indices.
/// Those follow physical placement, so under wrap-reverse they decrease along
/// order-modified document order. Only a complete, contiguous assignment that
/// starts at the first line in that order is used; anything else is re-collected.
pub(super) fn pinned_lines(
    tree: &impl LayoutFlexboxContainer,
    items: &[FlexItem],
    constants: &AlgoConstants,
) -> Option<Vec<usize>> {
    if !constants.is_wrap {
        return None;
    }
    let lines: Vec<usize> = items.iter().map(|item| tree.flex_item_fragmentation_line(item.node)).collect::<Option<_>>()?;
    let reverse = constants.is_wrap_reverse;
    let step = |a: usize, b: usize| if reverse { a.checked_sub(b) } else { b.checked_sub(a) };
    let first = if reverse { lines.last() } else { lines.first() };
    (first == Some(&0) && lines.windows(2).all(|w| matches!(step(w[0], w[1]), Some(0 | 1)))).then_some(lines)
}

/// Apply fragmentainer space after flexible sizing and before native line placement.
pub(super) fn apply(
    tree: &impl LayoutFlexboxContainer,
    node: NodeId,
    lines: &mut [FlexLine],
    known: Size<Option<f32>>,
    constants: &mut AlgoConstants,
) {
    let (axis, reversed) = constants.fragmentation_axis;
    let dir = constants.dir;
    if axis == dir.cross_axis() {
        let count = lines.len();
        let mut growth_sum = 0.0;
        for (index, line) in lines.iter_mut().enumerate() {
            let physical = if constants.is_wrap_reverse {
                count - 1 - index
            } else {
                index
            };
            line.fragmented = line
                .items
                .iter()
                .any(|item| tree.flex_item_fragmentation_consumed(item.node) > 0.0);
            if line.fragmented {
                line.cross_size = line
                    .items
                    .iter_mut()
                    .map(|item| {
                        let consumed = tree.flex_item_fragmentation_consumed(item.node);
                        item.offset_cross = 0.0;
                        (item.target_size.cross(dir) - consumed).max(0.0)
                            + if consumed == 0.0 {
                                item.margin.cross_axis_sum(dir)
                            } else if reversed {
                                item.margin.cross_start(dir)
                            } else {
                                item.margin.cross_end(dir)
                            }
                    })
                    .fold(0.0, f32::max);
            }
            let growth = tree.flex_line_fragmentation_growth(node, physical);
            line.cross_size += growth;
            // In a reversed block flow the item stays at the expanded line's high edge.
            if reversed && !line.fragmented {
                line.fragmentation_item_shift = growth;
            }
            growth_sum += growth;
        }
        let native_reversed = constants.is_column && constants.layout_direction.is_rtl();
        if known.cross(dir).is_some() {
            constants.fragmentation_cross_alignment = growth_sum;
        }
        if known.cross(dir).is_some() && reversed != native_reversed {
            let shift = if reversed { -growth_sum } else { growth_sum };
            for line in lines {
                line.fragmentation_shift += shift;
            }
        }
    } else if known.main(dir).is_none() {
        let growth = main_growth(tree, lines);
        constants.fragmentation_main_growth = growth;
        constants
            .container_size
            .set_main(dir, constants.container_size.main(dir) + growth);
        constants
            .inner_container_size
            .set_main(dir, constants.inner_container_size.main(dir) + growth);
    }
}

/// Physical placement order can disagree with logical block progression (reverse and RTL).
/// The native cursor remains untouched so flexible sizing and alignment see no fake margins.
pub(super) struct MainCursor {
    /// Whether fragmentation follows this native main axis.
    enabled: bool,
    /// Whether block progression starts at the physical high edge.
    reversed: bool,
    /// Whether native inline progression starts at the physical high edge.
    native_reversed: bool,
    /// Constraints still ahead in the native iteration order.
    remaining: f32,
    /// Constraints already traversed in the native iteration order.
    consumed: f32,
    /// Extra intrinsic main size reserved for these constraints.
    auto_growth: f32,
}
impl MainCursor {
    /// Preserve the native cursor while tracking logical fragmentainer progression.
    pub(super) fn new(
        tree: &impl LayoutFlexboxContainer,
        line: &FlexLine,
        constants: &AlgoConstants,
    ) -> Self {
        Self {
            enabled: constants.fragmentation_axis.0 == constants.dir.main_axis(),
            reversed: constants.fragmentation_axis.1,
            native_reversed: constants.dir.is_row() && constants.layout_direction.is_rtl(),
            remaining: line
                .items
                .iter()
                .map(|i| tree.flex_item_fragmentation_offset(i.node))
                .sum(),
            consumed: 0.0,
            auto_growth: constants.fragmentation_main_growth,
        }
    }
    /// Translate this item without feeding fragmentation space back into flex sizing.
    pub(super) fn next(&mut self, offset: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.consumed += offset;
        let shift = match (self.reversed, self.native_reversed) {
            (false, false) => self.consumed,
            (true, true) => -self.consumed,
            (true, false) => self.auto_growth - self.remaining,
            (false, true) => self.remaining - self.auto_growth,
        };
        self.remaining -= offset;
        shift
    }
}
