//! Метрики корня документа для rem/rlh: кегль и высота строки корневого элемента (поток-локальные).

thread_local! {
    /// Кегль КОРНЕВОГО элемента: база единицы `rem` (css-values-4 §6.1.4 —
    /// «the computed value of the em unit on the root element»). Постоянные
    /// 16 врали на любом документе, где корню задан свой кегль:
    /// `:root { font-size: 25% }` делает `25rem` сотней точек, а у нас
    /// выходило 400 (`percentage-rem-low`, снимок 500×500 против 125×125).
    ///
    /// Слот, а не поле: разбор длины живёт далеко от дерева, а корень всё
    /// равно разбирается ПЕРВЫМ в порядке документа — к моменту, когда
    /// разбирается объявление любого потомка, значение уже верное.
    static ROOT_FONT_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(16.0) };
    /// Высота строки корня: база единицы `rlh` (§6.1.4).
    static ROOT_LINE_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(19.2) };
    /// Кегль корня в единицах окна (`html { font-size: 100vw }`): точек на
    /// разборе ещё нет, окно знает только сборщик дерева. Тогда `rem` доживает
    /// той же единицей окна, умноженной на долю (`vh-em-inherit`: `1rem` =
    /// `100vw`, а не начальные 16). Флаг — `vh` вместо `vw`.
    pub(super) static ROOT_FONT_VIEW: std::cell::Cell<Option<(bool, f32)>> = const { std::cell::Cell::new(None) };
}

/// Записать кегль корня, заданный единицей окна (`None` — кегль в точках).
pub fn set_root_font_view(view: Option<(bool, f32)>) {
    ROOT_FONT_VIEW.with(|c| c.set(view));
}

/// Записать корневые метрики. Зовётся разбором дерева на элементе `html`
/// (`dom::walk`); сбрасывается на входе разбора (`dom::parse_media`):
/// вложенный документ рамки имеет СВОЙ корень.
pub fn set_root_metrics(font_px: f32, line_px: f32) {
    ROOT_FONT_PX.with(|c| c.set(if font_px > 0.0 { font_px } else { 16.0 }));
    ROOT_LINE_PX.with(|c| c.set(if line_px > 0.0 { line_px } else { 19.2 }));
}

/// Корневые метрики к умолчанию: документ без своего кегля на корне обязан
/// считать `rem` ровно как раньше — иначе правка была бы не про корень, а
/// про все страницы сразу.
pub fn reset_root_metrics() {
    set_root_metrics(16.0, 19.2);
    set_root_font_view(None);
}

pub fn root_font_px() -> f32 {
    ROOT_FONT_PX.with(|c| c.get())
}

pub fn root_line_px() -> f32 {
    ROOT_LINE_PX.with(|c| c.get())
}
