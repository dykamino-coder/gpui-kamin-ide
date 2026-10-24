//! Исполнение leaf до завершения: число фрагментов определяется токенами, не запасом копий.

use crate::layout::fragment::flow::blocks::leaf::Leaf;
use crate::layout::fragment::token::{Error, Space, TokenStore};
use crate::layout::fragment::types::Frag;

pub(crate) fn fragment_leaf(
    leaf_at: &dyn Fn(Space) -> Leaf,
    kid: usize,
    mut col: usize,
    mut offset: f32,
    space_at: &dyn Fn(usize) -> Space,
) -> Result<Vec<Frag>, Error> {
    let mut store = TokenStore::default();
    let mut token = None;
    let mut fragments = Vec::new();
    loop {
        let space = space_at(col);
        let output = leaf_at(space).resume(space, offset, token, &mut store)?;
        if let Some(f) = output.fragment {
            fragments
                .try_reserve(1)
                .map_err(|_| Error::ResourceExhausted)?;
            fragments.push(Frag {
                kid,
                copy: fragments.len(),
                col,
                y: offset,
                from: f.from,
                h: f.block_size,
                head: 0.0,
                foot: 0.0,
            });
        }
        let Some(next) = output.next else {
            return Ok(fragments);
        };
        token = Some(next);
        col = col.checked_add(1).ok_or(Error::ResourceExhausted)?;
        offset = 0.0;
    }
}
