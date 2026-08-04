//! Оверлеи: палитра, quick-open, quick-pick, модалки, тосты, тултипы,
//! меню и поповеры.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host_link::{self, ShellEvent};
use crate::state::fs_ops::respond_quick_pick;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Оверлеи: палитра, quick-open, quick-pick, модалки, тосты, тултипы.
    pub(crate) fn apply_overlays(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        match event {
            ShellEvent::TooltipShow(text, x, y) => {
                if crate::state::drag_flag::active() {
                    return; // драг ручки: тултипы молчат (мерцали под мышью)
                }
                let text = gpui::SharedString::from(text);
                // Позиция фиксируется ПРИ ПОКАЗЕ (иначе бегает за мышью);
                // повторные show того же текста позицию не двигают
                let same = self.tooltip_live.as_ref().is_some_and(|(t, _)| *t == text);
                if !same {
                    self.tooltip_live = Some((text, (x, y)));
                }
            }
            ShellEvent::TooltipHide => {
                self.tooltip_live = None;
            }
            ShellEvent::CloseInputOverlays => {
                self.palette_open = false;
                self.palette_input = None;
                self.palette_sub = None;
                self.sov.quickopen_open = false;
                self.sov.quickopen_input = None;
                self.sov.quickopen_sub = None;
                self.sov.quickopen_results.clear();
                self.sov.fif_open = false;
                self.sov.fif_input = None;
                self.sov.fif_sub = None;
                self.sov.fif_results.clear();
                self.sov.fif_query_len = 0;
                self.sov.fif_busy = false;
                self.sov.ws_open = false;
                self.close_ws();
            }
            ShellEvent::Cz(CzEvent::ExplorerMenuItems(items)) => self.explorer_menu = items,
            ShellEvent::QuickPickShow(req_id, items, options) => {
                // Новый пик поверх невыбранного старого: старый отменяем
                if let Some(old) = self.quick_pick.take() {
                    respond_quick_pick(old.req_id, None);
                }
                self.quick_pick = Some(crate::ui::quick_pick::QuickPickState::from_request(
                    req_id, &items, &options,
                ));
                self.qp_input = None;
            }
            ShellEvent::CloseToolTabMenu => {
                self.tool_tab_menu = None;
                self.tool_menu_sub = false;
            }
            ShellEvent::ToolMenuSub(open) => self.tool_menu_sub = open,
            ShellEvent::Cz(CzEvent::InstallVsixPrompt) => {
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                let tx = self.tx.clone();
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(paths))) = rx.await
                        && let Some(path) = paths.first()
                    {
                        let path = path.to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            let ok = host_link::client()
                                .map(|c| {
                                    c.request(
                                        "kamin:extensions:installVsix",
                                        vec![serde_json::json!(path)],
                                    )
                                    .is_ok()
                                })
                                .unwrap_or(false);
                            let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                                id: "vsix-install".into(),
                                severity: if ok { "success" } else { "error" }.into(),
                                title: None,
                                message: if ok {
                                    "Extension installed".into()
                                } else {
                                    "VSIX install failed".into()
                                },
                                actions: Vec::new(),
                                sticky: false,
                            }));
                            host_link::request_status(tx.clone());
                            crate::state::fs_ops::request_extensions_pub(tx);
                        });
                    }
                })
                .detach();
            }
            ShellEvent::QuickPickToggle(i) => {
                if let Some(qp) = self.quick_pick.as_mut()
                    && !qp.checked.remove(&i)
                {
                    qp.checked.insert(i);
                }
            }
            ShellEvent::QuickPickResolve(req_id, indices) => {
                self.quick_pick = None;
                self.qp_input = None;
                respond_quick_pick(req_id, indices);
            }
            ShellEvent::ConfirmModal => {
                if let Some(modal) = self.modal.take() {
                    self.run_modal_action(modal, None);
                }
            }
            ShellEvent::ConfirmModalInput(value) => {
                if let Some(modal) = self.modal.take() {
                    self.run_modal_action(modal, Some(value));
                }
            }
            ShellEvent::CloseModal => self.modal = None,
            ShellEvent::CloseToolPicker => self.tool_picker = None,
            ShellEvent::TogglePalette => {
                self.palette_open = !self.palette_open;
                if self.palette_open {
                    host_link::request_commands(self.tx.clone());
                } else {
                    self.palette_input = None;
                    self.palette_sub = None;
                }
            }
            ShellEvent::ClosePalette => {
                self.palette_open = false;
                self.palette_input = None;
                self.palette_sub = None;
            }
            ShellEvent::PaletteGate(entries, ctx) => {
                self.palette_menu = entries;
                self.context_keys = ctx.into_iter().collect();
            }
            ShellEvent::Toast(t) => {
                // Каунтдаун 8s с паузой на ховере: wall-clock в ToastTimer
                // (полосу 60fps ведёт repeat-анимация в toast_card), фоновый
                // поток лишь проверяет истечение и шлёт Dismiss (slide-out).
                let timer = std::sync::Arc::new(crate::ui::toasts::ToastTimer::default());
                if !t.sticky {
                    let tx = self.tx.clone();
                    let id = t.id.clone();
                    let tm = timer.clone();
                    std::thread::spawn(move || {
                        use std::sync::atomic::Ordering;
                        loop {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if tm.closing.load(Ordering::Relaxed) {
                                break;
                            }
                            if tm.active_ms() >= crate::ui::toasts::TOAST_MS {
                                let _ = tx.try_send(ShellEvent::DismissToast(id));
                                break;
                            }
                        }
                    });
                }
                self.toast_timers.insert(t.id.clone(), timer);
                self.toasts.retain(|x| x.id != t.id);
                self.toasts.push(t);
            }
            ShellEvent::DismissToast(id) => {
                // Двухфазно: closing → slide-out (toast_card) → ToastGone
                // удаляет. Повторный Dismiss во время slide-out глушится swap'ом.
                if let Some(tm) = self.toast_timers.get(&id) {
                    if !tm.closing.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        let tx = self.tx.clone();
                        std::thread::spawn(move || {
                            // Слак поверх slide-out: кадр с финальной фазой
                            // анимации должен успеть отрисоваться до удаления.
                            const TOAST_GONE_SLACK_MS: u64 = 30;
                            std::thread::sleep(std::time::Duration::from_millis(
                                crate::ui::toasts::TOAST_OUT_MS + TOAST_GONE_SLACK_MS,
                            ));
                            let _ = tx.try_send(ShellEvent::ToastGone(id));
                        });
                    }
                } else {
                    self.toasts.retain(|x| x.id != id);
                }
            }
            ShellEvent::ToastGone(id) => {
                self.toast_timers.remove(&id);
                self.toasts.retain(|x| x.id != id);
            }
            ShellEvent::WebMenu(menu) => {
                crate::web::set_menu_open(menu.is_some());
                self.web_menu = menu;
            }
            ShellEvent::WebMenuCmd(view, cmd) => {
                crate::web::execute_menu_command(&view, cmd);
                crate::web::set_menu_open(false);
                self.web_menu = None;
            }
            ShellEvent::FocusStep(forward) => {
                // `apply` без окна — переход делаем на ближайшем кадре
                self.pending_focus_step = Some(forward);
                cx.notify();
            }
            ShellEvent::OverlayMove(delta) => self.move_overlay_active(delta),
            ShellEvent::ToggleLayoutPopover => {
                self.close_popovers_except("layout");
                self.layout_popover = !self.layout_popover;
                self.appearance_popover = false;
            }
            ShellEvent::ToggleAppearancePopover => {
                self.close_popovers_except("appearance");
                self.appearance_popover = !self.appearance_popover;
                self.layout_popover = false;
            }
            ShellEvent::OverlayRowHover(kind, idx) => match kind {
                "qo" => self.sov.qo_active = idx,
                "fif" => self.sov.fif_active = idx,
                _ => self.sov.ws_active = idx,
            },
            ShellEvent::SetOverlayQuery(which, text) => {
                self.probe_query = Some((which, text));
            }
            ShellEvent::ExternalToast(t) => {
                let opts = crate::toast::ToastOpts {
                    kind: crate::toast::ToastKind::parse(&t.severity),
                    title: t.title.unwrap_or_default(),
                    message: t.message,
                    sticky: t.sticky,
                    actions: t.actions,
                };
                cx.defer(move |cx| crate::toast::show(opts, cx));
            }
            ShellEvent::ToggleWorkspaceSymbols => {
                self.sov.ws_open = !self.sov.ws_open;
                if !self.sov.ws_open {
                    self.close_ws();
                }
            }
            ShellEvent::CloseWorkspaceSymbols => {
                self.sov.ws_open = false;
                self.close_ws();
            }
            ShellEvent::WorkspaceSymbolsResults(hits) => self.sov.ws_results = hits,
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
