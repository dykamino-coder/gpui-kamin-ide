//! Кадр потока: проверка заместителей перед prepaint и записи после краски.

use super::*;

impl FlowReuse {
    /// Окно перед prepaint дерева: запомнить экран и проверить заместителей.
    /// Заместитель годен, если его чернила на новом месте вне экрана либо
    /// его вывод прошлого кадра повторяем на том же месте под той же маской
    /// (тогда он помечается к повтору). `false` — кадр нужно собрать заново:
    /// негодные заместители отказаны (`Store::refused`) и будут собраны, а
    /// годные остаются. Отказанный лист имеет точный размер собранного блока,
    /// поэтому новая раскладка ставит остальные на те же места.
    pub(crate) fn validate(&self, window: &mut Window) -> bool {
        let scale = window.scale_factor();
        let mask = window.content_mask().bounds;
        let screen = Bounds {
            origin: scaled(mask.origin, scale),
            size: size(
                ScaledPixels(mask.size.width.as_f32() * scale),
                ScaledPixels(mask.size.height.as_f32() * scale),
            ),
        };
        self.screen.set(Some(screen));
        self.mask.set(Some(mask));
        let root = window.element_offset() == point(px(0.0), px(0.0));
        let replayable = root && window.retained_root_environment();
        let epoch = self.store.epoch.get();
        let mut refused = std::collections::HashSet::new();
        for slot in self.placeholders.borrow().iter() {
            let record = slot.record();
            let fits = root
                && slot.layout().is_some_and(|id| {
                    let origin = scaled(window.layout_origin_unrounded(id), scale);
                    let replay = replayable
                        && !slot.stretched()
                        && record.replay.borrow().as_ref().is_some_and(|r| {
                            r.follows(epoch)
                                && r.origin == origin
                                && r.mask == mask
                                && window.retained_replayable(&r.prepaint, &r.paint)
                        });
                    slot.set_replay(replay.then_some(epoch));
                    // Чернила растянутого листа — до любого правого края.
                    let ink = |ink: Bounds<ScaledPixels>| match slot.stretched() {
                        true => Bounds {
                            size: size(ScaledPixels(f32::MAX / 4.0), ink.size.height),
                            ..ink
                        },
                        false => ink,
                    };
                    replay
                        || record
                            .ink
                            .is_none_or(|i| !placed(ink(i), origin, SNAP).intersects(&screen))
                });
            if !fits {
                refused.insert(record.source.id());
            }
        }
        let ok = refused.is_empty();
        self.store.refused.borrow_mut().extend(refused);
        ok
    }
    /// После краски дерева: записи устоявшихся блоков этого кадра. Ожидающие
    /// записи отдают записи вход сравнения и освобождаются.
    pub(crate) fn finish(&self) {
        let (Some(screen), Some(mask)) = (self.screen.get(), self.mask.get()) else {
            return;
        };
        self.store.refused.take();
        let mut next = self.next.take();
        let skipped = next.len() as u64;
        let replayed = self
            .placeholders
            .borrow()
            .iter()
            .filter(|slot| slot.replayed())
            .count() as u64;
        let mut recorded = 0;
        let epoch = self.store.epoch.get();
        for (pending, source) in self.pending.take() {
            if let Some(record) = pending.finish(source, screen, mask, epoch) {
                next.insert(record.source.id(), Rc::new(record));
                recorded += 1;
            }
        }
        // Кадр без записей и пропусков под тем же ключом оставляет прежние:
        // они подойдут следующему кадру, а их сброс стоил бы этому.
        if recorded > 0 || skipped > 0 {
            self.store.records.replace(next);
        }
        let mut stats = self.store.stats.get();
        stats.recorded += recorded;
        stats.skipped += skipped;
        stats.replayed += replayed;
        self.store.stats.set(stats);
    }

    /// Кадр собран заново без заместителей (`validate`).
    pub(crate) fn note_rebuild(&self) {
        let mut stats = self.store.stats.get();
        stats.rebuilt_frames += 1;
        self.store.stats.set(stats);
    }
}
