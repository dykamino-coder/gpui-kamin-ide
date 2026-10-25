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
