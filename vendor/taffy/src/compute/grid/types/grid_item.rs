//! Contains GridItem used to represent a single grid item during layout
use super::GridTrack;
use crate::compute::common::sizing_keyword::{SizingKeywordResolution, resolve_sizing_keyword};
use crate::compute::grid::OriginZeroLine;
use crate::geometry::AbstractAxis;
use crate::geometry::{Line, Point, Rect, Size};
use crate::style::{
    AlignItems, AlignSelf, AvailableSpace, Dimension, LengthPercentageAuto, Overflow,
};
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt, NodeId, SizingMode};
use crate::util::{MaybeMath, MaybeResolve, ResolveOrZero};
use crate::{AlignItemsKeyword, BoxSizing, GridItemStyle, LengthPercentage};
use core::ops::Range;

/// Represents a single grid item
#[derive(Debug, Clone)]
pub(in super::super) struct GridItem {
    /// The id of the node that this item represents
    pub node: NodeId,

    /// The order of the item in the children array
    ///
    /// We sort the list of grid items during track sizing. This field allows us to sort back the original order
    /// for final positioning
    pub source_order: u16,

    /// The item's definite row-start and row-end, as resolved by the placement algorithm
    /// (in origin-zero coordinates)
    pub row: Line<OriginZeroLine>,
    /// The items definite column-start and column-end, as resolved by the placement algorithm
    /// (in origin-zero coordinates)
    pub column: Line<OriginZeroLine>,

    /// Is it a compressible replaced element?
    /// https://drafts.csswg.org/css-sizing-3/#min-content-zero
    pub is_compressible_replaced: bool,
    /// The item's overflow style
    pub overflow: Point<Overflow>,
    /// The item's box_sizing style
    pub box_sizing: BoxSizing,
    /// The item's size style
    pub size: Size<Dimension>,
    /// The item's min_size style
    pub min_size: Size<LengthPercentageAuto>,
    /// The item's max_size style
    pub max_size: Size<LengthPercentageAuto>,
    /// The item's aspect_ratio style
    pub aspect_ratio: Option<f32>,
    /// The item's padding style
    pub padding: Rect<LengthPercentage>,
    /// The item's border style
    pub border: Rect<LengthPercentage>,
    /// The item's margin style
    pub margin: Rect<LengthPercentageAuto>,
    /// The item's align_self property, or the parent's align_items property is not set
    pub align_self: AlignSelf,
    /// The item's justify_self property, or the parent's justify_items property is not set
    pub justify_self: AlignSelf,
    /// The items first baseline (horizontal)
    pub baseline: Option<f32>,
    /// Shim for baseline alignment that acts like an extra top margin
    pub baseline_shim: f32,
    /// KaminIDE patch: прокладка `last baseline` — лишнее НИЖНЕЕ поле: группа
    /// последних базовых ряда прижимается к его концу (css-align-3 §9.3;
    /// Blink grid_layout_algorithm.cc `CalculateBaselineShim` с
    /// `IsLastBaselineSpecified` и `BaselineGroup::kMinor`).
    pub baseline_shim_end: f32,
    /// KaminIDE patch: ПОСЛЕДНЯЯ базовая элемента из итоговой раскладки (от
    /// верха рамки) — для последней базовой контейнера (css-grid-2 §10.8).
    /// First baseline from final child layout, excluding margins and shims.
    pub first_baseline: Option<f32>,
    pub last_baseline: Option<f32>,
    /// KaminIDE patch: биты выравнивания по базовой по оси x (стиль
    /// `baseline_x_flags`: 1 — группа у правого края, 2 — центральный синтез,
    /// 4 — своя базовая по x).
    pub baseline_x_flags: u8,
    /// KaminIDE patch: расстояние базовой по x до края группы (с полем) —
    /// мера группы колонки (`resolve_item_baselines_x`).
    pub baseline_x: Option<f32>,
    /// KaminIDE patch: прокладки по x — лишнее левое (группа у левого края)
    /// или правое (у правого) поле: `justify-self: baseline` в горизонтальной
    /// сетке и `align-self: baseline` в вертикальной (оси переставлены
    /// движком), css-align-3 §9.3.
    pub baseline_shim_x: f32,
    pub baseline_shim_x_end: f32,

    /// The item's definite row-start and row-end (same as `row` field, except in a different coordinate system)
    /// (as indexes into the Vec<GridTrack> stored in a grid's AbstractAxisTracks)
    pub row_indexes: Line<u16>,
    /// The items definite column-start and column-end (same as `column` field, except in a different coordinate system)
    /// (as indexes into the Vec<GridTrack> stored in a grid's AbstractAxisTracks)
    pub column_indexes: Line<u16>,

    /// Whether the item crosses a flexible row
    pub crosses_flexible_row: bool,
    /// Whether the item crosses a flexible column
    pub crosses_flexible_column: bool,
    /// Whether the item crosses a intrinsic row
    pub crosses_intrinsic_row: bool,
    /// Whether the item crosses a intrinsic column
    pub crosses_intrinsic_column: bool,

    // Caches for intrinsic size computation. These caches are only valid for a single run of the track-sizing algorithm.
    /// Cache for the known_dimensions input to intrinsic sizing computation
    pub grid_area_size_cache: Option<Size<Option<f32>>>,
    /// Cache for the min-content size
    pub min_content_contribution_cache: Size<Option<f32>>,
    /// Cache for the minimum contribution
    pub minimum_contribution_cache: Size<Option<f32>>,
    /// Cache for the max-content size
    pub max_content_contribution_cache: Size<Option<f32>>,

    /// Final y position. Used to compute baseline alignment for the container.
    pub y_position: f32,
    /// Final height. Used to compute baseline alignment for the container.
    pub height: f32,
    /// KaminIDE patch: обрезанные `margin-trim` стороны элемента (биты
    /// физических сторон контейнера) — для финального выравнивания.
    pub margin_trim: u8,
    /// KaminIDE patch: оси, в которых элемент ВКЛАДЫВАЕТСЯ в размер дорожек
    /// этого контейнера (биты `SUBGRID_COLUMNS`/`SUBGRID_ROWS`). Подсетка в
    /// своей подсеточной оси «acts as if it was completely empty» (css-grid-2
    /// §9 (g) `#subgrid-size-contribution`) — бит снят; элемент подсетки,
    /// сплющенный в этот контейнер, вкладывается только в подсеточные оси.
    pub sizing_axes: u8,
    /// KaminIDE patch: элемент ПОДСЕТКИ, сплющенный в дорожки этого
    /// контейнера (§9 (h) `#subgrid-item-contribution`). Он только вкладывает
    /// размеры; раскладывает и ставит его сама подсетка.
    pub flattened: bool,
    /// KaminIDE patch: накопленные края подсеток над элементом — «an extra
    /// layer of (potentially negative) margin» (§subgrid-margins,
    /// §subgrid-gaps), в точках, по физическим сторонам.
    pub extra_margin: Rect<f32>,
    /// KaminIDE patch: размер области сплющенного элемента в НЕподсеточной
    /// оси его подсетки — её собственная дорожка, когда она известна без
    /// содержимого (Blink меряет такой элемент по дорожкам «standalone»-оси
    /// подсетки, `grid_layout_algorithm.cc` `IsSubgridWithStandaloneAxis`).
    /// Перекрывает оценку по дорожкам контейнера в `grid_area_size`.
    pub subgrid_cross: Size<Option<f32>>,
    /// KaminIDE patch: НЕподсеточная ось подсетки — одни неявные `auto`-
    /// дорожки (шаблона нет), и своя дорожка элемента не уже его min-content
    /// (css-grid-2 §12.5: база `auto`-дорожки — min-content вклад, §12.6
    /// растяжка добирает до области). Сплющенный элемент меряется поперёк по
    /// `max(область − края, min-content)`, а не по одной области: иначе текст
    /// подсетки переносился уже своей колонки (`subgrid/auto-track-sizing-001`:
    /// колонка 100px, коробка содержимого 58px, слово «separated» шире).
    pub subgrid_cross_auto: Size<bool>,
    /// KaminIDE patch: подсетка-ребёнок контейнера, через которую элемент
    /// сплющен (у собственного элемента — `None`). Лункам: элементы
    /// АВТО-размещённой подсетки вкладываются во все дорожки (css-grid-3
    /// Overview.bs:686-694).
    pub subgrid_root: Option<NodeId>,
}

impl GridItem {
    /// Create a new item given a concrete placement in both axes
    pub fn new_with_placement_style_and_order<S: GridItemStyle>(
        node: NodeId,
        col_span: Line<OriginZeroLine>,
        row_span: Line<OriginZeroLine>,
        style: S,
        parent_align_items: AlignItems,
        parent_justify_items: AlignItems,
        source_order: u16,
    ) -> Self {
        GridItem {
            node,
            source_order,
            row: row_span,
            column: col_span,
            is_compressible_replaced: style.is_compressible_replaced(),
            overflow: style.overflow(),
            box_sizing: style.box_sizing(),
            size: style.size(),
            min_size: style.min_size(),
            max_size: style.max_size(),
            aspect_ratio: style.aspect_ratio(),
            padding: style.padding(),
            border: style.border(),
            margin: style.margin(),
            align_self: style.align_self().unwrap_or(parent_align_items),
            // Parallel items do not inherit the parent's first x-baseline group.
            // An explicit self value always wins; preserve its safety modifier.
            justify_self: match style.justify_self() {
                Some(own) => own,
                None if parent_justify_items.keyword == AlignItemsKeyword::Baseline
                    && style.baseline_x_flags() & 8 != 0 =>
                {
                    AlignItems::STRETCH
                }
                None => parent_justify_items,
            },
            baseline: None,
            baseline_shim: 0.0,
            baseline_shim_end: 0.0,
            first_baseline: None,
            last_baseline: None,
            baseline_x_flags: style.baseline_x_flags(),
            baseline_x: None,
            baseline_shim_x: 0.0,
            baseline_shim_x_end: 0.0,
            row_indexes: Line { start: 0, end: 0 }, // Properly initialised later
            column_indexes: Line { start: 0, end: 0 }, // Properly initialised later
            crosses_flexible_row: false,            // Properly initialised later
            crosses_flexible_column: false,         // Properly initialised later
            crosses_intrinsic_row: false,           // Properly initialised later
            crosses_intrinsic_column: false,        // Properly initialised later
            grid_area_size_cache: None,
            min_content_contribution_cache: Size::NONE,
            max_content_contribution_cache: Size::NONE,
            minimum_contribution_cache: Size::NONE,
            y_position: 0.0,
            height: 0.0,
            margin_trim: 0,
            sizing_axes: 3,
            flattened: false,
            extra_margin: Rect::ZERO,
            subgrid_cross: Size::NONE,
            subgrid_cross_auto: Size {
                width: false,
                height: false,
            },
            subgrid_root: None,
        }
    }

    /// KaminIDE patch: вкладывается ли элемент в размер дорожек оси `axis`
    /// (см. [`GridItem::sizing_axes`]). Оси сетки taffy физические: `Inline`
    /// — колонки, `Block` — ряды.
    #[inline(always)]
    pub fn sizes_axis(&self, axis: AbstractAxis) -> bool {
        let bit = match axis {
            AbstractAxis::Inline => 1,
            AbstractAxis::Block => 2,
        };
        self.sizing_axes & bit != 0
    }

    /// Whether the item has an auto margin in the block axis
    #[inline(always)]
    pub fn has_auto_block_margin(&self) -> bool {
        self.margin.top.is_auto() || self.margin.bottom.is_auto()
    }

    /// Whether the item's block size depends on the size of its row(s), creating a cyclic
    /// dependency with baseline alignment (which affects row sizing). Per
    /// <https://www.w3.org/TR/css-grid-1/#row-align> such items do not participate in baseline
    /// alignment and are aligned using their fallback alignment instead.
    #[inline(always)]
    pub fn has_cyclic_block_size_dependency(&self) -> bool {
        self.size.height.0.uses_percentage()
            && (self.crosses_intrinsic_row || self.crosses_flexible_row)
    }

    /// Returns true for first/last baseline participation; callers must keep their groups separate.
    /// Auto block margins and cyclic block sizes exclude either group.
    /// See <https://www.w3.org/TR/css-align-3/#baseline-align-self>
    #[inline(always)]
    pub fn participates_in_baseline_alignment(&self) -> bool {
        matches!(
            self.align_self.keyword,
            AlignItemsKeyword::Baseline | AlignItemsKeyword::LastBaseline
        ) && !self.has_auto_block_margin()
            && !self.has_cyclic_block_size_dependency()
    }

    /// Whether an inline-axis auto margin suppresses x-baseline participation.
    #[inline(always)]
    pub fn has_auto_inline_margin(&self) -> bool {
        self.margin.left.is_auto() || self.margin.right.is_auto()
    }

    /// Physical x counterpart of the native block-size cycle guard. Width
    /// percentages that depend on intrinsic/flexible columns would create a
    /// feedback loop through x-baseline shims and must use fallback alignment.
    #[inline(always)]
    pub fn has_cyclic_inline_size_dependency(&self) -> bool {
        self.size.width.0.uses_percentage()
            && (self.crosses_intrinsic_column || self.crosses_flexible_column)
    }

    /// Eligibility for first/last x-baseline groups. Group construction still
    /// separates the two keywords and retains physical start/end distances.
    #[inline(always)]
    pub fn participates_in_baseline_alignment_x(&self) -> bool {
        matches!(
            self.justify_self.keyword,
            AlignItemsKeyword::Baseline | AlignItemsKeyword::LastBaseline
        ) && !self.has_auto_inline_margin()
            && !self.has_cyclic_inline_size_dependency()
    }

    /// This item's placement in the specified axis in OriginZero coordinates
    pub fn placement(&self, axis: AbstractAxis) -> Line<OriginZeroLine> {
        match axis {
            AbstractAxis::Block => self.row,
            AbstractAxis::Inline => self.column,
        }
    }

    /// This item's placement in the specified axis as GridTrackVec indices
    pub fn placement_indexes(&self, axis: AbstractAxis) -> Line<u16> {
        match axis {
            AbstractAxis::Block => self.row_indexes,
            AbstractAxis::Inline => self.column_indexes,
        }
    }

    /// Returns a range which can be used as an index into the GridTrackVec in the specified axis
    /// which will produce a sub-slice of covering all the tracks and lines that this item spans
    /// excluding the lines that bound it.
    pub fn track_range_excluding_lines(&self, axis: AbstractAxis) -> Range<usize> {
        let indexes = self.placement_indexes(axis);
        (indexes.start as usize + 1)..(indexes.end as usize)
    }

    /// Returns the number of tracks that this item spans in the specified axis
    pub fn span(&self, axis: AbstractAxis) -> u16 {
        match axis {
            AbstractAxis::Block => self.row.span(),
            AbstractAxis::Inline => self.column.span(),
        }
    }

    /// Returns the pre-computed value indicating whether the grid item crosses a flexible track in
    /// the specified axis
    pub fn crosses_flexible_track(&self, axis: AbstractAxis) -> bool {
        match axis {
            AbstractAxis::Inline => self.crosses_flexible_column,
            AbstractAxis::Block => self.crosses_flexible_row,
        }
    }

    /// Returns the pre-computed value indicating whether the grid item crosses an intrinsic track in
    /// the specified axis
    pub fn crosses_intrinsic_track(&self, axis: AbstractAxis) -> bool {
        match axis {
            AbstractAxis::Inline => self.crosses_intrinsic_column,
            AbstractAxis::Block => self.crosses_intrinsic_row,
        }
    }

    /// For an item spanning multiple tracks, the upper limit used to calculate its limited min-/max-content contribution is the
    /// sum of the fixed max track sizing functions of any tracks it spans, and is applied if it only spans such tracks.
    pub fn spanned_track_limit(
        &mut self,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        axis_parent_size: Option<f32>,
        resolve_calc_value: &dyn Fn(*const (), f32) -> f32,
    ) -> Option<f32> {
        let spanned_tracks = &axis_tracks[self.track_range_excluding_lines(axis)];
        let tracks_all_fixed = spanned_tracks.iter().all(|track| {
            track
                .max_track_sizing_function
                .definite_limit(axis_parent_size, resolve_calc_value)
                .is_some()
        });
        if tracks_all_fixed {
            let limit: f32 = spanned_tracks
                .iter()
                .map(|track| {
                    track
                        .max_track_sizing_function
                        .definite_limit(axis_parent_size, resolve_calc_value)
                        .unwrap()
                })
                .sum();
            Some(limit)
        } else {
            None
        }
    }

    /// Similar to the spanned_track_limit, but excludes FitContent arguments from the limit.
    /// Used to clamp the automatic minimum contributions of an item
    pub fn spanned_fixed_track_limit(
        &mut self,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        axis_parent_size: Option<f32>,
        resolve_calc_value: &dyn Fn(*const (), f32) -> f32,
    ) -> Option<f32> {
        let spanned_tracks = &axis_tracks[self.track_range_excluding_lines(axis)];
        let tracks_all_fixed = spanned_tracks.iter().all(|track| {
            track
                .max_track_sizing_function
                .definite_value(axis_parent_size, resolve_calc_value)
                .is_some()
        });
        if tracks_all_fixed {
            let limit: f32 = spanned_tracks
                .iter()
                .map(|track| {
                    track
                        .max_track_sizing_function
                        .definite_value(axis_parent_size, resolve_calc_value)
                        .unwrap()
                })
                .sum();
            Some(limit)
        } else {
            None
        }
    }

    /// Compute the known_dimensions to be passed to the child sizing functions
    /// The key thing that is being done here is applying stretch alignment, which is necessary to
    /// allow percentage sizes further down the tree to resolve properly in some cases
    pub(in super::super) fn known_dimensions(
        &self,
        tree: &mut impl LayoutPartialTree,
        grid_area_size: Size<Option<f32>>,
    ) -> Size<Option<f32>> {
        let margins = self.margins_axis_sums_with_baseline_shims(grid_area_size.width, tree);

        let aspect_ratio = self.aspect_ratio;
        // CSS resolves percentage padding and border against the inline size of the containing
        // block. For a grid item under intrinsic measurement, that inline-size basis is the grid
        // area's width when it is definite.
        // Spec:
        // https://www.w3.org/TR/css-grid-1/#item-margins
        // https://www.w3.org/TR/CSS22/box.html#padding-properties
        let padding = self
            .padding
            .resolve_or_zero(grid_area_size.width, |val, basis| tree.calc(val, basis));
        let border = self
            .border
            .resolve_or_zero(grid_area_size.width, |val, basis| tree.calc(val, basis));
        let padding_border_size = (padding + border).sum_axes();
        let box_sizing_adjustment = if self.box_sizing == BoxSizing::ContentBox {
            padding_border_size
        } else {
            Size::ZERO
        };
        let inherent_size = self
            .size
            .maybe_resolve(grid_area_size, |val, basis| tree.calc(val, basis))
            .maybe_apply_aspect_ratio(aspect_ratio)
            .maybe_add(box_sizing_adjustment);
        let min_size = self
            .min_size
            .maybe_resolve(grid_area_size, |val, basis| tree.calc(val, basis))
            .maybe_apply_aspect_ratio(aspect_ratio)
            .maybe_add(box_sizing_adjustment);
        let max_size = self
            .max_size
            .maybe_resolve(grid_area_size, |val, basis| tree.calc(val, basis))
            .maybe_apply_aspect_ratio(aspect_ratio)
            .maybe_add(box_sizing_adjustment);

        let grid_area_minus_item_margins_size = grid_area_size.maybe_sub(margins);

        // If node is absolutely positioned and width is not set explicitly, then deduce it
        // from left, right and container_content_box if both are set.
        let width = inherent_size.width.or_else(|| {
            // A width that is a sizing keyword is not auto, so it does not stretch. The stretch
            // keyword resolves to an exact width; the others resolve during content measurement.
            if self.size.width.is_sizing_keyword() {
                return match resolve_sizing_keyword(
                    self.size.width,
                    grid_area_minus_item_margins_size.width,
                    grid_area_size.width,
                ) {
                    Some(SizingKeywordResolution::Exact(width)) => Some(width),
                    _ => None,
                };
            }

            // Apply width based on stretch alignment if:
            //  - Alignment style is "stretch"
            //  - The node is not absolutely positioned
            //  - The node does not have auto margins in this axis.
            if !self.margin.left.is_auto()
                && !self.margin.right.is_auto()
                && self.justify_self.keyword == AlignItemsKeyword::Stretch
            {
                // A flattened item's standalone auto track cannot be narrower
                // than min-content. The containing block is its grid area, not
                // the outer grid container (the native 0.14 percentage basis).
                if self.subgrid_cross_auto.width {
                    let floor = tree.measure_child_size(
                        self.node,
                        Size::NONE,
                        grid_area_size,
                        Size {
                            width: AvailableSpace::MinContent,
                            height: AvailableSpace::MinContent,
                        },
                        SizingMode::InherentSize,
                        crate::geometry::AbsoluteAxis::Horizontal,
                        Line::FALSE,
                    );
                    return grid_area_minus_item_margins_size
                        .width
                        .map(|width| width.max(floor));
                }
                return grid_area_minus_item_margins_size.width;
            }

            None
        });
        // Reapply aspect ratio after stretch and absolute position width adjustments
        let Size { width, height } = Size {
            width,
            height: inherent_size.height,
        }
        .maybe_apply_aspect_ratio(aspect_ratio);

        let height = height.or_else(|| {
            // A height that is a sizing keyword is not auto, so it does not stretch. The stretch
            // keyword resolves to an exact height; the others resolve during content measurement.
            if self.size.height.is_sizing_keyword() {
                return match resolve_sizing_keyword(
                    self.size.height,
                    grid_area_minus_item_margins_size.height,
                    grid_area_size.height,
                ) {
                    Some(SizingKeywordResolution::Exact(height)) => Some(height),
                    _ => None,
                };
            }

            // Apply height based on stretch alignment if:
            //  - Alignment style is "stretch"
            //  - The node is not absolutely positioned
            //  - The node does not have auto margins in this axis.
            if !self.margin.top.is_auto()
                && !self.margin.bottom.is_auto()
                && self.align_self.keyword == AlignItemsKeyword::Stretch
            {
                return grid_area_minus_item_margins_size.height;
            }

            None
        });
        // Reapply aspect ratio after stretch and absolute position height adjustments
        let Size { width, height } = Size { width, height }.maybe_apply_aspect_ratio(aspect_ratio);

        // Clamp size by min and max width/height
        let Size { width, height } = Size { width, height }.maybe_clamp(min_size, max_size);

        Size { width, height }
    }

    /// Returns the grid area's size in the specified axis when every spanned track has a definite fixed size.
    ///
    /// During intrinsic sizing, percentages on grid items resolve against the size of the grid area,
    /// not the grid container. If the spanned tracks in an axis are not all definite yet, the grid
    /// area is still indefinite in that axis and percentage-dependent values must stay unresolved here.
    ///
    /// Spec:
    /// https://www.w3.org/TR/css-grid-1/#grid-item-sizing
    /// https://www.w3.org/TR/css-grid-1/#algo-overview
    ///
    /// Compute the available_space to be passed to the child sizing functions
    /// These are estimates based on either the max track sizing function or the provisional base size in the opposite
    /// axis to the one currently being sized.
    /// https://www.w3.org/TR/css-grid-1/#algo-overview
    pub fn grid_area_size(
        &self,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        other_axis_tracks: &[GridTrack],
        available_space: Size<Option<f32>>,
        get_track_size_estimate: impl Fn(&GridTrack, Option<f32>) -> Option<f32>,
        resolve_calc_value: &impl Fn(*const (), f32) -> f32,
    ) -> Size<Option<f32>> {
        let mut size = Size::NONE;
        size.set(
            axis,
            axis_tracks[self.track_range_excluding_lines(axis)]
                .iter()
                .map(|track| {
                    let min_size = track
                        .min_track_sizing_function
                        .definite_value(available_space.get(axis), resolve_calc_value)?;
                    let max_size = track
                        .max_track_sizing_function
                        .definite_value(available_space.get(axis), resolve_calc_value)?;

                    if min_size == max_size {
                        Some(track.base_size)
                    } else {
                        None
                    }
                })
                .sum::<Option<f32>>(),
        );

        // Replace only the standalone cross-axis estimate. Keep the native
        // current-axis fixed-track definiteness and percentage-cycle handling.
        if let Some(cross) = self.subgrid_cross.get(axis.other()) {
            size.set(axis.other(), Some(cross));
            return size;
        }
        size.set(
            axis.other(),
            other_axis_tracks[self.track_range_excluding_lines(axis.other())]
                .iter()
                .map(|track| {
                    get_track_size_estimate(track, available_space.get(axis.other()))
                        .map(|size| size + track.content_alignment_adjustment)
                })
                .sum::<Option<f32>>(),
        );

        size
    }

    /// Retrieve the available_space from the cache or compute them using the passed parameters
    pub fn grid_area_size_cached(
        &mut self,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        other_axis_tracks: &[GridTrack],
        available_space: Size<Option<f32>>,
        get_track_size_estimate: impl Fn(&GridTrack, Option<f32>) -> Option<f32>,
        resolve_calc_value: &impl Fn(*const (), f32) -> f32,
    ) -> Size<Option<f32>> {
        self.grid_area_size_cache.unwrap_or_else(|| {
            let grid_area_size = self.grid_area_size(
                axis,
                axis_tracks,
                other_axis_tracks,
                available_space,
                get_track_size_estimate,
                resolve_calc_value,
            );
            self.grid_area_size_cache = Some(grid_area_size);
            grid_area_size
        })
    }

    /// Compute the item's resolved margins for size contributions. Horizontal percentage margins always resolve
    /// to zero if the container size is indefinite as otherwise this would introduce a cyclic dependency.
    #[inline(always)]
    pub fn margins_axis_sums_with_baseline_shims(
        &self,
        inner_node_width: Option<f32>,
        tree: &impl LayoutPartialTree,
    ) -> Size<f32> {
        Rect {
            left: self
                .margin
                .left
                .resolve_or_zero(Some(0.0), |val, basis| tree.calc(val, basis))
                + self.baseline_shim_x
                + self.extra_margin.left,
            right: self
                .margin
                .right
                .resolve_or_zero(Some(0.0), |val, basis| tree.calc(val, basis))
                + self.baseline_shim_x_end
                + self.extra_margin.right,
            top: self
                .margin
                .top
                .resolve_or_zero(inner_node_width, |val, basis| tree.calc(val, basis))
                + self.baseline_shim
                + self.extra_margin.top,
            bottom: self
                .margin
                .bottom
                .resolve_or_zero(inner_node_width, |val, basis| tree.calc(val, basis))
                + self.baseline_shim_end
                + self.extra_margin.bottom,
        }
        .sum_axes()
    }

    /// Compute the item's min content contribution from the provided parameters
    pub fn min_content_contribution(
        &self,
        axis: AbstractAxis,
        tree: &mut impl LayoutPartialTree,
        grid_area_size: Size<Option<f32>>,
        available_space: Size<Option<f32>>,
    ) -> f32 {
        let known_dimensions = self.known_dimensions(tree, grid_area_size);
        // The child sees the grid area as its containing block during intrinsic measurement, so
        // percentage box properties resolve against the grid area when that size is definite.
        // Spec:
        // https://www.w3.org/TR/css-grid-1/#grid-item-sizing
        // https://www.w3.org/TR/css-grid-1/#algo-overview
        tree.measure_child_size(
            self.node,
            known_dimensions,
            grid_area_size,
            self.keyword_adjusted_available_space(
                grid_area_size,
                // Only the requested contribution axis is min-content. An
                // indefinite perpendicular axis must not force extra wrapping.
                available_space.map(|opt| match opt {
                    Some(size) => AvailableSpace::Definite(size),
                    None => AvailableSpace::MaxContent,
                }).with(axis, available_space.get(axis)
                    .map_or(AvailableSpace::MinContent, AvailableSpace::Definite)),
                tree,
            ),
            SizingMode::InherentSize,
            axis.as_abs_naive(),
            Line::FALSE,
        )
    }

    /// Retrieve the item's min content contribution from the cache or compute it using the provided parameters
    #[inline(always)]
    pub fn min_content_contribution_cached(
        &mut self,
        axis: AbstractAxis,
        tree: &mut impl LayoutPartialTree,
        grid_area_size: Size<Option<f32>>,
        available_space: Size<Option<f32>>,
    ) -> f32 {
        self.min_content_contribution_cache
            .get(axis)
            .unwrap_or_else(|| {
                let size =
                    self.min_content_contribution(axis, tree, grid_area_size, available_space);
                self.min_content_contribution_cache.set(axis, Some(size));
                size
            })
    }

    /// Compute the item's max content contribution from the provided parameters
    pub fn max_content_contribution(
        &self,
        axis: AbstractAxis,
        tree: &mut impl LayoutPartialTree,
        grid_area_size: Size<Option<f32>>,
        available_space: Size<Option<f32>>,
    ) -> f32 {
        let known_dimensions = self.known_dimensions(tree, grid_area_size);
        // See the min-content path above. Max-content measurement uses the same containing-block
        // basis so percentage-dependent item geometry is measured from the grid area rather than
        // from the container.
        tree.measure_child_size(
            self.node,
            known_dimensions,
            grid_area_size,
            self.keyword_adjusted_available_space(
                grid_area_size,
                available_space.map(|opt| match opt {
                    Some(size) => AvailableSpace::Definite(size),
                    None => AvailableSpace::MaxContent,
                }),
                tree,
            ),
            SizingMode::InherentSize,
            axis.as_abs_naive(),
            Line::FALSE,
        )
    }

    /// Override the available space in each axis whose size style is a sizing keyword that
    /// measures the item under a specific available space constraint
    /// (min-content, max-content, fit-content, fit-content(...))
    fn keyword_adjusted_available_space(
        &self,
        grid_area_size: Size<Option<f32>>,
        available_space: Size<AvailableSpace>,
        tree: &impl LayoutPartialTree,
    ) -> Size<AvailableSpace> {
        if !self.size.width.is_sizing_keyword() && !self.size.height.is_sizing_keyword() {
            return available_space;
        }
        let margins = self.margins_axis_sums_with_baseline_shims(grid_area_size.width, tree);
        let mut adjusted = available_space;
        for axis in [AbstractAxis::Inline, AbstractAxis::Block] {
            let size_style = self.size.get(axis);
            if !size_style.is_sizing_keyword() {
                continue;
            }
            let stretch_size = grid_area_size.get(axis).maybe_sub(margins.get(axis));
            if let Some(SizingKeywordResolution::Measure(available)) =
                resolve_sizing_keyword(size_style, stretch_size, grid_area_size.get(axis))
            {
                adjusted.set(axis, available);
            }
        }
        adjusted
    }

    /// Retrieve the item's max content contribution from the cache or compute it using the provided parameters
    #[inline(always)]
    pub fn max_content_contribution_cached(
        &mut self,
        axis: AbstractAxis,
        tree: &mut impl LayoutPartialTree,
        grid_area_size: Size<Option<f32>>,
        available_space: Size<Option<f32>>,
    ) -> f32 {
        self.max_content_contribution_cache
            .get(axis)
            .unwrap_or_else(|| {
                let size =
                    self.max_content_contribution(axis, tree, grid_area_size, available_space);
                self.max_content_contribution_cache.set(axis, Some(size));
                size
            })
    }

    /// The minimum contribution of an item is the smallest outer size it can have.
    /// Specifically:
    ///   - If the item’s computed preferred size behaves as auto or depends on the size of its containing block in the relevant axis:
    ///     Its minimum contribution is the outer size that would result from assuming the item’s used minimum size as its preferred size;
    ///   - Else the item’s minimum contribution is its min-content contribution.
    ///
    /// Because the minimum contribution often depends on the size of the item’s content, it is considered a type of intrinsic size contribution.
    /// See: https://www.w3.org/TR/css-grid-1/#min-size-auto
    pub fn minimum_contribution(
        &mut self,
        tree: &mut impl LayoutPartialTree,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        grid_area_size: Size<Option<f32>>,
        inner_node_size: Size<Option<f32>>,
    ) -> f32 {
        let padding = self
            .padding
            .resolve_or_zero(grid_area_size.width, |val, basis| tree.calc(val, basis));
        let border = self
            .border
            .resolve_or_zero(grid_area_size.width, |val, basis| tree.calc(val, basis));
        let padding_border_size = (padding + border).sum_axes();
        let box_sizing_adjustment = if self.box_sizing == BoxSizing::ContentBox {
            padding_border_size
        } else {
            Size::ZERO
        };
        // Only an indefinite sizing axis has a cyclic minimum percentage.
        // Definite fixed grid areas keep their native 0.14 percentage basis.
        let mut min_basis = grid_area_size;
        if min_basis.get(axis).is_none() {
            min_basis.set(axis, Some(0.0));
        }
        super::super::preferred_contribution::resolve(self, tree, grid_area_size, box_sizing_adjustment)
            .get(axis)
            .or_else(|| {
                self.min_size
                    .maybe_resolve(min_basis, |val, basis| tree.calc(val, basis))
                    .maybe_apply_aspect_ratio(self.aspect_ratio)
                    .maybe_add(box_sizing_adjustment)
                    .get(axis)
            })
            .or_else(|| self.overflow.get(axis).maybe_into_automatic_min_size())
            .unwrap_or_else(|| {
                // Automatic minimum size. See https://www.w3.org/TR/css-grid-1/#min-size-auto

                // To provide a more reasonable default minimum size for grid items, the used value of its automatic minimum size
                // in a given axis is the content-based minimum size if all of the following are true:
                let item_axis_tracks = &axis_tracks[self.track_range_excluding_lines(axis)];

                // it is not a scroll container
                // TODO: support overflow property

                // it spans at least one track in that axis whose min track sizing function is auto
                let spans_auto_min_track = axis_tracks
                    .iter()
                    // TODO: should this be 'behaves as auto' rather than just literal auto?
                    .any(|track| track.min_track_sizing_function.is_auto());

                // if it spans more than one track in that axis, none of those tracks are flexible
                let only_span_one_track = item_axis_tracks.len() == 1;
                let spans_a_flexible_track = axis_tracks
                    .iter()
                    .any(|track| track.max_track_sizing_function.is_fr());

                let use_content_based_minimum =
                    spans_auto_min_track && (only_span_one_track || !spans_a_flexible_track);

                // Otherwise, the automatic minimum size is zero, as usual.
                if use_content_based_minimum {
                    let mut minimum_contribution = self.min_content_contribution_cached(
                        axis,
                        tree,
                        grid_area_size,
                        grid_area_size,
                    );

                    // If the item is a compressible replaced element, and has a definite preferred size or maximum size in the
                    // relevant axis, the size suggestion is capped by those sizes; for this purpose, any indefinite percentages
                    // in these sizes are resolved against zero (and considered definite).
                    if self.is_compressible_replaced {
                        let size = self
                            .size
                            .get(axis)
                            .maybe_resolve(Some(0.0), |val, basis| tree.calc(val, basis));
                        let max_size = self
                            .max_size
                            .get(axis)
                            .maybe_resolve(Some(0.0), |val, basis| tree.calc(val, basis));
                        minimum_contribution =
                            minimum_contribution.maybe_min(size).maybe_min(max_size);
                    }

                    // The content-based minimum size is additionally clamped by the sum of any fixed max track sizing
                    // functions of the tracks the item spans. Note that this clamp does not apply to explicitly specified
                    // preferred or minimum sizes, and that the argument to fit-content() does not clamp the content-based
                    // minimum size in the same way as a fixed max track sizing function.
                    let limit = self.spanned_fixed_track_limit(
                        axis,
                        axis_tracks,
                        inner_node_size.get(axis),
                        &|val, basis| tree.resolve_calc_value(val, basis),
                    );
                    minimum_contribution.maybe_min(limit)
                } else {
                    0.0
                }
            })
    }

    /// Retrieve the item's minimum contribution from the cache or compute it using the provided parameters
    #[inline(always)]
    pub fn minimum_contribution_cached(
        &mut self,
        tree: &mut impl LayoutPartialTree,
        axis: AbstractAxis,
        axis_tracks: &[GridTrack],
        grid_area_size: Size<Option<f32>>,
        inner_node_size: Size<Option<f32>>,
    ) -> f32 {
        self.minimum_contribution_cache
            .get(axis)
            .unwrap_or_else(|| {
                let size = self.minimum_contribution(
                    tree,
                    axis,
                    axis_tracks,
                    grid_area_size,
                    inner_node_size,
                );
                self.minimum_contribution_cache.set(axis, Some(size));
                size
            })
    }
}

#[cfg(all(test, feature = "taffy_tree"))]
mod migration_tests {
    use super::*;
    use crate::style_helpers::{TaffyAuto, TaffyZero};
    use crate::{AlignmentSafety, Style, TaffyTree};

    fn item(style: &Style, parent: AlignItems) -> GridItem {
        let span = Line {
            start: OriginZeroLine(0),
            end: OriginZeroLine(1),
        };
        GridItem::new_with_placement_style_and_order(
            NodeId::new(0),
            span,
            span,
            style,
            AlignItems::STRETCH,
            parent,
            0,
        )
    }

    #[test]
    fn explicit_self_preserves_safety_before_parallel_fallback() {
        let own = AlignItems {
            keyword: AlignItemsKeyword::End,
            safety: AlignmentSafety::Safe,
        };
        let style = Style {
            justify_self: Some(own),
            baseline_x_flags: 8,
            ..Style::default()
        };
        assert_eq!(item(&style, AlignItems::BASELINE).justify_self, own);
        let implicit = Style {
            justify_self: None,
            ..style
        };
        assert_eq!(
            item(&implicit, AlignItems::BASELINE).justify_self,
            AlignItems::STRETCH
        );
        assert_eq!(
            item(&implicit, AlignItems::LAST_BASELINE).justify_self,
            AlignItems::LAST_BASELINE
        );
    }

    #[test]
    fn first_and_last_baselines_obey_auto_margin_and_cycle_guards() {
        for alignment in [AlignItems::BASELINE, AlignItems::LAST_BASELINE] {
            let mut grid_item = item(
                &Style {
                    align_self: Some(alignment),
                    ..Style::default()
                },
                AlignItems::STRETCH,
            );
            assert!(grid_item.participates_in_baseline_alignment());
            grid_item.margin.bottom = LengthPercentageAuto::AUTO;
            assert!(!grid_item.participates_in_baseline_alignment());
            grid_item.margin.bottom = LengthPercentageAuto::ZERO;
            grid_item.size.height = Dimension::percent(0.5);
            grid_item.crosses_intrinsic_row = true;
            assert!(!grid_item.participates_in_baseline_alignment());
            grid_item.crosses_intrinsic_row = false;
            grid_item.crosses_flexible_row = true;
            assert!(!grid_item.participates_in_baseline_alignment());
            grid_item.crosses_flexible_row = false;
            assert!(grid_item.participates_in_baseline_alignment());
        }
    }

    #[test]
    fn x_baseline_guards_use_columns_and_horizontal_margins_only() {
        for alignment in [AlignItems::BASELINE, AlignItems::LAST_BASELINE] {
            let mut grid_item = item(
                &Style {
                    justify_self: Some(alignment),
                    ..Style::default()
                },
                AlignItems::STRETCH,
            );
            assert!(grid_item.participates_in_baseline_alignment_x());
            grid_item.margin.left = LengthPercentageAuto::AUTO;
            assert!(!grid_item.participates_in_baseline_alignment_x());
            grid_item.margin.left = LengthPercentageAuto::ZERO;
            grid_item.margin.right = LengthPercentageAuto::AUTO;
            assert!(!grid_item.participates_in_baseline_alignment_x());
            grid_item.margin.right = LengthPercentageAuto::ZERO;
            // A y cycle and vertical auto margin do not suppress an x group.
            grid_item.size.height = Dimension::percent(0.5);
            grid_item.crosses_intrinsic_row = true;
            grid_item.margin.top = LengthPercentageAuto::AUTO;
            assert!(grid_item.participates_in_baseline_alignment_x());
            grid_item.size.width = Dimension::percent(0.5);
            // A percentage width in fixed columns is not cyclic.
            assert!(grid_item.participates_in_baseline_alignment_x());
            grid_item.crosses_intrinsic_column = true;
            assert!(!grid_item.participates_in_baseline_alignment_x());
            grid_item.crosses_intrinsic_column = false;
            grid_item.crosses_flexible_column = true;
            assert!(!grid_item.participates_in_baseline_alignment_x());
            grid_item.size.width = Dimension::length(100.0);
            assert!(grid_item.participates_in_baseline_alignment_x());
            grid_item.justify_self = AlignItems::START;
            assert!(!grid_item.participates_in_baseline_alignment_x());
        }
    }

    #[test]
    fn flattened_mask_is_independent_of_baseline_metadata() {
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.sizing_axes = 1;
        grid_item.flattened = true;
        grid_item.baseline = Some(7.0);
        grid_item.last_baseline = Some(17.0);
        grid_item.baseline_x = Some(11.0);
        let clone = grid_item.clone();
        assert!(clone.sizes_axis(AbstractAxis::Inline));
        assert!(!clone.sizes_axis(AbstractAxis::Block));
        assert_eq!(
            (clone.baseline, clone.last_baseline, clone.baseline_x),
            (Some(7.0), Some(17.0), Some(11.0))
        );
    }

    #[test]
    fn signed_ancestor_margins_and_four_shims_accumulate_per_axis() {
        let mut tree: TaffyTree = TaffyTree::new();
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.extra_margin = Rect {
            left: -30.0,
            right: 4.0,
            top: -10.0,
            bottom: 2.0,
        };
        grid_item.baseline_shim_x = 3.0;
        grid_item.baseline_shim_x_end = 5.0;
        grid_item.baseline_shim = 7.0;
        grid_item.baseline_shim_end = 11.0;
        assert_eq!(
            grid_item.margins_axis_sums_with_baseline_shims(None, &tree.as_layout_tree()),
            Size {
                width: -18.0,
                height: 10.0
            }
        );
    }

    fn fixed_tracks(size: f32) -> [GridTrack; 3] {
        let mut track = GridTrack::new(
            crate::MinTrackSizingFunction::length(size),
            crate::MaxTrackSizingFunction::length(size),
        );
        track.base_size = size;
        [
            GridTrack::gutter(LengthPercentage::ZERO),
            track,
            GridTrack::gutter(LengthPercentage::ZERO),
        ]
    }

    #[test]
    fn subgrid_cross_keeps_definite_fixed_current_axis() {
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.column_indexes = Line { start: 0, end: 2 };
        grid_item.row_indexes = Line { start: 0, end: 2 };
        grid_item.subgrid_cross = Size {
            width: None,
            height: Some(31.0),
        };
        let result = grid_item.grid_area_size(
            AbstractAxis::Inline,
            &fixed_tracks(80.0),
            &[],
            Size::NONE,
            |_, _| panic!("cross override must bypass estimates"),
            &|_, _| 0.0,
        );
        assert_eq!(
            result,
            Size {
                width: Some(80.0),
                height: Some(31.0)
            }
        );
    }

    #[test]
    fn subgrid_cross_keeps_intrinsic_current_axis_indefinite() {
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.column_indexes = Line { start: 0, end: 2 };
        grid_item.subgrid_cross.height = Some(31.0);
        let mut tracks = fixed_tracks(80.0);
        tracks[1].max_track_sizing_function = crate::MaxTrackSizingFunction::AUTO;
        let result = grid_item.grid_area_size(
            AbstractAxis::Inline,
            &tracks,
            &[],
            Size {
                width: Some(300.0),
                height: None,
            },
            |_, _| panic!("override"),
            &|_, _| 0.0,
        );
        assert_eq!(
            result,
            Size {
                width: None,
                height: Some(31.0)
            }
        );
    }

    #[test]
    fn percentage_sizes_use_grid_area_and_cyclic_minimum_uses_zero() {
        let mut tree: TaffyTree = TaffyTree::new();
        let style = Style {
            size: Size {
                width: Dimension::percent(0.5),
                height: Dimension::AUTO,
            },
            min_size: Size {
                width: LengthPercentageAuto::percent(0.8),
                height: LengthPercentageAuto::AUTO,
            },
            ..Style::default()
        };
        let mut grid_item = item(&style, AlignItems::STRETCH);
        grid_item.column_indexes = Line { start: 0, end: 2 };
        let known = grid_item.known_dimensions(
            &mut tree.as_layout_tree(),
            Size {
                width: Some(80.0),
                height: None,
            },
        );
        assert_eq!(known.width, Some(64.0));
        // A zero cyclic min contribution returns before measuring NodeId(0).
        assert_eq!(
            grid_item.minimum_contribution(
                &mut tree.as_layout_tree(),
                AbstractAxis::Inline,
                &fixed_tracks(80.0),
                Size::NONE,
                Size {
                    width: Some(300.0),
                    height: None
                }
            ),
            0.0
        );
        assert_eq!(
            grid_item.minimum_contribution(
                &mut tree.as_layout_tree(),
                AbstractAxis::Inline,
                &fixed_tracks(80.0),
                Size {
                    width: Some(80.0),
                    height: None
                },
                Size {
                    width: Some(300.0),
                    height: None
                }
            ),
            40.0
        );
    }

    #[test]
    fn standalone_auto_cross_width_keeps_min_content_floor() {
        let mut tree: TaffyTree = TaffyTree::new();
        let child = tree
            .new_leaf(Style {
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::AUTO,
                },
                ..Style::default()
            })
            .unwrap();
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.node = child;
        grid_item.subgrid_cross_auto.width = true;
        for (area, expected) in [(58.0, 100.0), (120.0, 120.0)] {
            assert_eq!(
                grid_item
                    .known_dimensions(
                        &mut tree.as_layout_tree(),
                        Size {
                            width: Some(area),
                            height: None
                        }
                    )
                    .width,
                Some(expected)
            );
        }
        grid_item.size.width = Dimension::length(50.0);
        assert_eq!(
            grid_item
                .known_dimensions(
                    &mut tree.as_layout_tree(),
                    Size {
                        width: Some(58.0),
                        height: None
                    }
                )
                .width,
            Some(50.0)
        );
    }

    #[test]
    fn native_keyword_measurement_remains_available() {
        let mut tree: TaffyTree = TaffyTree::new();
        let mut grid_item = item(&Style::default(), AlignItems::STRETCH);
        grid_item.size.width = Dimension::min_content();
        let available = Size {
            width: AvailableSpace::Definite(80.0),
            height: AvailableSpace::MaxContent,
        };
        let adjusted = grid_item.keyword_adjusted_available_space(
            Size {
                width: Some(80.0),
                height: None,
            },
            available,
            &tree.as_layout_tree(),
        );
        assert_eq!(adjusted.width, AvailableSpace::MinContent);
        assert_eq!(adjusted.height, AvailableSpace::MaxContent);
    }
}
