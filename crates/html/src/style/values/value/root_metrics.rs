//! Метрики корня документа для rem/rlh: кегль, высота строки и семейство
//! корневого элемента — поле состояния документа (`document::state`).

use crate::document::state::ValueCx;
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
            font_view: None,
        }
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
    let mut r = doc.root.borrow_mut();
    r.font_px = if font_px > 0.0 { font_px } else { 16.0 };
    r.line_px = if line_px > 0.0 { line_px } else { 19.2 };
}

pub fn root_font_px(vcx: ValueCx<'_>) -> f32 {
    with_root(vcx, |r| r.font_px)
}

pub fn root_line_px(vcx: ValueCx<'_>) -> f32 {
    with_root(vcx, |r| r.line_px)
}

/// Записать семейство корня (пустое — шрифт документа).
pub fn set_root_family(doc: &crate::document::state::DocState, family: &str) {
    doc.root.borrow_mut().family = family.to_string();
}

/// Одна корневая шрифтовая единица в точках: `cap`, `ch`, `ex` или `ic`
/// шрифта корня его кеглем — те же замеры, что у `cap`/`ch`/`ex`/`ic`
/// элемента (`resolve_em`).
pub(super) fn root_font_unit(unit: &str, vcx: ValueCx<'_>) -> f32 {
    let (family, size) = with_root(vcx, |r| (r.family.clone(), r.font_px));
    match unit {
        "cap" => crate::text::metrics::vmetrics_px(&family, size, vcx).2,
        "ch" => crate::text::metrics::ch_ex_px(&family, size, vcx).0,
        "ex" => crate::text::metrics::ch_ex_px(&family, size, vcx).1,
        _ => crate::text::metrics::ic_px(&family, size, vcx),
    }
}
