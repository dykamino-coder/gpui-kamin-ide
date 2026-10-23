//! Каскад: реестр «longhand → поля `Computed`» для явного наследования.
//! Один список служит и учёту CSS-слов (`properties(key)`, включая раскрытие
//! сокращения `border-image`; сокращения реестра раскрыты разбором), и копированию полей
//! родителя (`Property::copy`) вместе с зависимыми полями (радиусы-эллипсы,
//! кадр перспективы, признак subgrid).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;

macro_rules! properties {
    ($($id:ident: $name:literal => [$($field:ident $(.$part:ident)*),+];)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Property { $($id,)* RowGap, ColumnGap, GridLine(bool, bool), RuleInset(bool, u8), ImageSource, ImageSlice, ImageWidth, ImageOutset, ImageRepeat, MaskBorderPart(u8) }

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
                    // Грань размещения сетки: своя половина линий и имён.
                    Self::GridLine(row, end) => {
                        let i = usize::from(end);
                        let (from, from_named, to, to_named) = if row {
                            (p.grid_row, &p.grid_row_named, &mut c.grid_row, &mut c.grid_row_named)
                        } else {
                            (p.grid_col, &p.grid_col_named, &mut c.grid_col, &mut c.grid_col_named)
                        };
                        let auto = crate::style::computed::Placement::Auto;
                        let mut pair = to.unwrap_or((auto, auto));
                        let src = from.unwrap_or((auto, auto));
                        if end { pair.1 = src.1 } else { pair.0 = src.0 }
                        to_named[i] = from_named[i].clone();
                        let unset = pair.0 == auto && pair.1 == auto && to_named.iter().all(Option::is_none);
                        *to = (!unset).then_some(pair);
                        c.grid_area_name = crate::style::computed::grid_area_name(&c.grid_row_named, &c.grid_col_named);
                    }
                    // Грань втяжки линейки (css-gaps-1 §inset).
                    Self::RuleInset(column, slot) => {
                        let (from, to) = if column {
                            (p.column_rule_inset, &mut c.column_rule_inset)
                        } else {
                            (p.row_rule_inset, &mut c.row_rule_inset)
                        };
                        let zero = crate::style::computed::GapInset::Len(crate::style::values::value::Len::Px(0.0));
                        to.get_or_insert([zero; 4])[usize::from(slot)] = from.map_or(zero, |f| f[usize::from(slot)]);
                    }
                    Self::MaskBorderPart(part) => {
                        let initial = crate::style::computed::initial_mask_border(vcx);
                        let src = p.mask_border.clone().unwrap_or_else(|| initial.clone());
                        let mut dst = c.mask_border.clone().unwrap_or_else(|| initial.clone());
                        match part {
                            0 => dst.src = src.src,
                            1 => { dst.slice = src.slice; dst.fill = src.fill; }
                            2 => dst.width = src.width,
                            3 => dst.outset = src.outset,
                            _ => dst.repeat = src.repeat,
                        }
                        c.mask_border = (dst != initial).then_some(dst);
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
                "grid-row-start" => &[GridLine(true, false)],
                "grid-row-end" => &[GridLine(true, true)],
                "grid-column-start" => &[GridLine(false, false)],
                "grid-column-end" => &[GridLine(false, true)],
                "column-rule-inset-cap-start" => &[RuleInset(true, 0)],
                "column-rule-inset-cap-end" => &[RuleInset(true, 1)],
                "column-rule-inset-junction-start" => &[RuleInset(true, 2)],
                "column-rule-inset-junction-end" => &[RuleInset(true, 3)],
                "row-rule-inset-cap-start" => &[RuleInset(false, 0)],
                "row-rule-inset-cap-end" => &[RuleInset(false, 1)],
                "row-rule-inset-junction-start" => &[RuleInset(false, 2)],
                "row-rule-inset-junction-end" => &[RuleInset(false, 3)],
                "border-radius" => &[TopLeftRadius, TopRightRadius, BottomRightRadius, BottomLeftRadius],
                "border-image" => &[ImageSource, ImageSlice, ImageWidth, ImageOutset, ImageRepeat],
                "border-image-source" => &[ImageSource],
                "border-image-slice" => &[ImageSlice],
                "border-image-repeat" => &[ImageRepeat],
                "border-image-width" => &[ImageWidth],
                "border-image-outset" => &[ImageOutset],
                "mask-border-source" => &[MaskBorderPart(0)],
                "mask-border-slice" => &[MaskBorderPart(1)],
                "mask-border-width" => &[MaskBorderPart(2)],
                "mask-border-outset" => &[MaskBorderPart(3)],
                "mask-border-repeat" => &[MaskBorderPart(4)],
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
    RowRuleStyle: "row-rule-style" => [row_rule_visible, row_rule_styles, row_rule_double, row_rule_kinds];
    ColumnRuleBreak: "column-rule-break" => [column_rule_break];
    RowRuleBreak: "row-rule-break" => [row_rule_break];
    ColumnRuleVisibility: "column-rule-visibility-items" => [column_rule_visibility];
    RowRuleVisibility: "row-rule-visibility-items" => [row_rule_visibility];
    ColumnHeight: "column-height" => [column_height];
    ColumnWrap: "column-wrap" => [column_wrap];
    FlexGrow: "flex-grow" => [flex_grow];
    FlexShrink: "flex-shrink" => [flex_shrink];
    FlexBasis: "flex-basis" => [flex_basis, basis_content];
    FlexDirection: "flex-direction" => [flex_dir];
    FlexWrap: "flex-wrap" => [flex_wrap, flex_wrap_reverse, flex_balance];
    Perspective: "perspective" => [perspective, perspective_frame];
    PerspectiveOrigin: "perspective-origin" => [perspective_origin, perspective_origin_px];
    Transform: "transform" => [transform, transform_raw];
    TransformOrigin: "transform-origin" => [transform_origin, transform_origin_px, transform_origin_z, transform_origin_raw];
    Translate: "translate" => [translate];
    Filter: "filter" => [filter, filter_ref];
    ZIndex: "z-index" => [z_index];
    VerticalAlign: "vertical-align" => [vertical_align, vertical_align_text, vertical_shift, vertical_shift_pct, vertical_shift_px, vertical_shift_len];
    DecorationLine: "text-decoration-line" => [td_lines, underline, line_through];
    DecorationStyle: "text-decoration-style" => [td_style];
    DecorationColor: "text-decoration-color" => [td_color];
    DecorationThickness: "text-decoration-thickness" => [td_thickness];
    DecorationInset: "text-decoration-inset" => [td_inset];
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
    MaskImage: "mask-image" => [mask_image];
    MaskMode: "mask-mode" => [mask_luminance, mask_alpha_mode];
    MaskPosition: "mask-position" => [mask_pos, mask_pos_far, mask_pos_list];
    MaskSize: "mask-size" => [mask_size, mask_fit];
    MaskRepeat: "mask-repeat" => [mask_no_repeat, mask_repeat_list, mask_repeat_modes];
    MaskOrigin: "mask-origin" => [mask_origin];
    MaskClip: "mask-clip" => [mask_clip];
    MaskComposite: "mask-composite" => [mask_composite];
    MaskBorderMode: "mask-border-mode" => [mask_border_luminance];
    MaxLines: "max-lines" => [line_clamp, clamp_auto];
    Continue: "continue" => [clamp_continue, clamp_legacy];
    TextBoxTrim: "text-box-trim" => [text_box_trim_start, text_box_trim_end];
}
