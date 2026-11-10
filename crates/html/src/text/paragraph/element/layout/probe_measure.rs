//! The measure function of a paragraph's layout leaf (`gpui::CompactMeasure`):
//! its probe, built once or rebuilt for every measure, and the plain form a
//! far block's kept leaf shrinks to (`layout::reuse::far`).

use super::measured_layout;
use super::normal_static::NormalStatic;
use super::plain_probe::{PlainProbe, Settings};
use crate::text::paragraph::Paragraph;
use crate::text::paragraph::atom_fit::AtomFit;
use gpui::{App, AvailableSpace, MeasuredBox, MeasuredContent, Pixels, Size, Window};
use std::cell::RefCell;
use std::rc::Rc;

/// Щуп замера: собран один раз либо пересобирается на каждый вызов.
#[allow(clippy::large_enum_variant)] // вариант выбран на всю жизнь замера
pub(super) enum Probe<F> {
    Built(Paragraph),
    /// A built probe kept once (`compact`): the next keep compacts it.
    Kept(Paragraph),
    Rebuild(Settings, F),
    /// Moved out by a compaction that is being made.
    Spent,
}

/// The measure of a paragraph leaf: one allocation with the probe inline,
/// like the closure it replaces.
pub(super) struct ProbeMeasure<F> {
    source: Probe<F>,
    atom_fit: Rc<RefCell<AtomFit>>,
    normal_static: NormalStatic,
}

impl<F> ProbeMeasure<F> {
    pub(super) fn new(
        source: Probe<F>,
        atom_fit: Rc<RefCell<AtomFit>>,
        normal_static: NormalStatic,
    ) -> Self {
        Self {
            source,
            atom_fit,
            normal_static,
        }
    }
}

/// Measure `probe` as the leaf's request built it.
fn measure(
    probe: &mut Paragraph,
    atom_fit: &Rc<RefCell<AtomFit>>,
    normal_static: &NormalStatic,
    known: Size<Option<Pixels>>,
    available: Size<AvailableSpace>,
    window: &mut Window,
    cx: &mut App,
) -> MeasuredContent {
    // Заданная ширина сильнее доступной: раскладка уже решила, в какую
    // коробку абзац ставится, и переносы считаются по ней.
    measured_layout(
        probe,
        atom_fit.clone(),
        known,
        available,
        normal_static,
        window,
        cx,
    )
}

impl<F: FnMut(Paragraph) -> Paragraph + 'static> gpui::CompactMeasure for ProbeMeasure<F> {
    type Compact = PlainMeasure;

    fn measure(
        &mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        _: MeasuredBox,
        window: &mut Window,
        cx: &mut App,
    ) -> MeasuredContent {
        let (fit, normal) = (&self.atom_fit, &self.normal_static);
        match &mut self.source {
            Probe::Built(probe) | Probe::Kept(probe) => {
                measure(probe, fit, normal, known, available, window, cx)
            }
            Probe::Rebuild(settings, make_probe) => {
                let mut probe = make_probe(settings.probe());
                measure(&mut probe, fit, normal, known, available, window, cx)
            }
            Probe::Spent => unreachable!("a paragraph measure is compacted only between measures"),
        }
    }

    /// The plain form of a built probe (`PlainProbe`) at its second keep; a
    /// rebuilt or not plain probe keeps its measure. The first keep, in the
    /// frame that records the far block, only marks the probe: that frame
    /// allocates nothing for it, and the next pass measures the built probe
    /// as its request would, then compacts it after its paint.
    fn compact(&mut self) -> Option<PlainMeasure> {
        if matches!(self.source, Probe::Built(_)) {
            let Probe::Built(probe) = std::mem::replace(&mut self.source, Probe::Spent) else {
                unreachable!()
            };
            self.source = Probe::Kept(probe);
            return None;
        }
        let Probe::Kept(probe) = &self.source else {
            return None;
        };
        if !PlainProbe::plain(probe) {
            return None;
        }
        let Probe::Kept(probe) = std::mem::replace(&mut self.source, Probe::Spent) else {
            unreachable!()
        };
        Some(PlainMeasure {
            plain: PlainProbe::of(probe),
            pass: None,
            atom_fit: self.atom_fit.clone(),
            normal_static: self.normal_static.clone(),
        })
    }
}

/// The probe of one pass and the break opportunities its first measure seeds.
type PassProbe = (Paragraph, Option<Box<[u32]>>);

/// The measure of a far block's kept paragraph leaf. The probe of a pass is
/// made from the plain form when the leaf is claimed (`prepare`, at request
/// time, where a build makes its probe), serves the measures of that pass and
/// is dropped when the leaf is kept again after it (`compact`), so between
/// frames only the plain form lives.
pub(super) struct PlainMeasure {
    plain: PlainProbe,
    /// Boxed: the kept leaf's measure box stays the size of the plain form
    /// between passes.
    pass: Option<Box<PassProbe>>,
    atom_fit: Rc<RefCell<AtomFit>>,
    normal_static: NormalStatic,
}

impl gpui::CompactMeasure for PlainMeasure {
    type Compact = Self;

    fn measure(
        &mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        _: MeasuredBox,
        window: &mut Window,
        cx: &mut App,
    ) -> MeasuredContent {
        let plain = &self.plain;
        let (probe, stops) = &mut **self.pass.get_or_insert_with(|| Box::new(plain.probe()));
        if let Some(stops) = stops.take() {
            probe.seed_stops(stops);
        }
        measure(
            probe,
            &self.atom_fit,
            &self.normal_static,
            known,
            available,
            window,
            cx,
        )
    }

    /// Kept again after its pass: the pass probe goes, the plain form stays.
    fn compact(&mut self) -> Option<Self> {
        self.pass = None;
        None
    }

    fn prepare(&mut self) {
        if self.pass.is_none() {
            self.pass = Some(Box::new(self.plain.probe()));
        }
    }
}
