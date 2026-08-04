//! Полоса файл-табов редактора (FileViewerTabs 1:1, ядро): pill-табы —
//! Catppuccin-иконка + имя + dirty-точка + close ×; активный accent-tinted;
//! RMB → меню (Close / Close Others / Close to the Right / Close All +
//! файловые действия дерева). (Pointer-reorder и overflow ▾ — след. итерация.)

use crate::host::events::EdEvent;
pub use crate::ui::editor::tab_menu::{EditorTabMenu, editor_tab_menu};
pub use crate::ui::editor::tab_name::{base_name, tab_width};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;

/// Полоса табов: (path, dirty, pinned) слайс + активный индекс.
/// drag_over — цель reorder (индикатор-полоса на левом крае таба).
/// Не влезшие в available_w — под кнопку «N ▾» (как чипы сессий).
#[allow(clippy::too_many_arguments)]
pub fn editor_tabs_bar(
    tabs: &[(String, bool, bool)],
    // Ширины табов, измеренные шейпером (индексы совпадают с `tabs`)
    widths: &[f32],
    active: usize,
    drag_over: Option<usize>,
    // `.tabDragging { opacity: 0.3 }` — индекс таба, который сейчас тащат
    dragging: Option<usize>,
    available_w: f32,
    // Высота вьюпорта: `.overflowMenu { max-height: 60vh }` — vh в gpui нет,
    // считаем от реального окна (было фикс 400 и без прокрутки)
    viewport_h: f32,
    overflow_open: bool,
    // Скролл стрипа + запрос «довезти активный таб» (plan/99 п.42).
    scroll: &gpui::ScrollHandle,
    reveal_active: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Стрип рисует ВСЕ табы в исходном порядке и режет их `overflow: hidden`
    // (`FileViewerTabs.tsx:166` + css:21). Кнопка «▾» включается по факту
    // переполнения, как `scrollWidth > clientWidth + 1` (:58-62), и НИЧЕГО
    // из стрипа не изымает — в меню перечислены все открытые файлы
    // (ревью ц.20: выкидывание оставляло 161 px пустоты и ломало порядок).
    let total_w: f32 = widths.iter().map(|w| w + 4.0).sum();
    let fit_all = total_w <= available_w;
    let visible: Vec<usize> = (0..tabs.len()).collect();

    // `.bar` — только ряд; отступы, gap и обрезка живут в `.strip`, у которого
    // `flex: 1` (иначе «▾» не прижимается к правому краю ряда — ревью ц.13)
    let bar = div().flex().items_center().flex_shrink_0();
    // Автоподвоз активного таба в видимую зону при его смене (иначе смена
    // из меню/дерева оставляла активный за обрезом). Смещение до таба
    // считается из тех же widths, что и раскладка. set_offset — интерьерная
    // мутабельность ScrollHandle, вызов из рендера легален.
    if reveal_active && !widths.is_empty() {
        let acc: f32 = widths.iter().take(active).map(|w| w + 4.0).sum();
        let aw = widths.get(active).copied().unwrap_or(0.0);
        let off = -f32::from(scroll.offset().x);
        let view_w = (available_w - 2.0 * m::SPACE_2).max(40.0);
        if acc < off {
            scroll.set_offset(gpui::point(px(-acc), px(0.0)));
        } else if acc + aw > off + view_w {
            scroll.set_offset(gpui::point(px(-(acc + aw - view_w)), px(0.0)));
        }
    }
    let mut strip = div()
        .id("editor-tabs-strip")
        .relative()
        .child(crate::probe::registry::probe_area("editor-tabs-bar"))
        .flex()
        .items_center()
        .gap(px(4.0))
        .flex_1()
        .min_w(px(0.))
        .px(px(m::SPACE_2))
        // .strip padding 4px по вертикали симметрично (ревью ц.1)
        .py(px(m::SPACE_1))
        // Скролл, а не обрез: колесо/тачпад листают стрип (plan/99 п.42);
        // «▾»-меню со ВСЕМИ файлами остаётся.
        .overflow_x_scroll()
        .track_scroll(scroll);
    // Цель вставки считается на СТРИПЕ по СЕРЕДИНАМ табов, как
    // `FileViewerTabs.tsx:112-117`: первый таб, чья середина правее курсора;
    // если такого нет — вставка ПОСЛЕ последнего (`over == len`).
    {
        let tx = tx.clone();
        let widths: Vec<f32> = widths.to_vec();
        strip = strip.on_mouse_move(move |e: &gpui::MouseMoveEvent, _, _| {
            if e.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            // probe-область — ребёнок стрипа и ЕДЕТ вместе со скроллом, так
            // что sx уже включает офсет — экранные координаты табов
            // считаются от неё без поправок.
            let Some([sx, _, _, _]) = crate::probe::registry::bounds_of("editor-tabs-bar") else {
                return;
            };
            let mut x = sx + m::SPACE_2;
            let cursor = f32::from(e.position.x);
            let mut over = widths.len();
            for (i, w) in widths.iter().enumerate() {
                if cursor < x + w / 2.0 {
                    over = i;
                    break;
                }
                x += w + 4.0;
            }
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::TabDragOver(over)));
        });
    }
    for i in visible {
        strip = crate::ui::editor::tab_item::tab_item(
            strip, i, tabs, active, widths, dragging, drag_over, p, tx,
        );
    }
    // `over === tabEls.length` (`FileViewerTabs.tsx:119-122`): индикатор ПОСЛЕ
    // последнего таба — его правый край плюс полугэп. Мы такой позиции не
    // рисовали вовсе, хотя событие с ней шлётся (ревью ц.26)
    let tail_noop = dragging.is_some_and(|from| from + 1 == tabs.len());
    if drag_over == Some(tabs.len()) && !tail_noop {
        strip = strip.child(
            div()
                .w(px(2.0))
                .my(px(5.0))
                .ml(px(2.0))
                .rounded(px(1.0))
                .bg(rgba(p.accent_primary))
                .flex_shrink_0(),
        );
    }
    crate::ui::editor::tabs_overflow::overflow(
        bar,
        strip,
        fit_all,
        tabs,
        widths,
        active,
        available_w,
        viewport_h,
        overflow_open,
        tx,
        p,
    )
}
