//! Ключ неизменяемого растра по идентичности: Arc удерживает память от повторного адреса.
use super::types::CpuRaster;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};
#[derive(Clone)]
pub(crate) struct RasterIdentity(pub(crate) Arc<CpuRaster>);
impl PartialEq for RasterIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RasterIdentity {}
impl Hash for RasterIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
