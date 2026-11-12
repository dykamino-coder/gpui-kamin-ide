//! Описание виртуальных частей собирается только по явному запросу геометрии.

use super::super::context::TableGeom;

pub(super) struct Part {
    pub(super) node: u64,
    pub(super) empty_row: Option<usize>,
    pub(super) row: Option<usize>,
    pub(super) columns: Vec<usize>,
    pub(super) group: Option<u64>,
    pub(super) shift: (f32, f32),
    pub(super) group_shift: (f32, f32),
    pub(super) font: (f64, crate::LineHeight),
}

pub(super) fn collect(
    geom: &TableGeom<'_>,
    opts: &crate::layout::options::RenderOpts,
) -> Vec<Part> {
    let font = |e: &crate::dom::Element| {
        crate::layout::geometry::font(
            &e.style.used(),
            crate::defaults::MEDIUM_FONT_SIZE_PX,
            opts.cx.vcx(),
        )
    };
    let fixed = super::super::setup::table_rows_fixed(super::super::anon::fixup_table_children(
        &geom.e.children,
    ));
    let mut rows = Vec::new();
    super::super::anon::collect_rows(&fixed, Some(geom.e), Default::default(), &mut rows);
    let mut parts = Vec::new();
    for (i, row) in geom.row_elements.iter().enumerate() {
        let record = rows.iter().find(|r| r.element.node_id == row.node_id);
        let carry = record.map(|r| r.carry).unwrap_or_default();
        let group = carry.group.map(|g| g.node_id);
        let group_shift = carry
            .group
            .map(|g| crate::layout::positioned::relative::relative_shift(g, Some(geom.e)))
            .unwrap_or_default();
        parts.push(Part {
            node: row.node_id,
            empty_row: None,
            row: Some(i),
            columns: Vec::new(),
            group,
            shift: (carry.dx, carry.dy),
            group_shift,
            font: font(row),
        });
        if let Some(group) = carry.group
            && !parts.iter().any(|p| p.node == group.node_id)
        {
            parts.push(Part {
                node: group.node_id,
                empty_row: None,
                row: None,
                columns: Vec::new(),
                group: None,
                shift: (0., 0.),
                group_shift: (0., 0.),
                font: font(group),
            });
        }
    }
    let mut boundary = 0;
    for node in &fixed {
        let crate::dom::Node::Element(element) = node else {
            continue;
        };
        let mut group_rows = Vec::new();
        super::super::anon::collect_rows(
            std::slice::from_ref(node),
            Some(geom.e),
            Default::default(),
            &mut group_rows,
        );
        if group_rows.is_empty()
            && (matches!(element.tag.as_str(), "thead" | "tbody" | "tfoot")
                || element.style.used().reset_box_layout.display
                    == Some(crate::style::computed::Display::TableRowGroup))
        {
            let shift = crate::layout::positioned::relative::relative_shift(element, Some(geom.e));
            parts.push(Part {
                node: element.node_id,
                empty_row: Some(boundary),
                row: None,
                columns: Vec::new(),
                group: None,
                shift,
                group_shift: shift,
                font: font(element),
            });
        }
        boundary += group_rows.len();
    }
    for columns in [
        super::super::columns::col_elements(&geom.e.children),
        super::super::columns::colgroup_elements(&geom.e.children),
    ] {
        for (i, column) in columns.iter().enumerate() {
            let Some(column) = column else { continue };
            if let Some(part) = parts.iter_mut().find(|p| p.node == column.node_id) {
                part.columns.push(i);
            } else {
                parts.push(Part {
                    node: column.node_id,
                    empty_row: None,
                    row: None,
                    columns: vec![i],
                    group: None,
                    shift: (0., 0.),
                    group_shift: (0., 0.),
                    font: font(column),
                });
            }
        }
    }
    parts
}
