//! Запись блока: прозрачная обёртка (`wrap`) снимает лист раскладки, начало
//! коробки, инертность prepaint и чернила краски, включая отложенную краску
//! строк прямо в запись блока (`Making`); `Record::seal` завершает её.

use super::lines::LineSink;
use super::narrow::Narrow;
use super::width::{Width, reusable_range};
use super::{Counters, FlowKey, Record, Replay, Source, placed};
use crate::hooks::LineCapture;
use gpui::{
    Bounds, InkSink, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint, ScaledPixels, point,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod wrap;
pub(super) use wrap::Recorder;

/// What the frame that makes a record measures, inline in the record: a
/// block being recorded costs one allocation, the record itself, and nothing
/// when the frame ends (Blink stores the result where the object keeps it,
/// with no input object beside it). `FlowReuse` owns the records being made
/// (the wrap in the GPUI tree knows only a number), so one that fails goes
/// with the frame's list.
pub(super) struct Making {
    layout: Cell<Option<LayoutId>>,
    origin: Cell<Option<Point<ScaledPixels>>>,
    /// Простые hitbox поддерева: пропуск теряет их только вне экрана.
    hits: Cell<Option<Bounds<ScaledPixels>>>,
    quiet: Cell<bool>,
    painted: Cell<bool>,
    sink: InkSink,
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
    lines: LineSink,
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
    pub(super) fn new(width: Width, settled: bool, unpainted: bool, tracks_flow: bool) -> Self {
        Self {
            layout: Cell::new(None),
            origin: Cell::new(None),
            hits: Cell::new(None),
            quiet: Cell::new(false),
            painted: Cell::new(false),
            sink: InkSink::default(),
            width,
            settled,
            unpainted,
            tracks_flow,
            border: Cell::new(0.0),
            lines: LineSink::default(),
            output: RefCell::new(None),
        }
    }

    /// The same inputs with nothing measured: of a record made from a
    /// sealed one (`Record::moved_to`).
    pub(super) fn sealed(&self) -> Self {
        Self::new(self.width, self.settled, self.unpainted, self.tracks_flow)
    }

    /// Строки абзацев блока (`lines::report`).
    pub(super) fn lines(&self) -> &LineSink {
        &self.lines
    }
}

impl gpui::InkTarget for Record {
    fn ink_sink(&self) -> &InkSink {
        &self.making.sink
    }
}

impl Record {
    /// The record of the block `source`, to be made by this frame.
    pub(super) fn new(
        source: Source,
        key: Rc<FlowKey>,
        (start, end): (Counters, Counters),
        making: Making,
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
            making,
        }
    }

    /// Завершить запись после краски кадра, на месте: `true`, если блок
    /// прошёл prepaint и краску инертно и либо был далеко от экрана
    /// `screen`, либо его вывод кадра `epoch` повторяем под маской корня
    /// `mask`. С ним — перенос текста блока: нет записи ни в одном потоке
    /// уже (`narrow`).
    pub(super) fn seal(
        &self,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> (bool, Option<Narrow>) {
        let m = &self.making;
        // A block whose text wrapped here wraps at any narrower flow, where
        // a record made while the width changes would be refused (`narrow`).
        let flow = self.key.flow_width();
        let wrapped = m.width == Width::Stretch
            && m.tracks_flow
            && m.lines.broken()
            && reusable_range(m.width, m.border.get(), flow, None).is_none();
        let narrow = flow
            .filter(|_| wrapped)
            .map(|flow| Narrow::of(&self.source, flow));
        // The frame's output goes with the frame whatever the record became.
        let output = m.output.take();
        (self.sealed(output, screen, mask, epoch).is_some(), narrow)
    }

    fn sealed(
        &self,
        output: Option<Box<Output>>,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> Option<()> {
        let m = &self.making;
        if !m.quiet.get() || !m.painted.get() || m.sink.tainted() {
            return None;
        }
        let origin = m.origin.get()?;
        let ink = match (m.sink.bounds(), m.hits.get()) {
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
            self.key.flow_width(),
            m.lines.slack(),
        );
        if !m.settled && resizable.is_none() {
            return None;
        }
        // The wrap wrote the leaf itself (`wrap`).
        self.snapshot.get()?;
        // Блок шире потока держит пол (`Width::Floor`), а не поток.
        let stretch = m.width == Width::Stretch
            && self
                .key
                .flow_width()
                .is_some_and(|flow| m.border.get() <= flow + 0.5);
        self.ink.set(ink);
        self.resizable.set(resizable);
        self.stretch.set(stretch);
        self.far.set(far);
        self.replay.replace(replay);
        Some(())
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
