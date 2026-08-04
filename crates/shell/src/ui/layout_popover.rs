//! Поповер «Layout panels» (LayoutToggles 1:1): 6 check-пунктов
//! Left / Left Bottom / File / Center Bottom / Right / Right Bottom —
//! PanelIcon-мини + label + галка; «дети» disabled без родителя (hint
//! Requires X). Клик НЕ закрывает (несколько тумблеров за одно открытие).
//! И поповер «Appearance»: Dark/Light + Icons (Catppuccin built-in).
//! Рендер — overlay-слой (поверх вебвью), hit_area в корне.

pub use crate::ui::appearance_popover::appearance_popover;
use crate::ui::popover::frame::menu_label;
use crate::ui::popover::frame::popover_frame;
use crate::ui::popover::frame::toggle_row;
use crate::ui::popover::presets::presets_section;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_model::LayoutSnapshot;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::panel_placeholder::SlotIcon;

pub(crate) const CHECK: &str = "\u{eab2}";
pub(crate) const POP_W: f32 = 220.0;
/// Поповер layout-тумблеров (кнопка fa-table-columns).
pub fn layout_popover(
    l: &LayoutSnapshot,
    vw: f32,
    vh: f32,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // left = anchor.left триггера ≈ vw−295 → right-офсет 295−POP_W = 75
    popover_frame("layout-popover", "layout-toggles", vw, vh, p)
        .child(menu_label("Layout", p))
        .child(toggle_row(
            "lp-main",
            SlotIcon::Main,
            "Left",
            l.main_visible,
            false,
            None,
            ShellEvent::ToggleLayoutFlag("main"),
            tx,
            p,
        ))
        .child(toggle_row(
            "lp-mainb",
            SlotIcon::MainBottom,
            "Left Bottom",
            l.main_bottom_visible,
            !l.main_visible,
            Some("Requires Left"),
            ShellEvent::ToggleLayoutFlag("mainBottom"),
            tx,
            p,
        ))
        .child(toggle_row(
            "lp-file",
            SlotIcon::Center,
            "File",
            l.file_panel_visible,
            false,
            None,
            ShellEvent::ToggleLayoutFlag("file"),
            tx,
            p,
        ))
        .child(toggle_row(
            "lp-fileb",
            SlotIcon::CenterBottom,
            "Center Bottom",
            l.file_panel_bottom_visible,
            !l.file_panel_visible,
            Some("Requires File"),
            ShellEvent::ToggleLayoutFlag("fileBottom"),
            tx,
            p,
        ))
        .child(toggle_row(
            "lp-right",
            SlotIcon::Right,
            "Right",
            l.right_panel_visible,
            false,
            None,
            ShellEvent::ToggleLayoutFlag("right"),
            tx,
            p,
        ))
        .child(toggle_row(
            "lp-rightb",
            SlotIcon::RightBottom,
            "Right Bottom",
            l.right_panel_bottom_visible,
            !l.right_panel_visible,
            Some("Requires Right"),
            ShellEvent::ToggleLayoutFlag("rightBottom"),
            tx,
            p,
        ))
        .child(
            div()
                .h(px(1.0))
                .mx(px(m::SPACE_2))
                .my(px(m::SPACE_1))
                .bg(tint(rgba(p.text_primary), 0.06)),
        )
        .child(presets_section(tx, p))
        // ⚠ Scrollable-обёртка РАСТЯГИВАЛА поповер на всю высоту (пойман
        // юзером) — скролл длинного меню решать иначе (внутренний список)
        .into_any_element()
}
