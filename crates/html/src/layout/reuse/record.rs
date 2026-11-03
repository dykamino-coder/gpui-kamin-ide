//! Запись блока между кадрами: вход сборки, лист, чернила и повтор вывода.

use super::*;

/// Запись блока: вход сборки и то, что нужно заместителю и проверке экрана.
pub(crate) struct Record {
    /// Вход сборки блока; в куче, чтобы переходить из ожидания без копии.
    pub(super) element: Box<Element>,
    pub(super) key: Rc<FlowKey>,
    pub(super) start: Counters,
    pub(super) end: Counters,
    pub(super) snapshot: LayoutSnapshot,
    /// Чернила относительно точного начала коробки; `None` — ничего не рисовал.
    pub(super) ink: Option<Bounds<ScaledPixels>>,
    /// Ширины потока, при которых годна запись (`width::reusable_range`);
    /// `None` — только при ширине записи.
    pub(super) resizable: Option<(f32, f32)>,
    /// Ширина листа — растяжение потока (`width::Width::Stretch`): при
    /// другой ширине краска записи не годна.
    pub(super) stretch: bool,
    /// Блок был далеко от экрана (`recorder::far`): его пропуск вероятен и
    /// без повтора краски.
    pub(super) far: bool,
    /// Вывод prepaint и краски блока в последнем кадре, где он рисовался.
    /// Только у ближнего блока: далёкий на том же месте под той же маской
    /// и так вне экрана, и его пропуску повтор не нужен (`far`).
    pub(super) replay: RefCell<Option<Box<Replay>>>,
}

impl Record {
    /// Запись годна в потоке ключа `key` другой ширины.
    pub(super) fn fits_width(&self, key: &FlowKey) -> bool {
        match self.resizable {
            Some((lo, hi)) if lo <= 0.0 && hi.is_infinite() => true,
            Some((lo, hi)) => key.constraints.avail_w.is_some_and(|w| lo <= w && w <= hi),
            None => false,
        }
    }

    /// Повтор записан в последнем кадре prepaint документа до этого.
    pub(super) fn current(&self, flow: &FlowReuse) -> bool {
        let epoch = flow.store.epoch.get() - u64::from(flow.drawn);
        self.replay
            .borrow()
            .as_ref()
            .is_some_and(|r| r.epoch == epoch)
    }
}
