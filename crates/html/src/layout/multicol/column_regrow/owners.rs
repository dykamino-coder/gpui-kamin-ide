//! Владение flex-элементами: первый фрагмент определяется началом margin box.

use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;

impl ColumnStack {
    /// Первый фрагмент каждого flex-элемента: окно, в котором начинается его
    /// margin box (`paginate::watch::owner`). Элемент между окнами (после
    /// разрыва) — следующего.
    pub(crate) fn assign_flex_owners(&self, plan: &[crate::layout::fragment::types::Frag]) {
        for (i, c) in self.children.iter().enumerate() {
            let (Some(r), Some(w)) = (c.regrow.as_ref(), c.watched.as_ref()) else {
                continue;
            };
            if r.laid.flexes.is_empty() {
                continue;
            }
            let mut frags: Vec<_> = plan.iter().filter(|f| f.kid == i).collect();
            frags.sort_by_key(|f| f.copy);
            let owners = match frags.last() {
                Some(last) if frags.len() > 1 => r
                    .laid
                    .margins
                    .keys()
                    .filter_map(|&id| {
                        let start = r.laid.margin_box(id)?.0;
                        let owner = frags
                            .iter()
                            .find(|f| start < f.from + f.h - COORD_EPS)
                            .map_or(last.copy, |f| f.copy);
                        Some((id, owner))
                    })
                    .collect(),
                _ => Default::default(),
            };
            w.set_flex_owners(owners);
        }
    }
}
