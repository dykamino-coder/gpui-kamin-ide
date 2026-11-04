//! Запись блока: прозрачная обёртка (`wrap`) снимает лист раскладки, начало
//! коробки, инертность prepaint и чернила краски, включая отложенную краску
//! строк; `Pending::finish` делает из них запись.

use super::lines::LineSink;
use super::width::{Width, reusable_range};
use super::{Counters, FlowKey, Record, Replay, placed};
use crate::dom::Element;
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
/// держит и обёртка в дереве GPUI до следующего кадра, поэтому элемент блока
/// лежит не здесь, а у `FlowReuse` и переходит в запись без копии.
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

impl Pending {
    pub(super) fn new(
        key: Rc<FlowKey>,
        start: Counters,
        end: Counters,
        width: Width,
        settled: bool,
        unpainted: bool,
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
            border: Cell::new(0.0),
            lines: Rc::default(),
            output: RefCell::new(None),
        }
    }

    /// Готовая запись блока `element`, если он прошёл prepaint и краску
    /// инертно и либо был далеко от экрана `screen`, либо его вывод кадра
    /// `epoch` повторяем под маской корня `mask`.
    pub(super) fn finish(
        &self,
        element: Box<Element>,
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
            element,
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
