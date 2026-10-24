//! Break propagation at the actual table wrapper edges (CSS Break 3 §3.1).
use super::TableBox;
use crate::layout::fragment::flow::blocks::Laid;

impl TableBox {
    pub(in crate::layout::fragment::flow::blocks) fn edge(
        &self,
        last: bool,
        laid: &Laid,
    ) -> (bool, bool) {
        let caption = if last {
            self.caps_bot.last()
        } else {
            self.caps_top.first()
        };
        if let Some(caption) = caption {
            return caption.edge(last, laid);
        }
        let row = if last {
            self.rows.last()
        } else {
            self.rows.first()
        };
        let Some(row) = row else {
            return (false, false);
        };
        let mut edge = if last {
            (row.force_after, row.avoid_after)
        } else {
            (row.force_before, row.avoid_before)
        };
        for (_, cell) in &row.cells {
            let value = cell.edge(last, laid);
            edge.0 |= value.0;
            edge.1 |= value.1;
        }
        edge
    }
}
