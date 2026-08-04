//! Ленивое создание инпутов кадра: переименование, палитра, поиск, модалка.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::host_link::{self, ShellEvent};
use crate::state::consts::{
    FIF_DEBOUNCE_MS, LOG_FILTER_DEBOUNCE_MS, QO_DEBOUNCE_MS, WS_DEBOUNCE_MS,
};
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{Context, Focusable, Window};
use gpui_component::input::{InputEvent, InputState};

impl RootView {
    pub(crate) fn frame_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.renaming_session.clone() {
            if self.rename_input.is_none() {
                let seed = self
                    .sessions
                    .as_ref()
                    .and_then(|s| s.sessions.iter().find(|x| x.id == id))
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                let input = cx.new(|cx| {
                    let mut st = InputState::new(window, cx);
                    st.set_value(seed, window, cx);
                    st
                });
                window.focus(&input.read(cx).focus_handle(cx));
                // `ref.select()` при входе в правку (`SessionItem.tsx:41`):
                // имя выделено целиком, ввод сразу заменяет его (ревью ц.13)
                window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
                // `onBlur → commit` (`SessionItem.tsx:82`): уход фокуса
                // сохраняет имя, как Enter. Раньше правку можно было только
                // подтвердить Enter'ом или отменить Esc (ревью ц.13).
                self.rename_sub = Some(cx.subscribe(&input, |this, _, ev: &InputEvent, _| {
                    if matches!(ev, InputEvent::Blur) && this.renaming_session.is_some() {
                        let _ = this.tx.try_send(ShellEvent::CommitRename);
                    }
                }));
                self.rename_input = Some(input);
            }
        } else if self.rename_input.is_some() {
            self.rename_input = None;
        }
        // QuickPick: фильтр-инпут (Change → перефильтр)
        if self.quick_pick.is_some() && self.qp_input.is_none() {
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder(
                    self.quick_pick
                        .as_ref()
                        .and_then(|q| q.placeholder.clone())
                        .unwrap_or_else(|| "Type to filter…".into()),
                )
            });
            window.focus(&input.read(cx).focus_handle(cx));
            self.qp_sub = Some(cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify()));
            self.qp_input = Some(input);
        }
        // Семпл-инпут Design: создаём при показе страницы
        if self.cz.customize_open
            && self.cz.customize_panel == "design"
            && self.cz.design_input.is_none()
        {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Sample input"));
            self.cz.design_input = Some(input);
        }
        // Фильтр Logs/System: инпут + Change-подписка (перефильтр по вводу)
        if self.cz.customize_open
            && matches!(self.cz.customize_panel, "logs" | "system")
            && (self.cz.log_filter_input.is_none()
                || self.cz.log_filter_panel != self.cz.customize_panel)
        {
            // Плейсхолдеры РАЗНЫЕ: Logs — «Filter…» (`LogsPanel.tsx:103`),
            // System — «Filter logs…» (`SystemLogPanel.tsx:33`). Один общий
            // инпут ставил в Logs чужой текст (регрессия волны 10).
            let ph = if self.cz.customize_panel == "system" {
                "Filter logs…"
            } else {
                "Filter…"
            };
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(ph));
            self.cz.log_filter_sub =
                Some(cx.subscribe(&input, |this, entity, _: &InputEvent, cx| {
                    // 150 мс, как `FILTER_DEBOUNCE_MS`: перефильтровка буфера
                    // на каждое нажатие перебирала весь канал
                    let q = entity.read(cx).value().to_string();
                    this.cz.log_filter_gen = this.cz.log_filter_gen.wrapping_add(1);
                    let g = this.cz.log_filter_gen;
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(
                            LOG_FILTER_DEBOUNCE_MS,
                        ))
                        .await;
                        let _ = this.update(cx, |this, cx| {
                            if this.cz.log_filter_gen == g {
                                this.cz.log_filter_value = q;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }));
            self.cz.log_filter_input = Some(input);
            self.cz.log_filter_panel = self.cz.customize_panel;
        }
        // Скрим внутрь вебвью: div rgba(0,0,0,.5) через evaluate_script —
        // единственный способ затемнить WebView2-чайлды вместе с фоном
        {
            let dim = self.palette_open
                || self.sov.quickopen_open
                || self.sov.fif_open
                || self.sov.ws_open
                || self.modal.is_some();
            if dim != self.webview_scrim {
                self.webview_scrim = dim;
                let js = if dim {
                    "(function(){if(!window.__kaminScrim){var d=document.createElement('div');d.style.cssText='position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:2147483647;pointer-events:none';document.documentElement.appendChild(d);window.__kaminScrim=d;}})();"
                } else {
                    "(function(){if(window.__kaminScrim){window.__kaminScrim.remove();window.__kaminScrim=null;}})();"
                };
                for id in crate::host_link::KNOWN_WEBVIEWS {
                    crate::web::execute_script(id, js);
                }
            }
        }
        // Палитра команд: инпут + подписка на Change (перефильтр по вводу)
        if self.palette_open && self.palette_input.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Type a command name…"));
            window.focus(&input.read(cx).focus_handle(cx));
            self.palette_sub = Some(cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify()));
            self.palette_input = Some(input);
        }
        // Quick Open: инпут + подписка (Change → findFile-запрос)
        if self.sov.quickopen_open && self.sov.quickopen_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a file name…"));
            window.focus(&input.read(cx).focus_handle(cx));
            self.sov.quickopen_sub =
                Some(cx.subscribe(&input, |this, entity, _: &InputEvent, cx| {
                    let q = entity.read(cx).value().to_string();
                    this.sov.qo_gen = this.sov.qo_gen.wrapping_add(1);
                    this.sov.qo_active = 0;
                    let g = this.sov.qo_gen;
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(QO_DEBOUNCE_MS)).await;
                        let _ = this.update(cx, |this, _| {
                            if this.sov.qo_gen == g {
                                host_link::request_find_file(this.tx.clone(), q);
                            }
                        });
                    })
                    .detach();
                    cx.notify();
                }));
            self.sov.quickopen_input = Some(input);
        }
        // Find in Files: инпут + подписка (Change → findInFiles при len>=2)
        if self.sov.fif_open && self.sov.fif_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search in files…"));
            window.focus(&input.read(cx).focus_handle(cx));
            self.sov.fif_sub = Some(cx.subscribe(&input, |this, entity, _: &InputEvent, cx| {
                let q = entity.read(cx).value().to_string();
                this.sov.fif_query_len = q.chars().count();
                this.sov.fif_gen = this.sov.fif_gen.wrapping_add(1);
                this.sov.fif_active = 0;
                let g = this.sov.fif_gen;
                if this.sov.fif_query_len >= 2 {
                    // busy взводится сразу, как в оригинале (setBusy до таймера)
                    this.sov.fif_busy = true;
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(FIF_DEBOUNCE_MS)).await;
                        let _ = this.update(cx, |this, _| {
                            if this.sov.fif_gen == g {
                                host_link::request_find_in_files(this.tx.clone(), q);
                            }
                        });
                    })
                    .detach();
                } else {
                    this.sov.fif_results.clear();
                    this.sov.fif_busy = false;
                }
                cx.notify();
            }));
            self.sov.fif_input = Some(input);
        }
        // Go to Symbol: инпут + подписка (Change → workspaceSymbol при len>=1)
        if self.sov.ws_open && self.sov.ws_input.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Go to symbol in workspace…"));
            window.focus(&input.read(cx).focus_handle(cx));
            self.sov.ws_sub = Some(cx.subscribe(&input, |this, entity, _: &InputEvent, cx| {
                let q = entity.read(cx).value().to_string();
                this.sov.ws_query_len = q.chars().count();
                this.sov.ws_gen = this.sov.ws_gen.wrapping_add(1);
                this.sov.ws_active = 0;
                let g = this.sov.ws_gen;
                if this.sov.ws_query_len >= 1 {
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(WS_DEBOUNCE_MS)).await;
                        let _ = this.update(cx, |this, _| {
                            if this.sov.ws_gen == g {
                                host_link::request_workspace_symbols(this.tx.clone(), q);
                            }
                        });
                    })
                    .detach();
                } else {
                    this.sov.ws_results.clear();
                }
                cx.notify();
            }));
            self.sov.ws_input = Some(input);
        }
        // Запрос парити-гейта в инпут оверлея. Ставим ПОСЛЕ создания инпутов:
        // `emit` открывает оверлей и задаёт текст одним прогоном, инпута на
        // момент `apply` ещё нет
        if let Some((which, text)) = self.probe_query.take() {
            let target = match which {
                "fif" => self.sov.fif_input.clone(),
                "qo" => self.sov.quickopen_input.clone(),
                "ws" => self.sov.ws_input.clone(),
                "palette" => self.palette_input.clone(),
                "qp" => self.qp_input.clone(),
                _ => None,
            };
            if let Some(input) = target {
                input.update(cx, |st, cx| st.set_value(text, window, cx));
            }
        }
        // Модалка: автофокус Confirm при открытии и возврат фокуса при
        // закрытии (`ConfirmModal.tsx:46-56`)
        if self.modal.is_some() {
            if self.modal_autofocus_pending
                && crate::ui::focus_ring::focus_id("modal-confirm", window)
            {
                self.modal_autofocus_pending = false;
            }
        } else if let Some(back) = self.modal_focus_return.take() {
            crate::ui::focus_ring::focus_id(&back, window);
        }
        if std::mem::take(&mut self.probe_editor_find)
            && let Some(tab) = self.ed.editor_tabs.get(self.ed.editor_active)
        {
            window.focus(&tab.input.read(cx).focus_handle(cx));
            window.dispatch_action(Box::new(gpui_component::input::Search), cx);
        }
    }
}
