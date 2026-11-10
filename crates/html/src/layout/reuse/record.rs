//! Запись блока между кадрами: вход сборки, лист, чернила и повтор вывода.

use super::*;

/// Запись блока: вход сборки и то, что нужно заместителю и проверке экрана.
pub(crate) struct Record {
    /// The block's node id.
    pub(super) id: u64,
    /// What a later build of the block is compared with (`source`); `None`:
    /// the block was compared at a rebuild of the document, by its document
    /// boxes (`Store::rebase`), and has not been built since.
    pub(super) source: RefCell<Option<Source>>,
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
    /// The block's paint-only values changed since its paint was recorded
    /// (`Store::same_tree`): the layout leaf holds, the recorded paint and
    /// ink do not (`FlowReuse::validate` tests the leaf's box as well).
    pub(super) repaint: Cell<bool>,
}

impl Record {
    /// This record compared with `source`, the block of a restyled tree.
    /// `repaint`: the block differs in paint-only values, so the recorded
    /// output is dropped (Blink keeps the layout result of a
    /// paint-invalidated object and drops its display items).
    pub(super) fn moved_to(&self, source: Option<Source>, repaint: bool) -> Self {
        Self {
            id: self.id,
            source: RefCell::new(source),
            key: self.key.clone(),
            start: self.start,
            end: self.end,
            snapshot: self.snapshot,
            ink: self.ink,
            resizable: self.resizable,
            stretch: self.stretch,
            far: self.far,
            replay: RefCell::new(match repaint {
                true => None,
                false => self.replay.take(),
            }),
            repaint: Cell::new(self.repaint.get() || repaint),
        }
    }

    /// This record compared at a rebuild of the document (`Store::rebase`),
    /// in place: it drops its source, and with `repaint` its output.
    pub(super) fn rebased(&self, repaint: bool) {
        self.source.take();
        if repaint {
            self.replay.take();
            self.repaint.set(true);
        }
    }

    /// Запись годна в потоке ключа `key` другой ширины.
    pub(super) fn fits_width(&self, key: &FlowKey) -> bool {
        match self.resizable {
            Some((lo, hi)) if lo <= 0.0 && hi.is_infinite() => true,
            Some((lo, hi)) => key.flow_width().is_some_and(|w| lo <= w && w <= hi),
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
