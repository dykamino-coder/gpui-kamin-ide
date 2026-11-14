//! Запись блока: прозрачная обёртка (`wrap`) снимает лист раскладки, начало
//! коробки, инертность prepaint и чернила краски, включая отложенную краску
//! строк, в то, что кадр записи меряет (`Making`); `Record::seal` завершает
//! запись по этим мерам.

use super::lines::LineSink;
use super::narrow::Narrow;
use super::width::{Width, reusable_range};
use super::{Counters, FlowKey, Record, Replay, Source, placed};
use crate::hooks::LineCapture;
use gpui::{
    Bounds, InkSinkList, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint, ScaledPixels,
    point,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod wrap;
pub(super) use wrap::{Recorder, Skip};

/// A record in the making with what its frame measures. The build's list of
/// them (`FlowReuse::pending`) goes when the frame seals its records
/// (`FlowReuse::finish`): a kept record holds only its result, and the
/// measures cost nothing after the frame (Blink: the object keeps its
/// result, the recorder that measures it lives as long as the paint does).
/// The wrap in the GPUI tree knows its entry by number, so a record that
/// fails goes with the list.
pub(super) struct Pending {
    pub(super) record: Rc<Record>,
    pub(super) making: Making,
}

/// What the frame that makes a record measures.
pub(super) struct Making {
    layout: Cell<Option<LayoutId>>,
    origin: Cell<Option<Point<ScaledPixels>>>,
    /// Простые hitbox поддерева: пропуск теряет их только вне экрана.
    hits: Cell<Option<Bounds<ScaledPixels>>>,
    quiet: Cell<bool>,
    painted: Cell<bool>,
    /// The block's ink sink in the build's list (`FlowReuse::ink`).
    sink: u32,
    /// Зависимость раскладки блока от ширины потока (`width::independent`).
    width: Width,
    /// Ключ потока прошлой сборки тот же: запись годна хотя бы при нём.
    /// Иначе (ширина меняется) пишется лишь запись, годная и при другой.
    settled: bool,
    /// Корень блока — обёртка, которая ничего не рисует и держит содержимое
    /// у начального края (`table::StartWrapper`): лист снимается сквозь неё,
    /// а ширина блока — ширина содержимого, если оно шире обёртки.
    unpainted: bool,
    /// The block's border box is the flow width less fixed margins
    /// (`width::tracks_flow`): a wrap at this flow width is one at every
    /// narrower flow (`narrow`).
    tracks_flow: bool,
    /// Ширина рамки блока в этом кадре.
    border: Cell<f32>,
    /// Строки абзацев блока в этом кадре.
    pub(super) lines: LineSink,
    /// Вывод prepaint и краски этого кадра для повтора; только у блока
    /// рядом с экраном (`Output`).
    output: RefCell<Option<Box<Output>>>,
}

/// Вывод блока в кадре для его повтора в следующем. Повтор нужен лишь
/// блоку рядом с экраном: далёкий пропускается и без него (`far`), а
/// записываемых блоков в кадре тысячи, поэтому вывод — в куче и только у
/// ближних.
struct Output {
    prepaint: RetainedPrepaint,
    /// Вывод краски, отложенные строки и маска краски.
    paint: Option<(RetainedPaint, Rc<LineCapture>, Bounds<Pixels>)>,
}

impl Making {
    pub(super) fn new(
        width: Width,
        settled: bool,
        (unpainted, tracks_flow): (bool, bool),
        sink: u32,
    ) -> Self {
        Self {
            layout: Cell::new(None),
            origin: Cell::new(None),
            hits: Cell::new(None),
            quiet: Cell::new(false),
            painted: Cell::new(false),
            sink,
            width,
            settled,
            unpainted,
            tracks_flow,
            border: Cell::new(0.0),
            lines: LineSink::default(),
            output: RefCell::new(None),
        }
    }

    /// Ink and taint of the block's sink in `ink`; `None` once released.
    fn ink(&self, ink: &InkSinkList) -> Option<(Option<Bounds<ScaledPixels>>, bool)> {
        ink.with(self.sink, |sink| (sink.bounds(), sink.tainted()))
    }
}

impl Pending {
    /// Завершить запись после краски кадра, на месте: `true`, если блок
    /// прошёл prepaint и краску инертно (чернила — в стоке `ink`) и либо был
    /// далеко от экрана `screen`, либо его вывод кадра `epoch` повторяем под
    /// маской корня `mask`. С ним — перенос текста блока: нет записи ни в
    /// одном потоке уже (`narrow`).
    pub(super) fn seal(
        &self,
        ink: &InkSinkList,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> (bool, Option<Narrow>) {
        let (m, record) = (&self.making, &self.record);
        // A block whose text wrapped here wraps at any narrower flow, where
        // a record made while the width changes would be refused (`narrow`).
        let flow = record.key.flow_width();
        let wrapped = m.width == Width::Stretch
            && m.tracks_flow
            && m.lines.broken()
            && reusable_range(m.width, m.border.get(), flow, None).is_none();
        let narrow = flow
            .filter(|_| wrapped)
            .map(|flow| Narrow::of(&record.source, flow));
        // The frame's output goes with the frame whatever the record became.
        let output = m.output.take();
        let sealed = self.sealed(m.ink(ink), output, screen, (mask, epoch));
        (sealed.is_some(), narrow)
    }

    fn sealed(
        &self,
        ink: Option<(Option<Bounds<ScaledPixels>>, bool)>,
        output: Option<Box<Output>>,
        screen: Bounds<ScaledPixels>,
        (mask, epoch): (Bounds<Pixels>, u64),
    ) -> Option<()> {
        let (m, record) = (&self.making, &self.record);
        let (ink, tainted) = ink?;
        if !m.quiet.get() || !m.painted.get() || tainted {
            return None;
        }
        let origin = m.origin.get()?;
        let ink = match (ink, m.hits.get()) {
            (Some(a), Some(b)) => Some(a.union(&b)),
            (a, b) => a.or(b),
        };
        let ink = ink.map(|ink| Bounds {
            origin: point(
                ScaledPixels(ink.origin.x.0 - origin.x.0),
                ScaledPixels(ink.origin.y.0 - origin.y.0),
            ),
            size: ink.size,
        });
        let far = far(ink, origin, screen);
        // Далёкий блок на том же месте под той же маской вне экрана и без
        // повтора (`FlowReuse::validate` пропускает его по чернилам), а на
        // другом месте повтор не годен: его вывод не хранится.
        let replay = match output.map(|o| (o.prepaint, o.paint)) {
            Some((prepaint, Some((paint, deferred, at))))
                if !far && at == mask && paint.replayable() && deferred.complete() =>
            {
                Some(Box::new(Replay {
                    prepaint,
                    paint,
                    deferred,
                    origin,
                    mask,
                    epoch,
                }))
            }
            _ => None,
        };
        if !far && replay.is_none() {
            return None;
        }
        let resizable = reusable_range(
            m.width,
            m.border.get(),
            record.key.flow_width(),
            m.lines.slack(),
        );
        if !m.settled && resizable.is_none() {
            return None;
        }
        // The wrap wrote the leaf itself (`wrap`).
        record.snapshot.get()?;
        // Блок шире потока держит пол (`Width::Floor`), а не поток.
        let stretch = m.width == Width::Stretch
            && record
                .key
                .flow_width()
                .is_some_and(|flow| m.border.get() <= flow + 0.5);
        record.ink.set(ink);
        record.resizable.set(resizable);
        record.stretch.set(stretch);
        record.far.set(far);
        record.replay.replace(replay);
        Some(())
    }
}

impl Record {
    /// The record of the block `source`, to be made by this frame.
    pub(super) fn new(
        source: Source,
        key: Rc<FlowKey>,
        (start, end): (Counters, Counters),
    ) -> Self {
        Self {
            source,
            key,
            start,
            end,
            snapshot: Cell::new(None),
            ink: Cell::new(None),
            resizable: Cell::new(None),
            stretch: Cell::new(false),
            far: Cell::new(false),
            replay: RefCell::new(None),
            repaint: Cell::new(false),
            kept: Cell::new(false),
        }
    }
}

/// Блок с чернилами `ink` (относительно начала `origin`) был дальше экрана
/// `screen` по каждой оси. Сдвиг следующего кадра обычно меньше, а точный
/// ответ даёт `FlowReuse::validate`; ближний блок не пропустит ни один кадр,
/// и его запись только занимала бы память.
pub(super) fn far(
    ink: Option<Bounds<ScaledPixels>>,
    origin: Point<ScaledPixels>,
    screen: Bounds<ScaledPixels>,
) -> bool {
    let Some(ink) = ink else { return true };
    let s = screen.size;
    !placed(ink, origin, s.width.0.max(s.height.0)).intersects(&screen)
}
