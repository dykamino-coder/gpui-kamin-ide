//! Идентификаторы коробок и таблиц, наблюдаемых рекордером измеренной стопки.
use super::BlockBox;
impl BlockBox {
    /// Принадлежность содержимого не выводится из пересечения параллельных потоков.
    pub(crate) fn descendants(&self, out: &mut std::collections::HashMap<u64, Vec<u64>>) {
        out.insert(
            self.id,
            self.ids().into_iter().filter(|&id| id != self.id).collect(),
        );
        for inner in self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .map(|k| k.inner.as_ref())
            .chain(
                self.grid
                    .iter()
                    .flat_map(|g| &g.items)
                    .map(|k| k.inner.as_ref()),
            )
            .flatten()
            .chain(self.table.iter().flat_map(|t| t.flows()))
        {
            inner.descendants(out);
        }
    }

    pub(super) fn push_ids(&self, out: &mut Vec<u64>) {
        for item in self.flex.iter().flat_map(|f| &f.items) {
            out.push(item.id);
            if let Some(inner) = &item.inner {
                inner.push_ids(out);
            }
        }
        if let Some(g) = &self.grid {
            g.push_ids(out);
        }
        for k in &self.kids {
            out.push(k.id);
            if let Some(inner) = &k.inner {
                inner.push_ids(out);
            }
        }
        if let Some(t) = &self.table {
            t.push_ids(out);
        }
    }

    pub(super) fn push_keys(&self, out: &mut Vec<u64>) {
        for inner in self
            .flex
            .iter()
            .flat_map(|f| &f.items)
            .filter_map(|i| i.inner.as_ref())
        {
            inner.push_keys(out);
        }
        if let Some(g) = &self.grid {
            for it in &g.items {
                if let Some(inner) = &it.inner {
                    inner.push_keys(out);
                }
            }
        }
        for k in &self.kids {
            if let Some(inner) = &k.inner {
                inner.push_keys(out);
            }
        }
        if let Some(t) = &self.table {
            t.push_keys(out);
        }
    }
}

impl BlockBox {
    /// Relative-сдвиг элемента не меняет геометрию его потока (§5.5 CSS Break 3).
    pub(crate) fn shift_flex_item(
        &self,
        id: u64,
        delta: f32,
        boxes: &mut std::collections::HashMap<u64, (f32, f32)>,
        shifts: &mut std::collections::HashMap<u64, f32>,
        tables: &mut std::collections::HashMap<u64, f32>,
    ) {
        for (item_id, inner) in self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .map(|i| (i.id, i.inner.as_ref()))
            .chain(
                self.grid
                    .iter()
                    .flat_map(|g| &g.items)
                    .map(|i| (i.id, i.inner.as_ref())),
            )
        {
            if item_id == id {
                let ids = inner.map_or_else(|| vec![id], BlockBox::ids);
                for child in ids {
                    *shifts.entry(child).or_default() += delta;
                    if let Some(span) = boxes.get_mut(&child) {
                        span.0 += delta;
                        span.1 += delta;
                    }
                }
                if let Some(inner) = inner {
                    for key in inner.table_keys() {
                        *tables.entry(key).or_default() += delta;
                    }
                }
                return;
            }
            if let Some(inner) = inner {
                inner.shift_flex_item(id, delta, boxes, shifts, tables);
            }
        }
        for flow in self.table.iter().flat_map(|t| t.flows()) {
            flow.shift_flex_item(id, delta, boxes, shifts, tables);
        }
    }
}
