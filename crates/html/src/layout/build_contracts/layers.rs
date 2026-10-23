//! Слои покраски поверх геометрии, собранной пробами layout в том же кадре.

use crate::document::state::ValueCx;
use crate::layout::cx::{LayoutCx, deferred::Deferred};
use crate::layout::multicol::gap_rules::{GapItems, GapRuleSpec};
use crate::layout::table::probes::{CellBgs, CellEdges, RowRects};
use crate::style::computed::Computed;
use std::rc::Rc;

/// A paint layer over geometry that layout's probes record in prepaint of
/// the same frame (`layout::table::probes`). The layer is laid out of flow
/// and only paints.
pub(crate) enum Layer<'a> {
    /// A row, row-group or column band: its background, shadows and outline
    /// painted inside the cells of the band (css-tables-3
    /// §drawing-backgrounds), under the given style.
    CellBand {
        rects: RowRects,
        style: Box<Computed>,
        vcx: ValueCx<'a>,
    },
    /// The cell backgrounds of a collapsed-border table, below its edges.
    CellBackgrounds(CellBgs),
    /// The collapsed edges of a table (CSS 2.1 §17.6.2.1).
    CollapsedEdges(CellEdges),
    /// Линейки промежутков над геометрией детей и колонок, под содержимым.
    GapRules {
        items: GapItems,
        spec: Box<GapRuleSpec>,
        owner: Rc<Deferred>,
    },
}

impl<'a> Layer<'a> {
    pub(crate) fn gap_rules(items: GapItems, spec: GapRuleSpec, cx: &LayoutCx) -> Self {
        Layer::GapRules {
            items,
            spec: Box::new(spec),
            owner: cx.deferred.paint.clone(),
        }
    }

    /// The band over `rects` under `style`, its values from the build `cx`.
    pub(crate) fn cell_band(rects: RowRects, style: Computed, cx: &'a LayoutCx) -> Self {
        Layer::CellBand {
            rects,
            style: Box::new(style),
            vcx: cx.vcx(),
        }
    }
}
