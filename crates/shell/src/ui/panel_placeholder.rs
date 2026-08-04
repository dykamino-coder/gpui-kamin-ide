//! PanelPlaceholder 1:1: пустое состояние карты-панели — иконка-рамка слота
//! (PanelIcon), заголовок, подсказка. Иконка рисуется нативными div (рамка +
//! подсвеченный слот) вместо SVG — currentColor берётся из text_muted.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;

/// Какой слот подсвечен внутри рамки-иконки (PanelIcon.tsx 1:1).
#[derive(Clone, Copy)]
pub enum SlotIcon {
    /// = left: полный левый столбец (main-колонка «Left»).
    Main,
    /// Левый НИЖНИЙ квадрат (Left Bottom drawer).
    MainBottom,
    /// Центральная вертикальная полоса (File-колонка).
    Center,
    /// Центральная нижняя полоска (Center Bottom).
    CenterBottom,
    /// Полный правый столбец.
    Right,
    /// Правый верхний квадрат.
    RightTop,
    /// Правый нижний квадрат.
    RightBottom,
    /// Нижняя широкая полоса (fallback «bottom» оригинала).
    #[allow(dead_code)]
    Bottom,
}

// Геометрия PanelIcon.tsx 1:1: канва 14×12; РАМКА = rect x1 y1 w12 h10
// rx1.5 stroke1.2 (НЕ бордер на всей канве — ревью ц.2/юзер); highlight-бары
// координатами ОТ КАНВЫ. Ширины 4.5, RIGHT_X=8, CENTER_X=4.75.
fn glyph(slot: SlotIcon, scale: f32, color: gpui::Rgba) -> AnyElement {
    // `PanelIcon.tsx` — это SVG: рамка `rect x1 y1 w12 h10 rx1.5 stroke-width 1.2`
    // и подсвеченный бар `fill-opacity .85`. Сборка из `div`+`border` давала
    // штрих 0.8 лог. вместо 1.2 (ревью ц.23: «упор gpui» был ложным — `svg()`
    // рисует и штрих, и fill-opacity), поэтому каждый слот — свой ассет.
    let path = match slot {
        SlotIcon::Main => "icons/panel-slot-main.svg",
        SlotIcon::MainBottom => "icons/panel-slot-main-bottom.svg",
        SlotIcon::Center => "icons/panel-slot-center.svg",
        SlotIcon::CenterBottom => "icons/panel-slot-center-bottom.svg",
        SlotIcon::Right => "icons/panel-slot-right.svg",
        SlotIcon::RightTop => "icons/panel-slot-right-top.svg",
        SlotIcon::RightBottom => "icons/panel-slot-right-bottom.svg",
        SlotIcon::Bottom => "icons/panel-slot-bottom.svg",
    };
    gpui::svg()
        .path(path)
        .flex_shrink_0()
        .w(px(14.0 * scale))
        .h(px(12.0 * scale))
        .text_color(color)
        .into_any_element()
}

/// Большая иконка плейсхолдера: 28×24 = масштаб 2.0 от 14×12 (оригинал).
fn slot_glyph(slot: SlotIcon, p: &Palette) -> AnyElement {
    glyph(slot, 2.0, rgba(p.text_muted))
}

/// Мини-иконка для поповера Layout panels (≈17×14) — цвет наследуется от
/// контейнера (`currentColor`), поэтому передаётся явно.
pub fn slot_glyph_small_colored(slot: SlotIcon, color: gpui::Rgba) -> AnyElement {
    glyph(slot, 1.0, color)
}

/// Мини-иконка цветом `--text-muted` (Layout-меню и меню «Move to»).
pub fn slot_glyph_small(slot: SlotIcon, p: &Palette) -> AnyElement {
    glyph(slot, 1.0, rgba(p.text_muted))
}

/// Пустое состояние панели: иконка слота + label + hint (+ `extra` — напр.
/// кнопка «Open Tool ▾» в конце колонки).
pub fn panel_placeholder_ex(
    label: &str,
    hint: &str,
    slot: SlotIcon,
    extra: Option<AnyElement>,
    p: &Palette,
) -> AnyElement {
    div()
        .relative()
        .child(crate::probe::registry::probe_area("panel-placeholder"))
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_2))
        .p(px(m::SPACE_5))
        .overflow_hidden()
        .text_color(rgba(p.text_muted))
        // `.placeholder { text-align: center }` — многострочный label иначе
        // прижимается влево (ревью ц.13)
        .text_center()
        .child(div().mb(px(m::SPACE_1)).child(slot_glyph(slot, p)))
        .child(
            div()
                .text_size(px(m::FS_LG))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                // lh-snug 1.3; max-width у .hint в CSS НЕТ (ревью цикла 1)
                .line_height(px(m::FS_SM * 1.3))
                .text_center()
                .child(hint.to_string()),
        )
        .when_some(extra, |d, extra| d.child(extra))
        .into_any_element()
}

/// Плейсхолдер без extra (например центр «File»).
pub fn panel_placeholder(label: &str, hint: &str, slot: SlotIcon, p: &Palette) -> AnyElement {
    panel_placeholder_ex(label, hint, slot, None, p)
}

/// ActivityPlaceholder 1:1 — пустое тело ВЫБРАННОЙ активности (без пикера):
/// ToolIcon 36 text-disabled + label fs-md/600 + «Nothing to show here yet.»
pub fn activity_placeholder(icon: &str, label: &str, p: &Palette) -> AnyElement {
    const GLYPH: f32 = 36.0;
    // Путь берём ИЗ мапы: у алиасов имя файла другое («problems» →
    // icons/warning.svg), а `format!("icons/{icon}.svg")` давал 404 (ревью ц.7)
    // ЕДИНЫЙ резолв, как у бара и стрипа (`ToolIcon`): свой код здесь
    // падал в `codicon-play` для неизвестного имени, а `<img>`-ветки не знал
    // вовсе — один тул давал «play» в теле панели и «file» в баре (ц.21).
    // svg-ветка 36 (атрибуты width/height), codicon — вендорная база 16.
    let icon_el: AnyElement =
        crate::ui::activity_bar::tool_glyph_split(icon, GLYPH, 16.0, rgba(p.text_disabled));
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_2))
        .p(px(m::SPACE_5))
        // `.placeholder { text-align: center }` — центрируется весь блок,
        // не только хинт (ревью ц.11)
        .text_center()
        .text_color(rgba(p.text_muted))
        .child(div().mb(px(m::SPACE_1)).child(icon_el))
        .child(
            div()
                .text_size(px(m::FS_MD))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(m::FS_XS))
                // `.hint { line-height: var(--lh-snug) }` = 1.3
                .line_height(px(m::FS_XS * 1.3))
                .max_w(px(240.0))
                .text_center()
                .child("Nothing to show here yet."),
        )
        .into_any_element()
}
