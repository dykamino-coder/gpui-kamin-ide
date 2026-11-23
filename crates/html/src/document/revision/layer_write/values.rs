//! Atomic preflight of paint writes spanning root and deferred scene layers.
use super::{LayerHandle, Path, Write};
#[derive(Default)]
pub(super) struct Values(Vec<(LayerHandle, Vec<Write>)>);
pub(super) fn held(layer: &LayerHandle, id: u64) -> bool {
    u32::try_from(id)
        .is_ok_and(|o| layer.holds_box(o) || layer.holds_band(o) || layer.holds_group_root(o))
}
impl Values {
    pub(super) fn push(&mut self, layer: LayerHandle, write: Write) {
        let id = layer.content().id();
        if let Some((_, writes)) = self.0.iter_mut().find(|(l, _)| l.content().id() == id) {
            if writes.last().is_none_or(|w| w.0 != write.0) {
                writes.push(write)
            }
        } else {
            self.0.push((layer, vec![write]))
        }
    }
    pub(super) fn apply(
        self,
        paths: &[(Path, u64)],
        records: &mut Vec<(Path, u64)>,
    ) -> Option<usize> {
        // Preserve value-only writes for decorated owners that cannot be
        // recorded again. The layer itself validates this batch atomically.
        if let [(layer, writes)] = self.0.as_slice() {
            if layer.set_backgrounds(writes).is_some() {
                return Some(writes.len());
            }
        }
        // A failed value write always has a recorded owner to repaint.
        // Validate every layer before mutating the first: no partial revision.
        if !self
            .0
            .iter()
            .all(|(l, w)| w.iter().all(|w| held(l, u64::from(w.0))))
        {
            crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
            return None;
        }
        let mut values = 0;
        for (layer, writes) in self.0 {
            if layer.set_backgrounds(&writes).is_some() {
                values += writes.len();
            } else {
                records.extend(
                    paths
                        .iter()
                        .filter(|(_, id)| writes.iter().any(|w| u64::from(w.0) == *id))
                        .cloned(),
                );
            }
        }
        Some(values)
    }
}
