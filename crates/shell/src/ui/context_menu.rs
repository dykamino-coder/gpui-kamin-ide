//! Контекст-меню сессии (SessionContextMenu 1:1): fixed-поповер у курсора,
//! min-w 200, bg-surface, divider-soft, shadow-dropdown. Пункты: Rename,
//! Auto-rename (open), Pin/Unpin, Deactivate (open), ряд свотчей + clear,
//! divider, Delete (danger). Клик-мимо/Esc закрывают (скрим-оверлей в root).

pub use crate::ui::ctxmenu::colors::{SESSION_COLORS, resolve_session_color};
use crate::ui::ctxmenu::items::{menu_item, rpc_then_close, swatch};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use serde_json::json;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::modal::{Modal, ModalAction};

// codicon-глифы пунктов
const EDIT: &str = "\u{ea73}";
const PIN: &str = "\u{eb2b}";
const PINNED_DIRTY: &str = "\u{ebb2}";
pub(crate) const CIRCLE_SLASH: &str = "\u{eabd}";
const TRASH: &str = "\u{ea81}";

// min-width 200 (кламп X считает по минимальной ширине)
pub(crate) const MENU_W: f32 = 200.0;
const MENU_MARGIN: f32 = 8.0;

/// Снимок сессии для меню (не держим ссылку на снапшот).
#[derive(Clone)]
pub struct SessionMenuData {
    pub id: String,
    pub name: String,
    pub open: bool,
    pub pinned: bool,
    pub color: Option<String>,
}

/// Открытое меню: данные + позиция клика.
#[derive(Clone)]
pub struct SessionMenu {
    pub data: SessionMenuData,
    pub x: f32,
    pub y: f32,
}

/// Меню-поповер. Скрим-оверлей и клик-мимо монтирует root.
pub fn session_menu(
    menu: &SessionMenu,
    tx: &Sender<ShellEvent>,
    viewport_w: f32,
    viewport_h: f32,
    p: &Palette,
) -> AnyElement {
    let s = &menu.data;
    // Кламп по ИЗМЕРЕННОЙ коробке прошлого кадра (`SessionContextMenu.tsx:33-36`
    // меряет `getBoundingClientRect`), до первого замера — консервативная
    // оценка. Фиксированные 200×260 промахивались на 4 и 81 px (ревью ц.20).
    let [_, _, meas_w, meas_h] =
        crate::probe::registry::bounds_of("session-menu").unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let menu_w = if meas_w > 1.0 { meas_w } else { MENU_W };
    let est_h = if meas_h > 1.0 { meas_h } else { 260.0 };
    let x = menu
        .x
        .min(viewport_w - menu_w - MENU_MARGIN)
        .max(MENU_MARGIN);
    let y = menu
        .y
        .min(viewport_h - est_h - MENU_MARGIN)
        .max(MENU_MARGIN);

    let mut col = div()
        .absolute()
        .left(px(x))
        .top(px(y))
        // Замер собственной коробки для клампа следующего кадра (ревью ц.20).
        // БЕЗ `.relative()`: он перезаписал бы position=Absolute самого меню
        .child(crate::probe::registry::probe_area("session-menu"))
        // .menu min-width 200 (не фикс 208), растёт по контенту — ревью ц.1
        .min_w(px(200.0))
        .flex()
        .flex_col()
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(crate::overlay::dropdown_shadow())
        // Hit-регион overlay-окна: ввод ловится только над меню
        .child(crate::overlay::hit_area())
        // клик внутри меню не закрывает его (скрим ловит только промах)
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());

    // Rename → inline-редактирование строки (1:1 beginRename)
    col = col.child(menu_item("mi-rename", EDIT, "Rename", false, p, {
        let tx = tx.clone();
        let id = s.id.clone();
        move |_, _| {
            let _ = tx.try_send(ShellEvent::CloseSessionMenu);
            let _ = tx.try_send(ShellEvent::BeginRename(id.clone()));
        }
    }));

    // «Auto-rename from chat» — для открытой сессии (CLI regenerateTitle,
    // codicon-sparkle) — как SessionContextMenu.tsx:54-57
    if s.open {
        col = col.child(menu_item(
            "mi-autorename",
            "\u{ec10}",
            "Auto-rename from chat",
            false,
            p,
            {
                let tx = tx.clone();
                let id = s.id.clone();
                move |_, _| {
                    rpc_then_close(
                        &tx,
                        "kamin:command:execute",
                        vec![json!("claude-bridge.regenerateTitle"), json!(id)],
                    );
                }
            },
        ));
    }

    // Pin / Unpin
    let (pin_glyph, pin_label) = if s.pinned {
        (PINNED_DIRTY, "Unpin from top bar")
    } else {
        (PIN, "Pin to top bar")
    };
    col = col.child(menu_item("mi-pin", pin_glyph, pin_label, false, p, {
        let tx = tx.clone();
        let id = s.id.clone();
        let pinned = s.pinned;
        move |_, _| {
            rpc_then_close(
                &tx,
                "kamin:sessions:setPinned",
                vec![json!(id), json!(!pinned)],
            );
        }
    }));

    // Deactivate (free memory) — только активная
    if s.open {
        col = col.child(menu_item(
            "mi-deact",
            CIRCLE_SLASH,
            "Deactivate (free memory)",
            false,
            p,
            {
                let tx = tx.clone();
                let id = s.id.clone();
                move |_, _| {
                    rpc_then_close(&tx, "kamin:sessions:deactivate", vec![json!(id)]);
                }
            },
        ));
    }

    // Свотчи цветов + clear
    let mut swatches = div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap(px(4.0))
        .px(px(m::SPACE_2))
        .py(px(6.0));
    for (i, c) in SESSION_COLORS.iter().enumerate() {
        swatches = swatches.child(swatch(
            &format!("sw-{i}"),
            c,
            s.color.as_deref() == Some(*c),
            s.id.clone(),
            tx,
            p,
        ));
    }
    swatches = swatches.child(
        div()
            .id("sw-clear")
            .w(px(18.0))
            .h(px(18.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .hover(|s| s.text_color(rgba(p.text_primary)))
            .tooltip(crate::ui::tooltip::tooltip("Clear colour"))
            .on_mouse_down(gpui::MouseButton::Left, {
                let tx = tx.clone();
                let id = s.id.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    rpc_then_close(
                        &tx,
                        "kamin:sessions:setColor",
                        vec![json!(id), serde_json::Value::Null],
                    );
                }
            })
            .child(codicon(CIRCLE_SLASH, 13.0)),
    );
    col = col.child(swatches);

    // divider
    col = col.child(
        div()
            .h(px(1.0))
            .mx(px(4.0))
            .my(px(m::SPACE_1))
            .bg(tint(rgba(p.text_primary), 0.06)),
    );

    // Delete → ConfirmModal danger
    col = col.child(menu_item("mi-del", TRASH, "Delete", true, p, {
        let tx = tx.clone();
        let id = s.id.clone();
        let name = s.name.clone();
        move |_, _| {
            let _ = tx.try_send(ShellEvent::CloseSessionMenu);
            let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                title: "Delete session?".into(),
                body: format!(
                    "Session <strong>{name}</strong> will be removed. This cannot be undone."
                )
                .into(),
                confirm_label: "Delete".into(),
                danger: true,
                prompt: None,
                placeholder: None,
                validate: None,
                cancel_label: None,
                action: ModalAction::DeleteSession(id.clone()),
            }));
        }
    }));

    col.into_any_element()
}
