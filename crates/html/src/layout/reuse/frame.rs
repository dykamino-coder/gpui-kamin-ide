//! Кадр потока: проверка заместителей перед prepaint и записи после краски.

use super::*;

impl FlowReuse {
    /// Окно перед prepaint дерева: запомнить экран и проверить заместителей.
    /// Заместитель годен, если его чернила на новом месте вне экрана либо
    /// его вывод прошлого кадра повторяем на том же месте под той же маской
    /// (тогда он помечается к повтору). `false` — кадр нужно собрать заново:
    /// негодные заместители отказаны (`Store::refused`) и будут собраны, а
    /// годные остаются. Отказанный лист имеет точный размер собранного блока,
    /// поэтому новая раскладка ставит остальные на те же места. Повторы с
    /// поддеревом раскладки проверяются тоже (`validate_replays`).
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
                        && slot.paint_holds()
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
                    // A repainted block may now paint its box where it painted
                    // nothing (a new background): its leaf counts as ink too.
                    let painted = match slot.repainted() {
                        true => {
                            let s = window.layout_bounds(id).size;
                            let leaf = Bounds {
                                origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
                                size: size(
                                    ScaledPixels(s.width.as_f32() * scale),
                                    ScaledPixels(s.height.as_f32() * scale),
                                ),
                            };
                            Some(record.ink.map_or(leaf, |i| i.union(&leaf)))
                        }
                        false => record.ink,
                    };
                    replay
                        || painted.is_none_or(|i| !placed(ink(i), origin, SNAP).intersects(&screen))
                });
            if !fits {
                refused.insert(record.source.id());
            }
        }
        let ok = refused.is_empty();
        // Отказанный блок сдвинулся: как и при отказе повтора с поддеревом,
        // он не повторяется на экране, пока ключ потока тот же (`Store::banned`).
        let mut banned = self.store.banned.take();
        banned.extend(refused.iter().copied());
        self.store.banned.replace(banned);
        // Повторная сборка кадра проверяется снова: отказы копятся до `finish`.
        let mut all = self.store.refused.take();
        all.extend(refused);
        self.store.refused.replace(all);
        self.validate_replays(window) && ok
    }

    /// После краски дерева: записи устоявшихся блоков этого кадра. Ожидающие
    /// записи отдают записи вход сравнения и освобождаются.
    pub(crate) fn finish(&self) {
        let (Some(screen), Some(mask)) = (self.screen.get(), self.mask.get()) else {
            return;
        };
        self.store.refused.take();
        // This frame compared the records with the patched document.
        self.store.patched.set(false);
        // A frame after no rebuild ends the restyles (`Store::restyled`).
        if !self.store.rebased.replace(false) {
            self.store.restyled.set(false);
        }
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
        let pending = self.pending.take();
        let tracked = pending.len() as u64;
        let mut unrecorded = self.store.unrecorded.take();
        // Each pending record goes as its record is made: the two do not
        // live side by side for the whole flow.
        for pending in pending {
            let node = pending.id();
            let finished = pending.finish(screen, mask, epoch);
            if let Some(narrow) = finished.narrow {
                self.narrowed.add(narrow);
            }
            if let Some(record) = finished.record {
                next.insert(node, Rc::new(record));
                unrecorded.remove(&node);
                recorded += 1;
            } else {
                unrecorded.insert(node);
            }
        }
        self.store.unrecorded.replace(unrecorded);
        self.learn_wraps();
        self.finish_replays(screen);
        // Кадр без записей и пропусков под тем же ключом оставляет прежние:
        // они подойдут следующему кадру, а их сброс стоил бы этому.
        if recorded > 0 || skipped > 0 {
            self.store.records.replace(next);
        }
        let mut stats = self.store.stats.get();
        stats.tracked += tracked;
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

pub(super) fn scaled(p: Point<Pixels>, scale: f32) -> Point<ScaledPixels> {
    point(
        ScaledPixels(p.x.as_f32() * scale),
        ScaledPixels(p.y.as_f32() * scale),
    )
}

/// Чернила записи, поставленные в начало `origin` и расширенные на `margin`.
pub(super) fn placed(
    ink: Bounds<ScaledPixels>,
    origin: Point<ScaledPixels>,
    margin: f32,
) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(
            ScaledPixels(ink.origin.x.0 + origin.x.0 - margin),
            ScaledPixels(ink.origin.y.0 + origin.y.0 - margin),
        ),
        size: size(
            ScaledPixels(ink.size.width.0 + 2.0 * margin),
            ScaledPixels(ink.size.height.0 + 2.0 * margin),
        ),
    }
}

/// Запас проверки экрана на округление краёв, в точках устройства.
pub(super) const SNAP: f32 = 2.0;
