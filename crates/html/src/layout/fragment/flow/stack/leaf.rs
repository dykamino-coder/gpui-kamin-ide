//! Исполнение leaf до завершения: число фрагментов определяется токенами, не запасом копий.

use crate::layout::fragment::flow::blocks::leaf::Leaf;
use crate::layout::fragment::token::{Error, Space, TokenStore};
use crate::layout::fragment::types::Frag;

pub(crate) fn fragment_leaf(
    leaf: Leaf,
    kid: usize,
    mut col: usize,
    mut offset: f32,
    target_at: &dyn Fn(usize) -> f32,
) -> Result<Vec<Frag>, Error> {
    let mut store = TokenStore::default();
    let mut token = None;
    let mut fragments = Vec::new();
    loop {
        let mut space = Space::pages();
        space.block_size = Some(target_at(col));
        let output = leaf.resume(space, offset, token, &mut store)?;
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
