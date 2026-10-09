//! Ширина содержащего блока и доступная ширина (охранники потока).
// owner: A

use crate::layout::block::margins::{COLLAPSE_CB_WIDTH_PX, COLLAPSE_FONT_PX};
use crate::render::in_flow;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

thread_local! {
    /// Ширина содержащего блока в точках, когда её видно из стиля родителя.
    /// Нужна замещаемому элементу БЕЗ собственного размера, но С соотношением:
    /// §10.3.2 (последний пункт) берёт его ширину из уравнения для блочных
    /// коробок, то есть из содержащего блока, а резерв 300×150 применяется
    /// только когда ширину взять неоткуда.
    pub(crate) static CB_WIDTH: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Вернуть прежнюю ширину содержащего блока по выходе из `blocks()`.
pub(crate) struct CbWidthGuard(pub(crate) Option<f32>);

impl Drop for CbWidthGuard {
    fn drop(&mut self) {
        CB_WIDTH.set(self.0);
    }
}

pub(crate) fn scopeguard_cb(prev: Option<f32>) -> CbWidthGuard {
    CbWidthGuard(prev)
}

thread_local! {
    /// Ширина, которую получит БЛОЧНЫЙ ребёнок с `width: auto` в текущем
    /// `blocks()`: stretch-fit содержащего блока (CSS 2.1 §10.3.3, css-sizing-3
    /// «definite»: inline-размер block-level коробки в потоке ОПРЕДЕЛЁН, даже когда
    /// `width` не задан). Нужна счёту `repeat(auto-fill | auto-fit, …)` в лунках
    /// (css-grid-1 §7.2.3.2): прежде у `width: auto` размер считался неизвестным, и
    /// повтор не разворачивался вовсе (`column-auto-repeat-016`: три лунки вместо
    /// семи по 100 в 784). `None` — ширину честно взять неоткуда (флекс, сетка,
    /// таблица, строчный блок, флоат, абсолют, многоколоночник, вертикальное письмо).
    pub(crate) static AVAIL_W: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Доступная ширина текущего уровня `blocks()` (см. `AVAIL_W`) — для долей
/// у строчных коробок абзаца: их содержащий блок — блок абзаца.
pub(crate) fn avail_width() -> Option<f32> {
    AVAIL_W.get()
}

/// Вернуть прежнюю доступную ширину по выходе из `blocks()`.
pub(crate) struct AvailWGuard(pub(crate) Option<f32>);

impl Drop for AvailWGuard {
    fn drop(&mut self) {
        AVAIL_W.set(self.0);
    }
}

/// Ширина СОДЕРЖИМОГО коробки в точках — содержащий блок её детей (CSS 2.1
/// §10.1 п.2) — для процентных полей внуков в цепочках схлопывания. `cb` —
/// содержащий блок самой коробки; `None` — в точках не выводится.
fn inner_width_px(c: &Computed, cb: Option<f32>) -> Option<f32> {
    let side = |l: Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => cb.map(|w| k * w),
        _ => None,
    };
    let b = c.borders();
    let edges =
        || Some(side(c.padding.left)? + side(c.padding.right)? + side(b.left)? + side(b.right)?);
    match c.width {
        Some(Len::Px(w)) if c.border_box == Some(true) => Some((w - edges()?).max(0.0)),
        Some(Len::Px(w)) => Some(w),
        Some(Len::Pct(k)) if c.border_box != Some(true) => cb.map(|w| k * w),
        // §10.3.3: блок в потоке с `width: auto` занимает содержащий блок за
        // вычетом своих полей, рамок и отступов (auto-поле — ноль).
        None | Some(Len::Auto) if in_flow(c) => {
            Some((cb? - side(c.margin.left)? - side(c.margin.right)? - edges()?).max(0.0))
        }
        _ => None,
    }
}

/// `f` под шириной содержимого коробки `c` вместо ширины уровня. Прежде
/// цепочка мерила доли внуков от содержащего блока УРОВНЯ: у `body` с
/// `width: auto` его нет, и `15%` у `#parent` внутри `#grand-parent {width:
/// 400px}` роняло цепочку целиком — поля абзаца и `#parent` складывались
/// (16 + 60 вместо 60, `margin-percentage-inherit-001`).
pub(super) fn with_inner_cb<T>(c: &Computed, f: impl FnOnce() -> T) -> T {
    let level = COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get);
    let prev = COLLAPSE_CB_WIDTH_PX.with(|w| w.replace(inner_width_px(c, level)));
    // Кегль уровня — тоже от спуска: поля детей в `em` без своего кегля (и
    // с кеглем в `em`) меряются от кегля ЭТОЙ коробки, а не уровня выше.
    // Прежде `html{font-size:2em} p{font-size:.5em}` схлопывал поле абзаца
    // сквозь `body` по 8 точкам вместо 16 (`numbers-units-021`).
    let font = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
    let own = match c.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * font,
        _ => font,
    };
    let prev_font = COLLAPSE_FONT_PX.with(|w| w.replace(own));
    let out = f();
    COLLAPSE_CB_WIDTH_PX.with(|w| w.set(prev));
    COLLAPSE_FONT_PX.with(|w| w.set(prev_font));
    out
}
