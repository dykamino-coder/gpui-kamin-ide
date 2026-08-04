//! Тулбар панели System log: поиск, фильтр уровней, «очистить».
//!
//! Вынесено из `system.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::logs::parts::capitalize;
use gpui::prelude::*;
use gpui::{Entity, SharedString, div, px};
use gpui_component::input::{Input, InputState};
use gpui_component::{Sizable as _, Size};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn toolbar(
    filter: Option<(&Entity<InputState>, String)>,
    search_focused: bool,
    level: &'static str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Div {
    // `.toolbar` — gap 8, padding 0 0 8: [search][levels][clear]
    let mut toolbar = div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .flex_shrink_0()
        .pb(px(m::SPACE_2));
    if let Some((inp, _)) = filter.as_ref() {
        // `.search` — flex 1, height 28, padding 0 10, bg-base, border divider-soft
        toolbar = toolbar.child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h(px(28.0))
                // `.search { padding: 0 10px }` → инсет 11 (10 + рамка 1);
                // `Input` кладёт свои 8, обёртке остаётся 3 (ревью ц.26)
                .pl(px(3.0))
                .pr(px(10.0))
                .flex()
                .items_center()
                .rounded(px(m::RADIUS_SM))
                .bg(rgba(p.bg_base))
                .border_1()
                // `:focus-within → accent-primary` (ревью ц.23)
                .border_color(if search_focused {
                    rgba(p.accent_primary)
                } else {
                    tint(rgba(p.text_primary), 0.06)
                })
                .text_size(px(m::FS_SM))
                .child(
                    Input::new(inp)
                        .appearance(false)
                        .with_size(Size::Size(px(m::FS_SM / 0.875))),
                ),
        );
    }
    // `.levels` gap 2; `.levelBtn` 4/10 r-sm fs-xs capitalize;
    // active = accent 22% + text-primary; hover = text-primary 8%
    let mut levels = div().flex().gap(px(2.0)).flex_shrink_0();
    for l in ["all", "error", "warning", "info"] {
        let active = level == l;
        let tx_l = tx.clone();
        let hover_bg = tint(rgba(p.text_primary), 0.08);
        let mut b = div()
            .id(SharedString::from(format!("syslog-lvl-{l}")))
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(m::RADIUS_SM))
            // `.levelBtn { border: 1px solid transparent }` — без резерва
            // пилюли на 2px уже и ниже оригинала (ревью ц.11)
            .border_1()
            .border_color(gpui::transparent_black())
            .text_size(px(m::FS_XS))
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx_l.try_send(ShellEvent::Cz(CzEvent::SetSysLogLevel(l)));
            })
            .child(capitalize(l));
        // `.levelBtn:hover` (0,2,0) объявлен ПОСЛЕ `.levelActive` (0,1,0) и
        // бьёт его: активная пилюля под курсором ТОЖЕ уходит в
        // `text-primary 8 %`, теряя accent-заливку (ревью ц.26).
        // `hover` у gpui можно поставить лишь ОДИН раз — иначе
        // `debug_assert` «hover style already set» роняет процесс, что я и
        // сделал первой версией правки
        if active {
            b = b
                .bg(tint(rgba(p.accent_primary), 0.22))
                .text_color(rgba(p.text_primary));
        }
        b = b.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)));
        levels = levels.child(b);
    }
    toolbar = toolbar.child(levels);
    // `.clear` — 28×28 grid, codicon-clear-all, hover text-primary 10%
    let tx_clear = tx.clone();
    toolbar = toolbar.child(
        div()
            .id("syslog-clear")
            .w(px(28.0))
            .h(px(28.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(m::RADIUS_SM))
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tooltip("Clear logs"))
            .hover({
                let hb = tint(rgba(p.text_primary), 0.10);
                move |s| s.bg(hb).text_color(rgba(p.text_primary))
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx_clear.try_send(ShellEvent::Cz(CzEvent::ClearSystemLog));
            })
            .child(codicon("\u{eabf}", 16.0)), // clear-all: своего кегля у класса нет
    );
    toolbar
}
