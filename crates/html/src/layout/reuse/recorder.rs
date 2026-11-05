//! Запись блока: прозрачная обёртка (`wrap`) снимает лист раскладки, начало
//! коробки, инертность prepaint и чернила краски, включая отложенную краску
//! строк; `Pending::finish` делает из них запись.

use super::lines::LineSink;
use super::width::{Width, reusable_range};
use super::{Counters, FlowKey, Record, Replay, Source, placed};
use crate::hooks::LineCapture;
use gpui::{
    Bounds, InkSink, LayoutId, LayoutSnapshot, Pixels, Point, RetainedPaint, RetainedPrepaint,
    ScaledPixels, point,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod wrap;
pub(super) use wrap::Recorder;

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`). Её
/// держит и обёртка в дереве GPUI до следующего кадра, поэтому вход сравнения
/// блока лежит не здесь, а у `FlowReuse` и переходит в запись без копии.
pub(super) struct Pending {
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    layout: Cell<Option<LayoutId>>,
    snapshot: RefCell<Option<LayoutSnapshot>>,
    origin: Cell<Option<Point<ScaledPixels>>>,
    /// Простые hitbox поддерева: пропуск теряет их только вне экрана.
    hits: Cell<Option<Bounds<ScaledPixels>>>,
    quiet: Cell<bool>,
    painted: Cell<bool>,
    sink: Rc<InkSink>,
    /// Зависимость раскладки блока от ширины потока (`width::independent`).
    width: Width,
    /// Ключ потока прошлой сборки тот же: запись годна хотя бы при нём.
    /// Иначе (ширина меняется) пишется лишь запись, годная и при другой.
    settled: bool,
    /// Корень блока — обёртка, которая ничего не рисует и держит содержимое
    /// у начального края (	able::StartWrapper): лист снимается сквозь неё,
    /// а ширина блока — ширина содержимого, если оно шире обёртки.
    unpainted: bool,
    /// The block's border box is the flow width less fixed margins
    /// (`width::tracks_flow`): a wrap at this flow width is one at every
    /// narrower flow (`Narrow`).
    tracks_flow: bool,
    /// Ширина рамки блока в этом кадре.
    border: Cell<f32>,
    /// Строки абзацев блока в этом кадре.
    lines: Rc<LineSink>,
    /// Вывод prepaint и краски этого кадра для повтора; только у блока
    /// рядом с экраном (Output).
    output: RefCell<Option<Box<Output>>>,
}

/// Вывод блока в кадре для его повтора в следующем. Повтор нужен лишь
/// блоку рядом с экраном: далёкий пропускается и без него (ar), а
/// записываемых блоков в кадре тысячи, поэтому вывод — в куче и только у
/// ближних.
struct Output {
    prepaint: RetainedPrepaint,
    /// Вывод краски, отложенные строки и маска краски.
    paint: Option<(RetainedPaint, Rc<LineCapture>, Bounds<Pixels>)>,
}

/// What a pending record became after the frame (`Pending::finish`).
pub(super) struct Finished {
    pub(super) record: Option<Record>,
    /// The block's text wrapped: no record at any narrower flow (`Narrow`).
    pub(super) narrow: Option<Narrow>,
}

/// A block of text alone whose border box tracks the flow and whose text
/// wrapped in a flow of width `flow`. Text set from the start edge that does
/// not fit on one line does not fit in less room (CSS Text 3 §5: a line breaks
/// only where its content overflows it), so in a flow no wider and with the
/// same text the block wraps again, and a record made while the flow width
/// changes would be refused (`width::reusable_range`): such a build does not
/// wrap it for recording at all.
///
/// The entry knows its text by the identity of the block's child list (a
/// changed text is a new list), through a mark that does not keep the list:
/// holding the block's subtree or styles here kept a previous build's styles
/// of every wrapped block alive. Not wrapping a block for recording never
/// changes its layout or paint, only whether a later frame may skip it, so a
/// style change under the same text costs at most that block's record.
#[derive(Clone)]
pub(super) struct Narrow {
    id: u64,
    children: crate::dom::ChildrenMark,
    identity: usize,
    flow: f32,
}

impl Narrow {
    fn of(source: &Source, flow: f32) -> Self {
        let children = source.children();
        Self {
            id: source.id(),
            children: children.mark(),
            identity: children.identity(),
            flow,
        }
    }

    /// The block `e` in a flow of width `flow` wraps as it did here.
    pub(super) fn holds(&self, e: &crate::dom::Element, flow: Option<f32>) -> bool {
        flow.is_some_and(|flow| flow <= self.flow)
            && e.node_id == self.id
            && self.children.is_alive()
            && e.children.identity() == self.identity
    }

    pub(super) fn id(&self) -> u64 {
        self.id
    }
}

impl Pending {
    pub(super) fn new(
        key: Rc<FlowKey>,
        start: Counters,
        end: Counters,
        width: Width,
        settled: bool,
        unpainted: bool,
        tracks_flow: bool,
    ) -> Self {
        Self {
            key,
            start,
            end,
            layout: Cell::new(None),
            snapshot: RefCell::new(None),
            origin: Cell::new(None),
            hits: Cell::new(None),
            quiet: Cell::new(false),
            painted: Cell::new(false),
            sink: Rc::default(),
            width,
            settled,
            unpainted,
            tracks_flow,
            border: Cell::new(0.0),
            lines: Rc::default(),
            output: RefCell::new(None),
        }
    }

    /// Готовая запись блока `source`, если он прошёл prepaint и краску
    /// инертно и либо был далеко от экрана `screen`, либо его вывод кадра
    /// `epoch` повторяем под маской корня `mask`.
    pub(super) fn finish(
        &self,
        source: Source,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> Finished {
        // A block whose text wrapped here wraps at any narrower flow, where
        // a record made while the width changes would be refused (`Narrow`).
        let flow = self.key.constraints.avail_w;
        let wrapped = self.width == Width::Stretch
            && self.tracks_flow
            && self.lines.broken()
            && reusable_range(self.width, self.border.get(), flow, None).is_none();
        let narrow = flow
            .filter(|_| wrapped)
            .map(|flow| Narrow::of(&source, flow));
        Finished {
            record: self.record(source, screen, mask, epoch),
            narrow,
        }
    }

    fn record(
        &self,
        source: Source,
        screen: Bounds<ScaledPixels>,
        mask: Bounds<Pixels>,
        epoch: u64,
    ) -> Option<Record> {
        if !self.quiet.get() || !self.painted.get() || self.sink.tainted() {
            return None;
        }
        let origin = self.origin.get()?;
        let ink = match (self.sink.bounds(), self.hits.get()) {
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
        let output = self.output.take().map(|o| (o.prepaint, o.paint));
        let replay = match output {
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
            self.width,
            self.border.get(),
            self.key.constraints.avail_w,
            self.lines.slack(),
        );
        if !self.settled && resizable.is_none() {
            return None;
        }
        let snapshot = self.snapshot.take()?;
        Some(Record {
            source,
            key: self.key.clone(),
            start: self.start,
            end: self.end,
            snapshot,
            ink,
            resizable,
            // Блок шире потока держит пол (`Width::Floor`), а не поток.
            stretch: self.width == Width::Stretch
                && self
                    .key
                    .constraints
                    .avail_w
                    .is_some_and(|flow| self.border.get() <= flow + 0.5),
            far,
            replay: RefCell::new(replay),
        })
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
