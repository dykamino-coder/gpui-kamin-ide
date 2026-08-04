//! Части выбора тула: подпись слота, иконка, якорь.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::activity::PanelSlot;
use crate::colors::rgba;
use gpui::AnyElement;
use kamin_theme::Palette;

pub(crate) const PICKER_W: f32 = 220.0;
pub(crate) const CHECK: &str = "\u{eab2}"; // codicon-check
pub(crate) const SINGLE: &str = "\u{ea75}"; // codicon-lock — метка тула-одиночки
/// Подпись слота — та же, что в меню «Move to» (`overlay.rs`).
pub(crate) fn slot_label(slot: PanelSlot) -> &'static str {
    match slot {
        PanelSlot::Sidebar => "Sidebar",
        PanelSlot::Main => "Left",
        PanelSlot::MainBottom => "Left Bottom",
        PanelSlot::CentralBottom => "Center Bottom",
        PanelSlot::RightTop => "Right",
        PanelSlot::RightBottom => "Right Bottom",
    }
}
/// Иконка тула — ЕДИНЫЙ резолв (`ToolIcon` оригинала). Своя таблица токенов
/// расходилась с общей мапой на четыре алиаса, и один и тот же contributed-тул
/// в пикере и в рейле рисовался разными иконками (ревью ц.12).
pub(crate) fn tool_icon(icon: &str, p: &Palette) -> AnyElement {
    // `.menuItem` красит содержимое в text-primary — иконка наследует
    // svg — 18 (`DEFAULT_SIZE_PX`), codicon — база 16: у `.menuItem` своего
    // правила для `.codicon` нет (ревью ц.13)
    crate::ui::activity_bar::tool_glyph_split(icon, 18.0, 16.0, rgba(p.text_primary))
}
/// Рендер пикера для `slot` в точке (x, y); `up` — раскрыть вверх.
/// id probe-региона триггера пикера в слоте: «…» стрипа, «…» бара сайдбара
/// либо пилюля «Open Tool ▾» пустого слота — в один момент виден ровно один.
pub fn picker_anchor_id(slot: PanelSlot) -> &'static str {
    crate::activity::intern(&format!("picker-anchor-{}", slot.as_str()))
}
