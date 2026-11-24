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
        // Past the screen by the interest margin of a document under its root
        // scroller: a scrolled replay may show what is recorded there.
        let margin = window.scroll_interest_margin();
        let mask = window.content_mask().bounds;
        let screen_mask = mask.dilate(margin);
        let screen = Bounds {
            origin: scaled(screen_mask.origin, scale),
            size: size(
                ScaledPixels(screen_mask.size.width.as_f32() * scale),
                ScaledPixels(screen_mask.size.height.as_f32() * scale),
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
                    // A movable output replays at a new place too, when what
                    // it shows there was shown, and so recorded, where it was.
                    let from = record.replay.borrow().as_ref().and_then(|r| {
                        (replayable
                            && slot.paint_holds()
                            && r.follows(&self.store, epoch)
                            && (r.origin == origin
                                || r.movable
                                    && raster_phase_matches(r.origin, origin)
                                    && recorded(record.ink.get(), r.origin, origin, screen))
                            && r.mask == mask
                            && window.retained_replayable(&r.prepaint, &r.paint))
                        .then_some(r.origin)
                    });
                    if let Some(from) = from {
                        let offset = point(
                            px((origin.x.0 - from.x.0) / scale),
                            px((origin.y.0 - from.y.0) / scale),
                        );
                        slot.place(origin, offset);
                    }
                    let replay = from.is_some();
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
                    let s = window.layout_bounds(id).size;
                    let leaf = Bounds {
                        origin: point(ScaledPixels(0.0), ScaledPixels(0.0)),
                        size: size(
                            ScaledPixels(s.width.as_f32() * scale),
                            ScaledPixels(s.height.as_f32() * scale),
                        ),
                    };
                    let repainted = record.ink.get().map_or(leaf, |i| i.union(&leaf));
                    let off =
                        |i: Bounds<ScaledPixels>| !placed(ink(i), origin, SNAP).intersects(&screen);
                    // Skipped and off the screen even repainted: a paint-only
                    // edit of a box inside changes no pixel of this frame
                    // (`Store::unpainted`).
                    slot.set_clear(!replay && off(repainted));
                    let painted = match slot.repainted() {
                        true => Some(repainted),
                        false => record.ink.get(),
                    };
                    replay || painted.is_none_or(off)
                });
            if !fits {
                refused.insert(record.source.id());
            }
        }
        let placed = refused.len();
        // Far blocks laid out at their new place must still be far (`far`).
        refused.extend(self.validate_far(window, root));
        let ok = refused.is_empty();
        if !ok {
            crate::why::note("layout_retry.flow_refused", || {
                let far = refused.len() - placed;
                format!(
                    "{placed} of {} stand-ins on screen without a replay, {far} far blocks no longer far",
                    self.placeholders.borrow().len()
                )
            });
        }
        // Отказанный блок сдвинулся: как и при отказе повтора с поддеревом,
        // он не повторяется на экране, пока ключ потока тот же (`Store::banned`).
        let mut banned = self.store.banned.take();
        banned.extend(refused.iter().copied());
        self.store.banned.replace(banned);
        // Повторная сборка кадра проверяется снова: отказы копятся до `finish`.
        let mut all = self.store.refused.take();
        all.extend(refused);
        self.store.refused.set(all);
        self.validate_replays(window) && ok
    }

    /// После краски дерева: записи устоявшихся блоков этого кадра завершаются
    /// на месте (`Record::seal`); незавершённые освобождаются.
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
            self.store.restyled_blocks.take();
        }
        let mut next = self.next.take();
        let skipped = next.len() as u64;
        let replayed = self
            .placeholders
            .borrow()
            .iter()
            .filter(|slot| slot.replayed())
            .count() as u64;
        // Blocks this frame did not paint: the layer holds no box of theirs.
        let slots = self.placeholders.borrow();
        let unpainted = slots.iter().filter(|s| !s.replayed() && s.clear());
        self.store
            .set_unpainted(unpainted.map(|s| s.record().source.id()).collect());
        drop(slots);
        let mut recorded = 0;
        let epoch = self.store.epoch.get();
        let pending = self.pending.take();
        let tracked = pending.len() as u64;
        let mut unrecorded = self.store.unrecorded.take();
        // Each record is sealed in place and kept as it is: nothing is
        // allocated per block here, and the map grows once.
        next.reserve(pending.len());
        for pending in pending {
            let node = pending.record.source.id();
            let (sealed, narrow) = pending.seal(&self.ink, screen, mask, epoch);
            let record = pending.record;
            if let Some(narrow) = narrow {
                self.narrowed.add(narrow);
            }
            if sealed {
                next.insert(node, record);
                unrecorded.remove(&node);
                recorded += 1;
            } else {
                unrecorded.insert(node);
            }
        }
        self.store.unrecorded.replace(unrecorded);
        self.learn_wraps();
        self.finish_replays(screen);
        self.finish_contents();
        self.finish_far();
        // The frame's ink measures are read: the sinks go with the frame.
        self.ink.release();
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

/// What the ink `ink` of a block moved from `from` to `to` shows on `screen`
/// (the window mask) was shown, and so recorded, at `from`: the window clips
/// what it paints, so a moved copy cannot bring in what lay outside.
fn recorded(
    ink: Option<Bounds<ScaledPixels>>,
    from: Point<ScaledPixels>,
    to: Point<ScaledPixels>,
    screen: Bounds<ScaledPixels>,
) -> bool {
    ink.is_none_or(|ink| {
        let shown = screen.intersect(&placed(ink, to, 0.0));
        let was = screen.intersect(&placed(ink, from, 0.0));
        let was = placed(was, point(to.x - from.x, to.y - from.y), 0.0);
        shown.size.width.0 <= 0.0 || shown.size.height.0 <= 0.0 || was.intersect(&shown) == shown
    })
}

/// Replaying rasterized output must preserve its device-pixel phase.
/// Widen before subtraction so f32 cancellation cannot hide a phase change.
pub(super) fn raster_phase_matches(from: Point<ScaledPixels>, to: Point<ScaledPixels>) -> bool {
    [
        f64::from(to.x.0) - f64::from(from.x.0),
        f64::from(to.y.0) - f64::from(from.y.0),
    ]
    .into_iter()
    .all(|delta| delta.is_finite() && delta.fract() == 0.0)
}
