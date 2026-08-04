//! Обработка событий: Каналы вывода, системный лог, проблемы, экспорт и импорт.
//!
//! Вызывается диспетчером `state/events/dispatch.rs` — он и решает,
//! чьё это событие, по варианту `ShellEvent`. Тела армов перенесены
//! из `root.rs` дословно.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Каналы вывода, системный лог, проблемы, экспорт и импорт.
    pub(crate) fn apply_output(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Ed(EdEvent::CopyToClipboard(text)) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            }
            ShellEvent::Ed(EdEvent::CopyRelativePath(path)) => {
                let rel = self
                    .workspace
                    .as_deref()
                    .and_then(|root| {
                        path.strip_prefix(root)
                            .map(|r| r.trim_start_matches(['\\', '/']).to_string())
                    })
                    .unwrap_or_else(|| path.clone());
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(rel));
            }
            ShellEvent::Cz(CzEvent::DesignSample(action)) => {
                use crate::ui::design_samples::DesignAction;
                match action {
                    DesignAction::ToggleDropdown => {
                        self.design.dropdown_open = !self.design.dropdown_open;
                    }
                    // Клик по пункту закрывает меню (как в оригинале),
                    // а по чекбоксу — НЕ закрывает (рецепт LayoutToggles).
                    DesignAction::Pick(id) => {
                        self.design.picked = id;
                        self.design.dropdown_open = false;
                    }
                    DesignAction::PickStripTab(id) => self.design.strip_tab = id,
                    DesignAction::PickColumnTile(id) => self.design.column_tile = id,
                    DesignAction::ToggleCheck(i) => {
                        if let Some(v) = self.design.checks.get_mut(i) {
                            *v = !*v;
                        }
                    }
                    // `onToggle` + `onSelect` семпла: папка раскрывается,
                    // выделение переезжает на кликнутый узел
                    DesignAction::TreeClick(id, is_dir) => {
                        if is_dir && !self.design.tree_expanded.remove(&id) {
                            self.design.tree_expanded.insert(id.clone());
                        }
                        self.design.tree_selected = id;
                    }
                }
            }
            ShellEvent::Cz(CzEvent::PushSysLog(msg)) => self.push_syslog("info", "probe", &msg),
            ShellEvent::Cz(CzEvent::OutputEvent(ext, channel, op, text)) => {
                let show = self.output.apply(&ext, &channel, &op, text.as_deref());
                if show {
                    // outputChannel.show() → Customize→Logs (как оригинал)
                    self.cz.customize_open = true;
                    self.cz.customize_panel = "logs";
                }
            }
            ShellEvent::Cz(CzEvent::ClearOutputChannel(key)) => self.output.clear(&key),
            ShellEvent::Cz(CzEvent::SelectOutputChannel(key)) => {
                self.output.active = Some(key);
                // Смена канала сбрасывает фильтр (`LogsPanel.tsx:39`)
                self.cz.log_filter_value.clear();
                self.cz.log_filter_gen = self.cz.log_filter_gen.wrapping_add(1);
                self.cz.log_filter_input = None;
                self.cz.log_filter_sub = None;
                self.cz.log_filter_panel = "";
            }
            ShellEvent::Cz(CzEvent::SystemLog(level, source, message)) => {
                self.push_syslog(level, &source, &message);
            }
            ShellEvent::Cz(CzEvent::SetSysLogLevel(level)) => {
                self.cz.syslog_level = level;
            }
            ShellEvent::Cz(CzEvent::ClearSystemLog) => self.cz.system_log.clear(),
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
