//! KaminIDE patch: the size contributions of the items of a large grid, kept for one layout pass.
//!
//! A parent asks a grid for its size several times in one pass (a column flex container asks the
//! min-content and the max-content height of an item separately, each at the min-content and the
//! max-content width of its own parent). Every query sizes the tracks again and asks every item
//! for its contributions; the item answers from its layout cache, but reaching that answer touches
//! the item's node, style and cache for each of thousands of items per query.
//!
//! A contribution is a pure function of the item, the kind and axis of the contribution, the size
//! of its grid area and (for the minimum contribution) the container's inner size: within one pass
//! no child changes. The memo keeps the last contribution of each kind and axis of each item keyed
//! by exactly those inputs, compared bit for bit. It is only read where the item's own per-query
//! cache is empty, and a hit fills that cache as the measurement would, so a query answers as it
//! would without the memo. Items whose contributions depend on anything else (baseline shims,
//! subgrid margins) never use it, and grids that place or trim items in ways the key does not
//! capture (subgrids, `margin-trim`) do not keep one. The host drops the memo at the end of the
//! pass ([`crate::LayoutGridContainer::take_contribution_memo`]).

use super::types::GridItem;
use crate::geometry::{AbstractAxis, Size};
use crate::tree::NodeId;
use crate::util::sys::Vec;

/// Grids with fewer items lay out their tracks again for less than the memo costs.
pub(crate) const MEMO_MIN_ITEMS: usize = 64;

/// The bits of an absent size: a NaN payload no arithmetic produces, so it never equals a size.
const NONE_BITS: u32 = 0x7FC0_0A5E;
/// The bits of a slot that holds nothing: another such NaN payload.
const EMPTY_BITS: u32 = 0x7FC0_0E70;

/// The bits of an optional size.
#[inline(always)]
fn bits(size: Option<f32>) -> u32 {
    size.map_or(NONE_BITS, f32::to_bits)
}

/// The grid area size of a contribution, bit for bit.
#[inline(always)]
fn area_bits(area: Size<Option<f32>>) -> [u32; 2] {
    [bits(area.width), bits(area.height)]
}

/// The index of an axis.
#[inline(always)]
fn axis_index(axis: AbstractAxis) -> usize {
    match axis {
        AbstractAxis::Inline => 0,
        AbstractAxis::Block => 1,
    }
}

/// A kept min-content or max-content contribution.
#[derive(Debug, Clone, Copy)]
struct ContentSlot {
    /// The grid area it was measured for ([`EMPTY_BITS`] when the slot holds nothing).
    area: [u32; 2],
    /// The contribution.
    value: f32,
}

impl ContentSlot {
    /// A slot that holds nothing.
    const EMPTY: ContentSlot = ContentSlot { area: [EMPTY_BITS; 2], value: 0.0 };
}

/// A kept minimum contribution.
#[derive(Debug, Clone, Copy)]
struct MinimumSlot {
    /// The grid area it was computed for ([`EMPTY_BITS`] when the slot holds nothing).
    area: [u32; 2],
    /// The container's inner size in the axis.
    inner: u32,
    /// The contribution.
    value: f32,
    /// The bits of the min-content contribution the computation measured and left in the item's
    /// cache ([NONE_BITS] when it left none).
    min_content: u32,
}

impl MinimumSlot {
    /// A slot that holds nothing.
    const EMPTY: MinimumSlot = MinimumSlot { area: [EMPTY_BITS; 2], inner: EMPTY_BITS, value: 0.0, min_content: NONE_BITS };
}

/// The kept contributions of one item.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ItemMemo {
    /// Whether the item uses the memo.
    on: bool,
    /// Min-content (`[axis][0]`) and max-content (`[axis][1]`) contributions.
    content: [[ContentSlot; 2]; 2],
    /// Minimum contributions per axis.
    minimum: [MinimumSlot; 2],
    /// Contributions read from the memo in the current query.
    hits: u16,
}

impl Default for ItemMemo {
    fn default() -> Self {
        ItemMemo::OFF
    }
}

impl ItemMemo {
    /// The memo of an item that does not use one.
    pub(crate) const OFF: ItemMemo =
        ItemMemo { on: false, content: [[ContentSlot::EMPTY; 2]; 2], minimum: [MinimumSlot::EMPTY; 2], hits: 0 };

    /// An empty memo for an item that uses one.
    const ON: ItemMemo = ItemMemo { on: true, ..ItemMemo::OFF };
}

/// Whether an item's contributions depend only on the inputs the memo compares.
#[inline(always)]
fn item_is_memoizable(item: &GridItem) -> bool {
    item.contribution_memo.on
        && item.baseline_shim == 0.0
        && item.baseline_shim_end == 0.0
        && item.baseline_shim_x == 0.0
        && item.baseline_shim_x_end == 0.0
        && !item.flattened
        && item.subgrid_root.is_none()
        && item.extra_margin == crate::geometry::Rect::ZERO
}

/// The item's min-content (`max_content == false`) or max-content contribution in `axis` for a
/// grid area of `area`, measured with the area's size in the other axis, as `measure` (the item's
/// cached measurement) gives it.
#[inline(always)]
pub(super) fn content_contribution(
    item: &mut GridItem,
    axis: AbstractAxis,
    max_content: bool,
    area: Size<Option<f32>>,
    measure: impl FnOnce(&mut GridItem) -> f32,
) -> f32 {
    let cache = if max_content { &item.max_content_contribution_cache } else { &item.min_content_contribution_cache };
    if cache.get(axis).is_some() || !item_is_memoizable(item) {
        return measure(item);
    }
    let key = area_bits(area);
    let slot = item.contribution_memo.content[axis_index(axis)][max_content as usize];
    if slot.area == key {
        let cache =
            if max_content { &mut item.max_content_contribution_cache } else { &mut item.min_content_contribution_cache };
        cache.set(axis, Some(slot.value));
        item.contribution_memo.hits = item.contribution_memo.hits.saturating_add(1);
        return slot.value;
    }
    let value = measure(item);
    item.contribution_memo.content[axis_index(axis)][max_content as usize] = ContentSlot { area: key, value };
    value
}

/// The item's minimum contribution in `axis` for a grid area of `area` in a container of inner size
/// `inner`, as `compute` (the item's cached computation) gives it.
#[inline(always)]
pub(super) fn minimum_contribution(
    item: &mut GridItem,
    axis: AbstractAxis,
    area: Size<Option<f32>>,
    inner: Option<f32>,
    compute: impl FnOnce(&mut GridItem) -> f32,
) -> f32 {
    // The computation may read the min-content cache, so it is only kept while that cache is empty.
    if item.minimum_contribution_cache.get(axis).is_some()
        || item.min_content_contribution_cache.get(axis).is_some()
        || !item_is_memoizable(item)
    {
        return compute(item);
    }
    let (key, inner) = (area_bits(area), bits(inner));
    let slot = item.contribution_memo.minimum[axis_index(axis)];
    if slot.area == key && slot.inner == inner {
        item.minimum_contribution_cache.set(axis, Some(slot.value));
        item.contribution_memo.hits = item.contribution_memo.hits.saturating_add(1);
        if slot.min_content != NONE_BITS {
            item.min_content_contribution_cache.set(axis, Some(f32::from_bits(slot.min_content)));
        }
        return slot.value;
    }
    let value = compute(item);
    let min_content = bits(item.min_content_contribution_cache.get(axis));
    item.contribution_memo.minimum[axis_index(axis)] = MinimumSlot { area: key, inner, value, min_content };
    value
}

/// The contributions of the items of one grid, kept between the queries of one pass.
#[derive(Debug, Clone, Default)]
pub struct ContributionMemo {
    /// Per item, in item order, with the item's node.
    items: Vec<(NodeId, ItemMemo)>,
    /// Contributions read from the memo by the last query.
    hits: usize,
}

impl ContributionMemo {
    /// Whether a grid keeps a memo: one large enough, whose items the key describes completely.
    #[inline(always)]
    pub(super) fn applies(item_count: usize, has_subgrids: bool, margin_trim: bool) -> bool {
        item_count >= MEMO_MIN_ITEMS && !has_subgrids && !margin_trim
    }

    /// Give each item its kept contributions (or an empty memo when its node changed or it has none).
    pub(super) fn load(&self, items: &mut [GridItem]) {
        for (index, item) in items.iter_mut().enumerate() {
            item.contribution_memo = match self.items.get(index) {
                Some((node, memo)) if *node == item.node => *memo,
                _ => ItemMemo::ON,
            };
        }
    }

    /// Keep the contributions of the items for the next query of the pass.
    pub(super) fn store(&mut self, items: &[GridItem]) {
        self.items.clear();
        self.items.reserve_exact(items.len());
        self.hits = items.iter().map(|item| item.contribution_memo.hits as usize).sum();
        self.items.extend(items.iter().map(|item| (item.node, ItemMemo { hits: 0, ..item.contribution_memo })));
    }

    /// Contributions read from the memo by the last query.
    pub fn hits(&self) -> usize {
        self.hits
    }

    /// Bytes held by the memo.
    pub fn heap_bytes(&self) -> usize {
        self.items.capacity() * core::mem::size_of::<(NodeId, ItemMemo)>()
    }
}

/// Hand the memo of the grid `node`, updated with the contributions of `items`, back to the host.
pub(super) fn keep(
    tree: &mut impl crate::LayoutGridContainer,
    node: NodeId,
    memo: Option<ContributionMemo>,
    items: &[GridItem],
) {
    if let Some(mut memo) = memo {
        memo.store(items);
        tree.put_contribution_memo(node, memo);
    }
}

#[cfg(all(test, feature = "taffy_tree"))]
#[path = "contribution_memo_tests.rs"]
mod tests;
