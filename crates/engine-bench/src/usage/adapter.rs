//! Адаптер GPU для чтения памяти процесса: открывается один раз до первого замера.

use super::super::process::gpu::Memory;
use std::sync::{Arc, Mutex};

/// Адаптер GPU для чтения памяти, открытый один раз на процесс до первого замера:
/// фабрика DXGI и перебор адаптеров не попадают в окно кадра. Копия сэмплера и копия
/// границы раздельны, как и раньше; образцы идут по одному, поэтому замок без соперника.
pub(crate) struct Adapter {
    pub(super) sampler: Arc<Mutex<Option<Memory>>>,
    pub(super) boundary: Arc<Mutex<Option<Memory>>>,
    pub(super) error: Option<String>,
}
impl Adapter {
    pub(crate) fn open(luid: (i32, u32)) -> Self {
        let open = || Memory::open(luid.0, luid.1);
        let (sampler, error) = match open() {
            Ok(gpu) => (Some(gpu), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let boundary = sampler.as_ref().and_then(|_| open().ok());
        Self {
            sampler: Arc::new(Mutex::new(sampler)),
            boundary: Arc::new(Mutex::new(boundary)),
            error,
        }
    }
}
