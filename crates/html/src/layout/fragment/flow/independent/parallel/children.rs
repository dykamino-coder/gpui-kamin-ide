//! Стабильный порядок дочерних потоков, общий для записи и чтения child tokens.
use crate::layout::fragment::flow::blocks::BlockBox;

pub(super) struct Child<'a> {
    pub(super) id: u64,
    pub(super) monolith: bool,
    pub(super) inner: Option<&'a BlockBox>,
}

pub(super) fn children(b: &BlockBox) -> Vec<Child<'_>> {
    let mut out: Vec<_> = b
        .kids
        .iter()
        .chain(b.flex.iter().flat_map(|f| &f.items))
        .map(|k| Child {
            id: k.id,
            monolith: k.monolith,
            inner: k.inner.as_ref(),
        })
        .collect();
    out.extend(b.grid.iter().flat_map(|g| &g.items).map(|k| Child {
        id: k.id,
        monolith: k.monolith,
        inner: k.inner.as_ref(),
    }));
    if let Some(t) = &b.table {
        out.extend(t.caps_top.iter().map(|k| Child {
            id: k.id,
            monolith: k.monolith,
            inner: k.inner.as_ref(),
        }));
        out.extend(t.rows.iter().flat_map(|r| &r.cells).map(|(_, k)| Child {
            id: k.id,
            monolith: false,
            inner: Some(k),
        }));
        out.extend(t.caps_bot.iter().map(|k| Child {
            id: k.id,
            monolith: k.monolith,
            inner: k.inner.as_ref(),
        }));
    }
    out
}
