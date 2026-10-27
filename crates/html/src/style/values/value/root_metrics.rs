//! Метрики корня документа для rem/rlh: кегль, высота строки и семейство
//! корневого элемента — поле состояния документа (`document::state`).

use crate::document::state::ValueCx;
use crate::font_facts::FontRequest;
/// Кегль КОРНЕВОГО элемента — база единицы `rem` (css-values-4 §6.1.4 —
/// «the computed value of the em unit on the root element»). Постоянные 16
/// врали на любом документе, где корню задан свой кегль: `:root { font-size:
/// 25% }` делает `25rem` сотней точек, а у нас выходило 400
/// (`percentage-rem-low`, снимок 500×500 против 125×125).
///
/// Поле документа (читается через `ValueCx`): корень разбирается ПЕРВЫМ в
/// порядке документа — к моменту, когда разбирается объявление любого
/// потомка, значение уже верное. У рамки свой документ и свой корень.
#[derive(Clone, Debug)]
pub struct RootMetrics {
    pub(crate) font_px: f32,
    /// Высота строки корня: база единицы `rlh` (§6.1.4).
    pub(crate) line_px: f32,
    /// Семейство корня: база `rcap`/`rch`/`rex`/`ric` (css-values-4 §6.1.2).
    pub(crate) family: String,
    /// Computed cap/ch/ex/ic lengths; numeric resolution never queries fonts.
    units: [f32; 4],
    /// Кегль корня в единицах окна (`html { font-size: 100vw }`): точек на
    /// разборе ещё нет, окно знает только сборщик дерева. Тогда `rem` доживает
    /// той же единицей окна, умноженной на долю (`vh-em-inherit`: `1rem` =
    /// `100vw`, а не начальные 16). Флаг — `vh` вместо `vw`.
    pub(crate) font_view: Option<(bool, f32)>,
}

/// Документ без своего кегля на корне считает `rem` от начальных 16 точек.
impl Default for RootMetrics {
    fn default() -> Self {
        RootMetrics {
            font_px: 16.0,
            line_px: 19.2,
            family: String::new(),
            units: [11.2, 8.0, 8.0, 16.0],
            font_view: None,
        }
    }
}

impl RootMetrics {
    pub(crate) fn resolve_units(&mut self, vcx: ValueCx<'_>) {
        let family = self.family.clone();
        self.resolve_font_units(FontRequest::new(&family, self.font_px), vcx);
    }

    pub(crate) fn resolve_font_units(&mut self, request: FontRequest<'_>, vcx: ValueCx<'_>) {
        let size = request.size_px;
        let Ok(facts) = vcx
            .doc
            .font_provider()
            .metrics(request, &vcx.doc.font_snapshot())
        else {
            return;
        };
        self.units = [
            facts
                .cap_height
                .or(facts.ascent)
                .map_or(0.7 * size, |m| m.px),
            facts.zero_advance.map_or(0.5 * size, |m| m.px),
            facts.x_height.map_or(0.5 * size, |m| m.px),
            facts.ideographic_advance.map_or(size, |m| m.px),
        ];
    }
}

fn with_root<R>(vcx: ValueCx<'_>, f: impl FnOnce(&RootMetrics) -> R) -> R {
    f(&vcx.doc.root.borrow())
}

/// Записать кегль корня, заданный единицей окна (`None` — кегль в точках).
pub fn set_root_font_view(doc: &crate::document::state::DocState, view: Option<(bool, f32)>) {
    doc.root.borrow_mut().font_view = view;
}

pub(super) fn root_font_view(vcx: ValueCx<'_>) -> Option<(bool, f32)> {
    with_root(vcx, |r| r.font_view)
}

/// Записать корневые метрики. Зовётся разбором дерева на элементе `html`
/// (`dom::walk`).
pub fn set_root_metrics(doc: &crate::document::state::DocState, font_px: f32, line_px: f32) {
    {
        let mut r = doc.root.borrow_mut();
        r.font_px = if font_px > 0.0 { font_px } else { 16.0 };
        r.line_px = if line_px > 0.0 { line_px } else { 19.2 };
        r.resolve_units(doc.values());
    }
}

/// Высота строки `line-height: normal` корня (`family` кеглем `font`) для
/// `set_root_metrics`: метрика шрифта, без неё — `1.2 * font`.
pub fn root_normal_line_px(family: &str, font: f32, vcx: ValueCx<'_>) -> f32 {
    crate::style::font_metrics::normal_height(family, font, vcx)
}

pub fn root_font_px(vcx: ValueCx<'_>) -> f32 {
    with_root(vcx, |r| r.font_px)
}

pub fn root_line_px(vcx: ValueCx<'_>) -> f32 {
    with_root(vcx, |r| r.line_px)
}

/// Записать семейство корня (пустое — шрифт документа).
pub fn set_root_family(doc: &crate::document::state::DocState, family: &str) {
    {
        let mut r = doc.root.borrow_mut();
        r.family.clear();
        r.family.push_str(family);
        r.resolve_units(doc.values());
    }
}

/// Одна корневая шрифтовая единица в точках: `cap`, `ch`, `ex` или `ic`
/// шрифта корня его кеглем — те же замеры, что у `cap`/`ch`/`ex`/`ic`
/// элемента (`resolve_em`).
pub(super) fn root_font_unit(unit: &str, vcx: ValueCx<'_>) -> f32 {
    with_root(vcx, |r| {
        r.units[match unit {
            "cap" => 0,
            "ch" => 1,
            "ex" => 2,
            _ => 3,
        }]
    })
}
