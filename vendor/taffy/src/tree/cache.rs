//! A cache for storing the results of layout computation

#![allow(clippy::unusual_byte_groupings)]

use crate::geometry::Size;
use crate::style::AvailableSpace;
use crate::tree::{CollapsibleMarginSet, LayoutInput, LayoutOutput, RunMode, SizingMode};
use crate::util::sys::{Box, Vec};
use crate::RequestedAxis;

/// The number of cache entries for each node in the tree
const CACHE_SIZE: usize = 9;

// Manually written-out results of float to u32 bit casts because
// `f32::to_bits` is not yet const at our MSRV.

/// `f32::INFINITY` as a u32
const INFINITY_BITS: u32 = 0b_0_11111111_00000000000000000000000_u32;
/// `f32::NEG_INFINITY` as a u32
const NEG_INFINITY_BITS: u32 = 0b_1_11111111_00000000000000000000000_u32;

// The `CacheKey` encodes two f32s as a u64. We know that the f32s will always be
// non-negative, so we pack two extra bits encoding the `RequestedAxis` into the
// sign bits of the f32s. These constants help to encode and decode those bits.

/// The sign bit of the first f32
const SIGN_BIT_1: u64 = 1u64 << 63;
/// The sign bit of the second f32
const SIGN_BIT_2: u64 = 1u64 << 31;
/// Mask of both sign bits (used to compute NON_SIGN_BITS_MASK)
const BOTH_SIGN_BITS_MASK: u64 = SIGN_BIT_1 | SIGN_BIT_2;
/// Mask of excluding the sign bits (used when setting/getting the size excluding the packed bits)
const NON_SIGN_BITS_MASK: u64 = !BOTH_SIGN_BITS_MASK;

/// Mask which includes only the bits which encode the x-axis value that we can use to ignore the
/// y-axis value when comparing a cache key.
const X_AXIS_VALUE_MASK: u64 = (u32::MAX as u64) << 32;

/// Pack `Option<f32>` into `u32`
#[inline(always)]
fn option_cache_key(input: Option<f32>) -> u32 {
    match input {
        Some(value) => value.to_bits(),
        None => INFINITY_BITS,
    }
}

/// Pack `Size<Option<f32>>` into `u64`
#[inline(always)]
fn size_option_cache_key(input: Size<Option<f32>>) -> u64 {
    (option_cache_key(input.width) as u64) << 32 | option_cache_key(input.height) as u64
}

/// Pack `AvailableSpace` into `u32`
#[inline(always)]
fn available_space_cache_key(input: AvailableSpace) -> u32 {
    match input {
        AvailableSpace::Definite(value) => (-value).to_bits(),
        AvailableSpace::MinContent => NEG_INFINITY_BITS,
        AvailableSpace::MaxContent => INFINITY_BITS,
    }
}

/// Pack `Size<AvailableSpace>` into `u64`
#[inline(always)]
#[allow(dead_code)]
fn size_available_space_cache_key(input: Size<AvailableSpace>) -> u64 {
    (available_space_cache_key(input.width) as u64) << 32 | available_space_cache_key(input.height) as u64
}

/// Encodes combination of a `known_dimension` (Option<f32>) and `AvailableSpace` in
/// a single dimension into a cache key in a single dimension.
#[inline(always)]
fn mixed_cache_key(kd: Option<f32>, avs: AvailableSpace) -> u32 {
    kd.map(|kd| kd.to_bits()).unwrap_or_else(|| available_space_cache_key(avs))
}

/// Encodes combination of a `known_dimension` (Option<f32>) and `AvailableSpace` in
/// two dimensions into a cache key in a single dimension.
#[inline(always)]
fn size_mixed_cache_key(kd: Size<Option<f32>>, avs: Size<AvailableSpace>) -> u64 {
    (mixed_cache_key(kd.width, avs.width) as u64) << 32 | mixed_cache_key(kd.height, avs.height) as u64
}

/// Space-optimised cache key that packs bits into as small a size as possible
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CacheKey {
    /// The initial cached size of the node itself
    kd_available_space: u64,
    /// The initial cached size of the parent's node
    parent_size: u64,
    /// Whether each known dimension is definite. Normalized such that an axis
    /// without a known dimension is always `true`.
    known_dimensions_are_definite: Size<bool>,
}

impl CacheKey {
    #[inline(always)]
    #[allow(dead_code)]
    /// Return the parent size with the extra bits that encode the requested axis masked out
    fn parent_size(&self) -> u64 {
        self.parent_size & NON_SIGN_BITS_MASK
    }

    /// Return the parent size with the extra bits that encode the requested axis masked out
    /// And the y-axis value masked out
    fn x_axis_parent_size(&self) -> u64 {
        self.parent_size & (X_AXIS_VALUE_MASK & NON_SIGN_BITS_MASK)
    }

    /// Return the bits that encode the requested axis
    fn requested_axis_bits(&self) -> u64 {
        self.parent_size & BOTH_SIGN_BITS_MASK
    }

    /// Whether a cached entry with this key contains a valid size for the axis requested by `other`.
    /// Sizes computed for a single axis may contain garbage values in the other axis, so an entry
    /// is only usable if it was computed for the same axis (or for both axes).
    fn size_is_valid_for(&self, other: &CacheKey) -> bool {
        let entry_axis = self.requested_axis_bits();
        entry_axis == BOTH_SIGN_BITS_MASK || entry_axis == other.requested_axis_bits()
    }
}

impl From<&LayoutInput> for CacheKey {
    fn from(input: &LayoutInput) -> Self {
        // Pack axis enum into spare bits in the known_dimensions and available_space values
        let extra_bits = match input.axis {
            RequestedAxis::Horizontal => SIGN_BIT_1,
            RequestedAxis::Vertical => SIGN_BIT_2,
            RequestedAxis::Both => SIGN_BIT_1 | SIGN_BIT_2,
        };

        Self {
            kd_available_space: size_mixed_cache_key(input.known_dimensions, input.available_space),
            parent_size: (size_option_cache_key(input.parent_size) & NON_SIGN_BITS_MASK) | extra_bits,
            known_dimensions_are_definite: input
                .known_dimensions_are_definite
                .zip_map(input.known_dimensions, |is_definite, kd| {
                    is_definite || kd.is_none()
                }),
        }
    }
}

/// The layout inputs that affect a node's layout but are not part of the [`CacheKey`]: the
/// [`SizingMode`] and whether each of the node's vertical margins is collapsible, packed into a `u8`.
///
/// These are determined by the layout algorithm of the node's parent and by the node's own style,
/// so they only change for a given node when one of those changes. Rather than keying every entry
/// on them, a [`Cache`] records the single mode that all of its entries were computed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct CacheMode(u8);

impl CacheMode {
    /// Set if the sizing mode is `SizingMode::InherentSize`
    const INHERENT_SIZE_BIT: u8 = 0b001;
    /// Set for a probe that ignores own min/max constraints.
    const INTRINSIC_SIZE_BIT: u8 = 0b1000;
    /// Set if the node's top margin is collapsible
    const COLLAPSIBLE_START_MARGIN_BIT: u8 = 0b010;
    /// Set if the node's bottom margin is collapsible
    const COLLAPSIBLE_END_MARGIN_BIT: u8 = 0b100;

    /// The mode of a cache that has never been stored to. Which mode this is does not matter,
    /// as such a cache has no entries.
    const INITIAL: Self = Self(0);
}

impl From<&LayoutInput> for CacheMode {
    #[inline(always)]
    fn from(input: &LayoutInput) -> Self {
        let inherent_size = matches!(input.sizing_mode, SizingMode::InherentSize) as u8;
        let intrinsic_size = matches!(input.sizing_mode, SizingMode::IntrinsicSize) as u8;
        let collapsible = input.vertical_margins_are_collapsible;
        Self(
            (inherent_size * Self::INHERENT_SIZE_BIT)
                | (intrinsic_size * Self::INTRINSIC_SIZE_BIT)
                | (collapsible.start as u8 * Self::COLLAPSIBLE_START_MARGIN_BIT)
                | (collapsible.end as u8 * Self::COLLAPSIBLE_END_MARGIN_BIT),
        )
    }
}

#[cfg(all(debug_assertions, feature = "std"))]
std::thread_local! {
    /// See [`cache_mode_change_evictions`]
    static MODE_CHANGE_EVICTIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// The number of times that the current thread has stored a result into a non-empty [`Cache`]
/// whose entries were computed with a different `sizing_mode` or `vertical_margins_are_collapsible`,
/// dropping those entries.
///
/// This is expected when the layout algorithm of a node's parent has changed since the node was
/// last laid out. It must not happen while laying out a tree whose caches started out empty: that
/// would mean that Taffy lays a node out with more than one mode in a single pass, with each
/// change of mode throwing away the node's cache. Taffy's test suite uses this function to check
/// that. Only available in debug builds.
#[doc(hidden)]
#[cfg(all(debug_assertions, feature = "std"))]
pub fn cache_mode_change_evictions() -> usize {
    MODE_CHANGE_EVICTIONS.with(|count| count.get())
}

/// Cached intermediate layout results
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub(crate) struct CacheEntry<T> {
    /// The key for the cache entry
    key: CacheKey,
    /// The cached size and baselines of the item
    content: T,
}

/// Size-only results retain the margin information required by their parent.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
struct Measurement {
    /// Border-box size of the measured node.
    size: Size<f32>,
    /// Collapsible margins at the block-start edge.
    top_margin: CollapsibleMarginSet,
    /// Collapsible margins at the block-end edge.
    bottom_margin: CollapsibleMarginSet,
    /// Whether the node collapses through (empty for margin collapsing).
    margins_can_collapse_through: bool,
}

impl Measurement {
    /// Captures the size and margin facts of a layout output.
    fn new(output: &LayoutOutput) -> Self {
        Self {
            size: output.size,
            top_margin: output.top_margin,
            bottom_margin: output.bottom_margin,
            margins_can_collapse_through: output.margins_can_collapse_through,
        }
    }

    /// Rebuilds a size-only layout output from the stored facts.
    fn output(&self) -> LayoutOutput {
        LayoutOutput {
            top_margin: self.top_margin,
            bottom_margin: self.bottom_margin,
            margins_can_collapse_through: self.margins_can_collapse_through,
            ..LayoutOutput::from_outer_size(self.size)
        }
    }
}

/// KaminIDE patch: the size-only measurements of one node.
type MeasureEntries = [Option<CacheEntry<Measurement>>; CACHE_SIZE];

/// Workaround for `Option<CacheEntry<_>>` not being `Copy` (required for array repeat expressions)
const NONE_MEASURE_ENTRY: Option<CacheEntry<Measurement>> = None;

/// KaminIDE patch: a cache without measurement storage yet.
const NO_MEASURE_SLOT: u32 = u32::MAX;

/// KaminIDE patch: the measurement entries of the caches of one tree, in one contiguous store.
///
/// Most nodes of a frame tree are only laid out, never measured, so a cache takes a slot on
/// its first measurement. The slots of a tree live in one vector rather than in a block per
/// node: a tree that is cleared and rebuilt every frame keeps the vector's capacity, so
/// measuring allocates nothing in the steady state and no small blocks are freed per frame.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct MeasureStore {
    /// Measurement entries, one array per slot.
    slots: Vec<MeasureEntries>,
    /// Slots released by removed nodes, reused before the vector grows.
    free: Vec<u32>,
}

impl MeasureStore {
    /// Create an empty store
    pub const fn new() -> Self {
        Self { slots: Vec::new(), free: Vec::new() }
    }

    /// Drop every slot, keeping the capacity for the next tree.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
    }

    /// Return the slot of a cache whose node is removed; the cache must not be used again.
    pub fn release(&mut self, cache: &mut Cache) {
        if cache.measure_slot != NO_MEASURE_SLOT {
            self.free.push(cache.measure_slot);
            cache.measure_slot = NO_MEASURE_SLOT;
            cache.occupied_entries = 0;
        }
    }

    /// The entries of `slot`, empty for a cache that was never measured.
    fn entries(&self, slot: u32) -> &[Option<CacheEntry<Measurement>>] {
        self.slots.get(slot as usize).map_or(&[], |entries| entries.as_slice())
    }

    /// The entries of the cache's slot, taking a slot on its first measurement.
    fn entries_mut(&mut self, slot: &mut u32) -> &mut MeasureEntries {
        if *slot == NO_MEASURE_SLOT {
            *slot = self.free.pop().unwrap_or_else(|| {
                self.slots.push([NONE_MEASURE_ENTRY; CACHE_SIZE]);
                (self.slots.len() - 1) as u32
            });
        }
        &mut self.slots[*slot as usize]
    }
}

/// A cache for caching the results of a sizing a Grid Item or Flexbox Item
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Cache {
    /// The cache entry for the node's final layout
    final_layout_entry: Option<CacheEntry<LayoutOutput>>,
    /// The cache entries for the node's preliminary size measurements.
    ///
    /// KaminIDE patch: a slot of the tree's [`MeasureStore`], taken on the first measurement.
    /// The entries are 576 of the 648 bytes of a cache inline, and most nodes of a frame tree
    /// are only laid out, never measured.
    measure_slot: u32,
    /// KaminIDE patch: which entries of the slot hold a measurement of this cache. Clearing
    /// resets the mask, so a clear neither touches nor needs the store.
    occupied_entries: u16,
    /// KaminIDE patch: the measurements of a cache used without a tree's store (`get`,
    /// `store`), as by trees that own their nodes.
    own: Option<Box<MeasureStore>>,
    /// Tracks which measure entries have been used since the eviction cursor last passed them
    recently_used_entries: u16,
    /// The next measure entry to consider replacing
    next_measure_entry: u8,
    /// Tracks if all cache entries are empty
    is_empty: bool,
    /// The `sizing_mode` and `vertical_margins_are_collapsible` inputs that all of the cache's
    /// entries were computed with. Results are only retrieved for inputs that have the same mode,
    /// and storing a result that was computed with a different mode drops the existing entries.
    mode: CacheMode,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// The most recent full layout output, never an intrinsic size-only result.
    pub(crate) fn performed_layout_output(&self) -> Option<LayoutOutput> {
        self.final_layout_entry
            .as_ref()
            .map(|entry| entry.content.clone())
    }

    /// Create a new empty cache
    pub const fn new() -> Self {
        Self {
            final_layout_entry: None,
            measure_slot: NO_MEASURE_SLOT,
            occupied_entries: 0,
            own: None,
            recently_used_entries: 0,
            next_measure_entry: 0,
            is_empty: true,
            mode: CacheMode::INITIAL,
        }
    }

    /// Try to retrieve a cached result from a cache that keeps its own measurements
    #[inline]
    pub fn get(&mut self, input: &LayoutInput) -> Option<LayoutOutput> {
        match self.own.take() {
            Some(own) => {
                let output = self.get_in(&own, input);
                self.own = Some(own);
                output
            }
            None => self.get_in(&MeasureStore::new(), input),
        }
    }

    /// Store a computed size in a cache that keeps its own measurements
    pub fn store(&mut self, input: &LayoutInput, layout_output: LayoutOutput) {
        let mut own = self.own.take().unwrap_or_default();
        self.store_in(&mut own, input, layout_output);
        self.own = Some(own);
    }

    /// KaminIDE patch: try to retrieve a cached result; `store` holds the tree's measurements.
    #[inline]
    pub fn get_in(&mut self, store: &MeasureStore, input: &LayoutInput) -> Option<LayoutOutput> {
        if self.mode != CacheMode::from(input) {
            return None;
        }
        let key = CacheKey::from(input);
        match input.run_mode {
            RunMode::PerformLayout => self
                .final_layout_entry
                .as_ref()
                .filter(|entry| entry.key == key)
                .map(|e| e.content.clone()),
            RunMode::ComputeSize => {
                for (index, entry) in store.entries(self.measure_slot).iter().enumerate() {
                    let Some(entry) = entry.as_ref().filter(|_| self.occupied_entries & (1 << index) != 0) else {
                        continue;
                    };
                    if entry.key.kd_available_space == key.kd_available_space
                        && entry.key.known_dimensions_are_definite == key.known_dimensions_are_definite
                        && (entry.key.x_axis_parent_size() == key.x_axis_parent_size())
                        && entry.key.size_is_valid_for(&key)
                    {
                        self.recently_used_entries |= 1 << index;
                        return Some(entry.content.output());
                    }
                }

                None
            }
            RunMode::PerformHiddenLayout => None,
        }
    }

    /// KaminIDE patch: store a computed size; `store` holds the tree's measurements.
    pub fn store_in(&mut self, store: &mut MeasureStore, input: &LayoutInput, layout_output: LayoutOutput) {
        if input.run_mode == RunMode::PerformHiddenLayout {
            return;
        }
        let mode = CacheMode::from(input);
        if self.mode != mode {
            #[cfg(all(debug_assertions, feature = "std"))]
            if !self.is_empty {
                MODE_CHANGE_EVICTIONS.with(|count| count.set(count.get() + 1));
            }
            self.clear();
            self.mode = mode;
        }
        let key = CacheKey::from(input);
        match input.run_mode {
            RunMode::PerformLayout => {
                self.is_empty = false;
                self.final_layout_entry = Some(CacheEntry { key, content: layout_output })
            }
            RunMode::ComputeSize => {
                self.is_empty = false;
                let occupied = self.occupied_entries;
                let entries = store.entries_mut(&mut self.measure_slot);
                if let Some(index) = entries.iter().enumerate().position(|(index, entry)| {
                    occupied & (1 << index) != 0 && entry.as_ref().is_some_and(|entry| entry.key == key)
                }) {
                    entries[index].as_mut().unwrap().content = Measurement::new(&layout_output);
                    self.recently_used_entries |= 1 << index;
                    return;
                }
                while self.recently_used_entries & (1 << self.next_measure_entry) != 0 {
                    self.recently_used_entries &= !(1 << self.next_measure_entry);
                    self.next_measure_entry += 1;
                    if self.next_measure_entry == CACHE_SIZE as u8 {
                        self.next_measure_entry = 0;
                    }
                }
                let entry_index = self.next_measure_entry as usize;
                entries[entry_index] = Some(CacheEntry { key, content: Measurement::new(&layout_output) });
                self.occupied_entries |= 1 << entry_index;
                self.recently_used_entries |= 1 << entry_index;
                self.next_measure_entry += 1;
                if self.next_measure_entry == CACHE_SIZE as u8 {
                    self.next_measure_entry = 0;
                }
            }
            RunMode::PerformHiddenLayout => {}
        }
    }

    /// Clear all cache entries and reports clear operation outcome ([`ClearState`])
    pub fn clear(&mut self) -> ClearState {
        if self.is_empty {
            return ClearState::AlreadyEmpty;
        }
        self.is_empty = true;
        self.final_layout_entry = None;
        // KaminIDE patch: a measured node keeps its slot for the next measurements; the mask
        // alone says that its entries are gone.
        self.occupied_entries = 0;
        self.recently_used_entries = 0;
        self.next_measure_entry = 0;
        ClearState::Cleared
    }

    /// Returns true if all cache entries are None, else false
    pub fn is_empty(&self) -> bool {
        self.final_layout_entry.is_none() && self.occupied_entries == 0
    }
}

/// Clear operation outcome. See [`Cache::clear`]
pub enum ClearState {
    /// Cleared some values
    Cleared,
    /// Everything was already cleared
    AlreadyEmpty,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Line;
    use crate::tree::SizingMode;

    /// A cache with the measurement store of its own one-node tree.
    #[derive(Default)]
    pub(super) struct Tested {
        cache: Cache,
        store: MeasureStore,
    }

    impl Tested {
        pub(super) fn get(&mut self, input: &LayoutInput) -> Option<LayoutOutput> {
            self.cache.get_in(&self.store, input)
        }

        pub(super) fn store(&mut self, input: &LayoutInput, output: LayoutOutput) {
            self.cache.store_in(&mut self.store, input, output)
        }

        fn measure_slots(&self) -> Vec<&CacheEntry<Measurement>> {
            let entries = self.store.entries(self.cache.measure_slot).iter().enumerate();
            entries.filter(|(index, _)| self.cache.occupied_entries & (1 << index) != 0).filter_map(|(_, e)| e.as_ref()).collect()
        }
    }

    impl core::ops::Deref for Tested {
        type Target = Cache;
        fn deref(&self) -> &Cache {
            &self.cache
        }
    }

    impl core::ops::DerefMut for Tested {
        fn deref_mut(&mut self) -> &mut Cache {
            &mut self.cache
        }
    }

    fn input(width: f32) -> LayoutInput {
        LayoutInput {
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: Size { width: Some(width), height: None },
            known_dimensions_are_definite: Size { width: true, height: true },
            parent_size: Size::NONE,
            available_space: Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
            vertical_margins_are_collapsible: Line::FALSE,
        }
    }

    fn output(width: f32) -> LayoutOutput {
        LayoutOutput::from_outer_size(Size { width, height: width })
    }

    #[test]
    fn measure_storage_is_allocated_by_the_first_measurement_and_kept_by_clear() {
        let mut cache = Tested::default();
        cache.store(&LayoutInput { run_mode: RunMode::PerformLayout, ..input(1.0) }, output(1.0));
        assert_eq!(cache.measure_slot, NO_MEASURE_SLOT);
        cache.store(&input(2.0), output(2.0));
        assert_eq!(cache.measure_slots().len(), 1);
        let _ = cache.clear();
        assert!(cache.is_empty());
        assert_ne!(cache.measure_slot, NO_MEASURE_SLOT);
        assert_eq!(cache.get(&input(2.0)), None);
        cache.store(&input(3.0), output(3.0));
        assert_eq!(cache.get(&input(3.0)), Some(output(3.0)));
    }

    #[test]
    fn measure_slots_of_one_store_are_reused_after_release_and_clear() {
        let mut store = MeasureStore::new();
        let (mut first, mut second) = (Cache::new(), Cache::new());
        first.store_in(&mut store, &input(1.0), output(1.0));
        second.store_in(&mut store, &input(2.0), output(2.0));
        assert_eq!(store.slots.len(), 2);
        store.release(&mut first);
        let mut third = Cache::new();
        third.store_in(&mut store, &input(3.0), output(3.0));
        assert_eq!(store.slots.len(), 2, "a released slot is taken before the store grows");
        assert_eq!(third.get_in(&store, &input(1.0)), None, "the previous owner's entries are not visible");
        assert_eq!(third.get_in(&store, &input(3.0)), Some(output(3.0)));
        assert_eq!(second.get_in(&store, &input(2.0)), Some(output(2.0)));
        let capacity = store.slots.capacity();
        store.clear();
        assert_eq!(store.slots.capacity(), capacity, "a cleared tree keeps the storage");
    }

    #[test]
    fn recently_used_measure_entries_get_a_second_chance() {
        let mut cache = Tested::default();
        for width in 0..CACHE_SIZE {
            cache.store(&input(width as f32), output(width as f32));
        }
        cache.store(&input(CACHE_SIZE as f32), output(CACHE_SIZE as f32));

        assert_eq!(cache.get(&input(1.0)), Some(output(1.0)));
        cache.store(&input((CACHE_SIZE + 1) as f32), output((CACHE_SIZE + 1) as f32));

        assert_eq!(cache.get(&input(1.0)), Some(output(1.0)));
        assert_eq!(cache.get(&input(2.0)), None);
    }

    #[test]
    fn storing_an_existing_measurement_updates_it_in_place() {
        let mut cache = Tested::default();
        cache.store(&input(1.0), output(1.0));
        cache.store(&input(2.0), output(2.0));
        cache.store(&input(1.0), output(3.0));

        assert_eq!(cache.measure_slots().len(), 2);
        assert_eq!(cache.get(&input(1.0)), Some(output(3.0)));
    }

    #[test]
    fn measurements_preserve_and_update_margin_collapse_metadata() {
        let mut cache = Tested::default();
        let mut through = output(1.0);
        through.margins_can_collapse_through = true;
        cache.store(&input(1.0), through.clone());
        assert_eq!(cache.get(&input(1.0)), Some(through));

        let mut carried = output(2.0);
        carried.top_margin = CollapsibleMarginSet::from_margin(10.0);
        carried.bottom_margin = CollapsibleMarginSet::from_margin(-4.0);
        cache.store(&input(2.0), carried.clone());
        assert_eq!(cache.get(&input(2.0)), Some(carried));
        cache.store(&input(1.0), output(3.0));
        assert_eq!(cache.get(&input(1.0)), Some(output(3.0)));
        assert_eq!(cache.measure_slots().len(), 2);
    }

    #[test]
    fn retrieving_a_measurement_only_marks_its_slot_as_used() {
        let mut cache = Tested::default();
        cache.store(&input(1.0), output(1.0));
        cache.store(&input(2.0), output(2.0));
        cache.recently_used_entries = 0;
        let entries = cache.store.clone();

        assert_eq!(cache.get(&input(1.0)), Some(output(1.0)));
        assert_eq!(cache.store, entries);
        assert_ne!(cache.recently_used_entries, 0);
    }

    fn perform_layout_input(width: f32, height: f32) -> LayoutInput {
        LayoutInput {
            run_mode: RunMode::PerformLayout,
            known_dimensions: Size { width: Some(width), height: Some(height) },
            parent_size: Size { width: Some(width), height: Some(height) },
            ..input(width)
        }
    }

    /// Inputs that differ from `input`/`perform_layout_input` only in their mode
    fn other_modes(input: LayoutInput) -> [LayoutInput; 4] {
        [
            LayoutInput { sizing_mode: SizingMode::ContentSize, ..input },
            LayoutInput { vertical_margins_are_collapsible: Line::TRUE, ..input },
            LayoutInput { vertical_margins_are_collapsible: Line { start: true, end: false }, ..input },
            LayoutInput { vertical_margins_are_collapsible: Line { start: false, end: true }, ..input },
        ]
    }

    #[test]
    fn cache_mode_distinguishes_every_combination_of_inputs() {
        let mut modes = [CacheMode::INITIAL; 8];
        for (index, mode) in modes.iter_mut().enumerate() {
            *mode = CacheMode::from(&LayoutInput {
                sizing_mode: if index & 1 == 0 { SizingMode::ContentSize } else { SizingMode::InherentSize },
                vertical_margins_are_collapsible: Line { start: index & 2 != 0, end: index & 4 != 0 },
                ..input(1.0)
            });
        }
        for (index, mode) in modes.iter().enumerate() {
            assert!(!modes[..index].contains(mode));
        }
    }

    #[test]
    fn retrieving_with_a_different_mode_misses_and_keeps_the_entries() {
        let mut cache = Tested::default();
        cache.store(&perform_layout_input(100.0, 50.0), output(1.0));
        cache.store(&input(1.0), output(2.0));

        for other in other_modes(perform_layout_input(100.0, 50.0)) {
            assert_eq!(cache.get(&other), None);
        }
        for other in other_modes(input(1.0)) {
            assert_eq!(cache.get(&other), None);
        }

        assert_eq!(cache.get(&perform_layout_input(100.0, 50.0)), Some(output(1.0)));
        assert_eq!(cache.get(&input(1.0)), Some(output(2.0)));
    }

    #[test]
    fn storing_with_a_different_mode_drops_the_existing_entries() {
        for other in other_modes(input(3.0)) {
            let mut cache = Tested::default();
            cache.store(&perform_layout_input(100.0, 50.0), output(1.0));
            cache.store(&input(1.0), output(2.0));

            cache.store(&other, output(3.0));
            assert_eq!(cache.get(&other), Some(output(3.0)));
            assert!(cache.final_layout_entry.is_none());
            assert_eq!(cache.measure_slots().len(), 1);

            // The old entries are gone, even for inputs that have the mode that the cache had before
            cache.store(&input(4.0), output(4.0));
            assert_eq!(cache.get(&perform_layout_input(100.0, 50.0)), None);
            assert_eq!(cache.get(&input(1.0)), None);
            assert_eq!(cache.get(&other), None);
            assert_eq!(cache.get(&input(4.0)), Some(output(4.0)));
        }

        for other in other_modes(perform_layout_input(100.0, 50.0)) {
            let mut cache = Tested::default();
            cache.store(&perform_layout_input(100.0, 50.0), output(1.0));
            cache.store(&input(1.0), output(2.0));

            cache.store(&other, output(3.0));
            assert_eq!(cache.get(&other), Some(output(3.0)));
            assert!(cache.measure_slots().is_empty());
            assert_eq!(cache.get(&perform_layout_input(100.0, 50.0)), None);
        }
    }

    #[test]
    fn hidden_layouts_do_not_affect_the_cache() {
        let mut cache = Tested::default();
        cache.store(&input(1.0), output(1.0));
        cache.store(&LayoutInput::HIDDEN, output(2.0));
        assert_eq!(cache.get(&input(1.0)), Some(output(1.0)));
    }

    #[test]
    #[cfg(all(debug_assertions, feature = "std"))]
    fn mode_change_evictions_are_counted_for_non_empty_caches_only() {
        let content_size = LayoutInput { sizing_mode: SizingMode::ContentSize, ..input(1.0) };
        let evictions = cache_mode_change_evictions();

        let mut cache = Tested::default();
        cache.store(&content_size, output(1.0));
        cache.store(&LayoutInput { sizing_mode: SizingMode::ContentSize, ..input(2.0) }, output(2.0));
        assert_eq!(cache_mode_change_evictions(), evictions);

        cache.store(&input(1.0), output(1.0));
        assert_eq!(cache_mode_change_evictions(), evictions + 1);

        cache.clear();
        cache.store(&content_size, output(1.0));
        assert_eq!(cache_mode_change_evictions(), evictions + 1);
    }
}

#[cfg(test)]
mod kamin_cache_regressions {
    use super::tests::Tested;
    use super::*;
    use crate::style_helpers::TaffyMaxContent;

    #[test]
    fn min_content_measure_is_not_layout_at_its_returned_width() {
        let input = LayoutInput {
            run_mode: RunMode::ComputeSize,
            available_space: Size {
                width: AvailableSpace::MinContent,
                height: AvailableSpace::MaxContent,
            },
            ..LayoutInput::HIDDEN
        };
        let mut cache = Tested::default();
        cache.store(
            &input,
            LayoutOutput::from_outer_size(Size {
                width: 60.0,
                height: 110.0,
            }),
        );
        assert!(cache.get(&input).is_some());
        let constrained = LayoutInput {
            known_dimensions: Size {
                width: Some(60.0),
                height: None,
            },
            available_space: Size::MAX_CONTENT,
            ..input
        };
        assert!(cache.get(&constrained).is_none());
    }

    #[test]
    fn definite_and_content_derived_widths_have_distinct_cache_entries() {
        let definite = LayoutInput {
            run_mode: RunMode::ComputeSize,
            known_dimensions: Size {
                width: Some(40.0),
                height: None,
            },
            known_dimensions_are_definite: Size {
                width: true,
                height: true,
            },
            available_space: Size::MAX_CONTENT,
            ..LayoutInput::HIDDEN
        };
        let mut cache = Tested::default();
        cache.store(
            &definite,
            LayoutOutput::from_outer_size(Size {
                width: 40.0,
                height: 30.0,
            }),
        );
        assert!(cache.get(&definite).is_some());
        let content_derived = LayoutInput {
            known_dimensions_are_definite: Size {
                width: false,
                height: true,
            },
            ..definite
        };
        assert!(cache.get(&content_derived).is_none());
    }
}
