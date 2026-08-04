//! Контекст-меню страницы в offscreen-режиме — В ТЕМЕ приложения.
//!
//! В windowless-CEF никто меню не показывает: `run_context_menu` отдаёт нам
//! модель (Undo/Copy/Paste/Back/Reload…), а рисовать обязан хост. Рисуем
//! СВОИМ слоем оверлеев (`ui/web_menu.rs`) — как остальные поповеры, по
//! запросу «под тему» (системный `TrackPopupMenu` был белым и вдобавок
//! требовал отдельный поток: владелец обязан жить на вызывающем потоке).
//!
//! `RunContextMenuCallback` не переносится между потоками и слоем не
//! используется вовсе: CEF'у сразу отвечаем `cancel()`, а выбранную команду
//! исполняем САМИ методами `Frame`/`Browser` (copy/paste/undo/go_back/…).

use cef::rc::*;
use cef::*;

/// Команды стандартного меню CEF (`cef_menu_id_t`) — исполняем сами.
const MENU_ID_BACK: i32 = 100;
const MENU_ID_FORWARD: i32 = 101;
const MENU_ID_RELOAD: i32 = 102;
const MENU_ID_RELOAD_NOCACHE: i32 = 103;
const MENU_ID_STOPLOAD: i32 = 104;
const MENU_ID_UNDO: i32 = 110;
const MENU_ID_REDO: i32 = 111;
const MENU_ID_CUT: i32 = 112;
const MENU_ID_COPY: i32 = 113;
const MENU_ID_PASTE: i32 = 114;
// 115 — «Paste as plain text» (PASTE_MATCH_STYLE): у `Frame` нет метода
// вставки без форматирования, пункт с подменённым поведением не показываем.
const MENU_ID_DELETE: i32 = 116;
const MENU_ID_SELECT_ALL: i32 = 117;
const MENU_ID_VIEW_SOURCE: i32 = 132;

fn known(cmd: i32) -> bool {
    matches!(
        cmd,
        MENU_ID_BACK
            | MENU_ID_FORWARD
            | MENU_ID_RELOAD
            | MENU_ID_RELOAD_NOCACHE
            | MENU_ID_STOPLOAD
            | MENU_ID_UNDO
            | MENU_ID_REDO
            | MENU_ID_CUT
            | MENU_ID_COPY
            | MENU_ID_PASTE
            | MENU_ID_DELETE
            | MENU_ID_SELECT_ALL
            | MENU_ID_VIEW_SOURCE
    )
}

/// Выполнить команду меню над браузером вью (на потоке CEF).
pub(crate) fn execute(id: &str, cmd: i32) {
    super::input::on_browser(id, move |host| {
        let Some(browser) = host.browser() else {
            return;
        };
        match cmd {
            MENU_ID_BACK => browser.go_back(),
            MENU_ID_FORWARD => browser.go_forward(),
            MENU_ID_RELOAD => browser.reload(),
            MENU_ID_RELOAD_NOCACHE => browser.reload_ignore_cache(),
            MENU_ID_STOPLOAD => browser.stop_load(),
            _ => {
                let Some(frame) = browser.focused_frame().or(browser.main_frame()) else {
                    return;
                };
                match cmd {
                    MENU_ID_UNDO => frame.undo(),
                    MENU_ID_REDO => frame.redo(),
                    MENU_ID_CUT => frame.cut(),
                    MENU_ID_COPY => frame.copy(),
                    MENU_ID_PASTE => frame.paste(),
                    MENU_ID_DELETE => frame.del(),
                    MENU_ID_SELECT_ALL => frame.select_all(),
                    MENU_ID_VIEW_SOURCE => frame.view_source(),
                    _ => {}
                }
            }
        }
    });
}

cef::wrap_context_menu_handler! {
    pub(crate) struct ViewContextMenu {
        id: String,
    }
    impl ContextMenuHandler {
        fn run_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> ::std::os::raw::c_int {
            let (Some(params), Some(model), Some(callback)) = (params, model, callback) else {
                return 0;
            };
            // CEF'у меню «отменено» сразу: его callback не пережил бы смену
            // потока. Команду выбора исполним сами (см. шапку модуля).
            callback.cancel();
            let Some((wx, wy)) =
                super::element::window_point(&self.id, params.xcoord(), params.ycoord())
            else {
                return 1;
            };
            let items = collect_items(model);
            if items.iter().flatten().count() == 0 {
                return 1;
            }
            if let Some(tx) = crate::host_link::event_tx() {
                let _ = tx.try_send(crate::host_link::ShellEvent::WebMenu(Some(
                    crate::ui::web_menu::WebMenuState {
                        view: self.id.clone(),
                        items,
                        x: wx,
                        y: wy,
                    },
                )));
                super::repaint_requested();
            }
            1
        }
    }
}

/// Пункт меню: команда, подпись, доступность; `None` — разделитель.
type MenuItem = Option<(i32, String, bool)>;

fn collect_items(model: &mut MenuModel) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = Vec::new();
    for i in 0..model.count() {
        let sep =
            model.type_at(i).as_ref() == &cef::sys::cef_menu_item_type_t::MENUITEMTYPE_SEPARATOR;
        if sep {
            // Схлопнуть подряд идущие/ведущие разделители сразу.
            if items.last().is_some_and(|last| last.is_some()) {
                items.push(None);
            }
            continue;
        }
        let cmd = model.command_id_at(i);
        // Показываем только команды, которые умеем исполнить сами: чужой
        // пункт с «мёртвым» кликом хуже отсутствующего.
        if !known(cmd) {
            continue;
        }
        // «&» в подписях CEF — мнемоники Win32-меню; наш слой рисует текст
        // как есть: одиночные убираем, «&&» — буквальный амперсанд.
        let raw = CefStringUtf16::from(&model.label_at(i)).to_string();
        let label = raw
            .replace("&&", "\u{1}")
            .replace('&', "")
            .replace('\u{1}', "&");
        if label.is_empty() {
            continue;
        }
        items.push(Some((cmd, label, model.is_enabled_at(i) == 1)));
    }
    while items.last().is_some_and(|last| last.is_none()) {
        items.pop();
    }
    items
}
