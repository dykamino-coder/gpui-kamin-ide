//! Longhand-to-computed-field registry shared by keyword tracking and copying.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;

macro_rules! properties {
    ($($id:ident: $name:literal => [$($field:ident $(.$part:ident)*),+];)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Property { $($id,)* RowGap, ColumnGap, ImageSource, ImageSlice, ImageWidth, ImageOutset, ImageRepeat }

        impl Property {
            #[allow(clippy::clone_on_copy)] // One registry covers both owned and Copy fields.
            pub(super) fn copy(self, p: &Computed, c: &mut Computed, vcx: ValueCx<'_>) {
                match self {
                    $(Self::$id => { $(c.$field $(.$part)* = p.$field $(.$part)*.clone();)+ },)*
                    Self::RowGap => c.gap.get_or_insert((None, None)).0 = p.gap.and_then(|g| g.0),
                    Self::ColumnGap => {
                        c.column_gap = p.column_gap;
                        c.gap.get_or_insert((None, None)).1 = p.gap.and_then(|g| g.1);
                    }
                    Self::ImageSource | Self::ImageSlice | Self::ImageWidth | Self::ImageOutset | Self::ImageRepeat => {
                        // Initialise just this longhand; preserve source, slices and repeat.
                        let (key, initial) = match self {
                            Self::ImageSource => ("border-image-source", "none"),
                            Self::ImageSlice => ("border-image-slice", "100%"),
                            Self::ImageWidth => ("border-image-width", "1"),
                            Self::ImageOutset => ("border-image-outset", "0"),
                            _ => ("border-image-repeat", "stretch"),
                        };
                        c.set_border_image(key, initial, vcx);
                        if let (Some(src), Some(dst)) = (&p.border_image, &mut c.border_image) {
                            match self {
                                Self::ImageSource => dst.src.clone_from(&src.src),
                                Self::ImageSlice => { dst.slice = src.slice; dst.fill = src.fill; }
                                Self::ImageWidth => dst.width = src.width,
                                Self::ImageOutset => dst.outset = src.outset,
                                _ => dst.repeat = src.repeat,
                            }
                        }
                    }
                }
                let corner = match self {
                    Self::TopLeftRadius => Some(0), Self::TopRightRadius => Some(1),
                    Self::BottomRightRadius => Some(2), Self::BottomLeftRadius => Some(3),
                    _ => None,
                };
                if let Some(i) = corner.filter(|_| c.radius_ell.is_some() || p.radius_ell.is_some()) {
                    c.radius_ell.get_or_insert([None; 4])[i] = p.radius_ell.and_then(|r| r[i]);
                    if c.radius_ell.is_some_and(|r| r.iter().all(Option::is_none)) {
                        c.radius_ell = None;
                    }
                }
                if self == Self::Perspective {
                    c.perspective_frame = c.perspective.map(|_| Default::default());
                }
                if matches!(self, Self::GridColumns | Self::GridRows) {
                    c.subgrid = c.subgrid_cols || c.subgrid_rows;
                }
            }
        }

        pub(super) fn properties(key: &str) -> &'static [Property] {
            use Property::*;
            match key {
                $($name => &[$id],)*
                "row-gap" => &[RowGap],
                "column-gap" => &[ColumnGap],
                "gap" => &[RowGap, ColumnGap],
                "columns" => &[ColumnWidth, ColumnCount],
                "column-rule" => &[ColumnRuleWidth, ColumnRuleStyle, ColumnRuleColor],
                "border-radius" => &[TopLeftRadius, TopRightRadius, BottomRightRadius, BottomLeftRadius],
                "grid-template" => &[GridColumns, GridRows, GridAreas],
                "grid" => &[GridColumns, GridRows, GridAreas, GridAutoColumns, GridAutoRows, GridAutoFlow],
                "border-image" => &[ImageSource, ImageSlice, ImageWidth, ImageOutset, ImageRepeat],
                "border-image-source" => &[ImageSource],
                "border-image-slice" => &[ImageSlice],
                "border-image-repeat" => &[ImageRepeat],
                "border-image-width" => &[ImageWidth],
                "border-image-outset" => &[ImageOutset],
                _ => &[],
            }
        }
    };
}

properties! {
    PathLength: "path-length" => [svg_path_length];
    ColumnWidth: "column-width" => [column_width];
    ColumnCount: "column-count" => [column_count];
    ColumnRuleWidth: "column-rule-width" => [column_rule_width, column_rule_widths];
    ColumnRuleStyle: "column-rule-style" => [column_rule_visible, column_rule_styles, column_rule_double, column_rule_kinds];
    ColumnRuleColor: "column-rule-color" => [column_rule_color, column_rule_colors];
    RowRuleWidth: "row-rule-width" => [row_rule_width, row_rule_widths];
    Perspective: "perspective" => [perspective, perspective_frame];
    PerspectiveOrigin: "perspective-origin" => [perspective_origin, perspective_origin_px];
    Transform: "transform" => [transform, transform_raw];
    TransformOrigin: "transform-origin" => [transform_origin, transform_origin_px, transform_origin_z, transform_origin_raw];
    Translate: "translate" => [translate];
    Filter: "filter" => [filter, filter_ref];
    ZIndex: "z-index" => [z_index];
    VerticalAlign: "vertical-align" => [vertical_align, vertical_align_text, vertical_shift, vertical_shift_pct, vertical_shift_px, vertical_shift_len];
    DecorationThickness: "text-decoration-thickness" => [td_thickness];
    DecorationInset: "text-decoration-inset" => [td_inset];
    BorderSpacing: "border-spacing" => [border_spacing];
    TopLeftRadius: "border-top-left-radius" => [radius.tl];
    TopRightRadius: "border-top-right-radius" => [radius.tr];
    BottomRightRadius: "border-bottom-right-radius" => [radius.br];
    BottomLeftRadius: "border-bottom-left-radius" => [radius.bl];
    GridColumns: "grid-template-columns" => [grid_cols, grid_tracks, grid_col_line_names, grid_auto_fill_min, grid_auto_fill_tracks, subgrid_cols, auto_repeat_body_cols, auto_repeat_cols];
    GridRows: "grid-template-rows" => [grid_rows, grid_row_line_names, grid_auto_fill_row, subgrid_rows, auto_repeat_body_rows, auto_repeat_rows];
    GridAreas: "grid-template-areas" => [grid_areas];
    GridAutoColumns: "grid-auto-columns" => [grid_auto_cols, grid_auto_cols_list];
    GridAutoRows: "grid-auto-rows" => [grid_auto_rows, grid_auto_rows_list];
    GridAutoFlow: "grid-auto-flow" => [grid_auto_flow];
    ShapeOutside: "shape-outside" => [shape_outside];
}
