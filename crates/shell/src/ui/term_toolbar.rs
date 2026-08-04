//! Тулбар терминала (TerminalToolbar 1:1, ядро): pill-табы шеллов слева
//! (codicon terminal + label + close ×), «+» справа → дропдаун профилей.
//! Star-default: звезда справа в строке профиля (persist defaultShellId),
//! «default»-тег у дефолтного; новые табы «+»-кнопкой без выбора — им.
//! (Overflow-шевроны табов — след. итерация.)

use crate::host::events::TermEvent;
use crate::ui::term_tb_parts::{TAB_W, scroll_btn};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::term::TermSession;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub fn term_toolbar(
    terminals: &[TermSession],
    active: usize,
    menu_open: bool,
    default_shell: Option<&str>,
    // Горизонтальный скролл полосы табов (`.tabs { overflow-x: auto }`)
    tab_scroll: &gpui::ScrollHandle,
    panel_w: f32,
    // Ширина окна для клампа поповера шеллов гуттером 8
    viewport_w: f32,
    // Высота окна для `max-height: calc(100vh - 16px)` у меню шеллов
    viewport_h: f32,
    p: &'static Palette,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    // `.tabs { overflow-x: auto }` — полоса СКРОЛЛИТСЯ, все табы живут в
    // дереве; шевроны включаются по факту переполнения (ревью ц.23)
    let content_w = terminals.len() as f32 * TAB_W;
    let overflow = content_w > (panel_w - 70.0).max(0.0);
    let mut tabs = div()
        .id("term-tabs")
        .flex()
        .items_end()
        .gap(px(2.0))
        .flex_1()
        .min_w(px(0.))
        // `scrollbar-width: none` — полоса без видимого ползунка
        .overflow_x_scroll()
        .track_scroll(tab_scroll);
    for (i, t) in terminals.iter().enumerate() {
        tabs = crate::ui::term_tab::term_tab(tabs, i, t, active, tx, p);
    }

    // «+» с дропдауном профилей (PowerShell / cmd / Git Bash)
    // .addBtn: 28×28 круглая, self-center; open → accent 14% + accent
    let mut add_btn = div()
        .id("term-add-btn")
        .w(px(28.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .tooltip(crate::ui::tooltip::tooltip("New terminal"))
        .on_mouse_down(gpui::MouseButton::Left, {
            let tx = tx.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Term(TermEvent::ToggleTermMenu));
            }
        })
        .child(crate::ui::icon::codicon("\u{ea60}", 15.0)); // codicon-add
    add_btn = if menu_open {
        add_btn
            .bg(tint(rgba(p.accent_primary), 0.14))
            .text_color(rgba(p.accent_primary))
    } else {
        let hb = rgba(p.bg_surface);
        add_btn.hover(move |s| s.bg(hb).text_color(rgba(p.text_primary)))
    };
    // Обёртка h30 + items_center = align-self:center кнопки в items_end-баре
    let mut add = div()
        .id("term-add")
        .relative()
        .child(crate::probe::registry::probe_area("term-add-btn"))
        // `.anchor { flex-shrink: 0 }` — «+» не сжимается стрипом табов
        .flex_shrink_0()
        .h(px(30.0))
        .flex()
        .items_center()
        .child(add_btn);
    if menu_open {
        let menu =
            crate::ui::term_shell_menu::shell_menu(default_shell, viewport_w, viewport_h, p, tx);
        add = add.child(gpui::deferred(menu).with_priority(60));
    }

    // .bar: align-end, gap space-1, px 25, min-height 30 — активный таб
    // прижат к нижней кромке (сливается с editor-bg телом ниже)
    let mut bar = div()
        .relative()
        .child(crate::probe::registry::probe_area("term-toolbar"))
        .flex()
        .items_end()
        .gap(px(m::SPACE_1))
        .flex_shrink_0()
        .px(px(25.0))
        .min_h(px(30.0));
    // `canLeft = scrollLeft > 1`, `canRight = scrollLeft + clientWidth <
    // scrollWidth − 1` (`TerminalToolbar.tsx:46-47`) — по РЕАЛЬНОМУ скроллу,
    // а не по индексу окна (ревью ц.23)
    let scrolled = -f32::from(tab_scroll.offset().x);
    let max_scroll = f32::from(tab_scroll.max_offset().width);
    if overflow {
        // codicon chevron-left / chevron-right
        bar = bar.child(scroll_btn(
            "term-tabs-left",
            "\u{eab5}",
            scrolled > 1.0,
            -1,
            p,
            tx,
        ));
    }
    bar = bar.child(tabs);
    if overflow {
        bar = bar.child(scroll_btn(
            "term-tabs-right",
            "\u{eab6}",
            scrolled < max_scroll - 1.0,
            1,
            p,
            tx,
        ));
    }
    bar.child(add).into_any_element()
}
