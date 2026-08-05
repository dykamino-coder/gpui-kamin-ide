//! Подсказки дропа плиток тулов по слотам.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::state::drag::move_item;
use crate::state::model::RootView;
use gpui::AnyElement;
use kamin_theme::Palette;
impl RootView {
    /// `isBlocked` (`useActivityDropTarget.ts:37`): тул тащат из ДРУГОГО
    /// слота, а здесь он уже пришпилен — дроп будет отвергнут.
    pub(crate) fn drop_blocked(&self, slot: crate::activity::PanelSlot) -> bool {
        self.tool_drag
            .as_ref()
            .filter(|d| d.started && d.over == Some(slot))
            .is_some_and(|d| d.src != slot && self.activity.state(slot).pinned.contains(&d.id))
    }

    /// Подсветка карты-цели (`[data-activity-drop]`): пунктир accent, а если
    /// тул в этом слоте уже закреплён — красное «дроп не пройдёт».
    pub(crate) fn card_drop_hint(
        &self,
        slot: crate::activity::PanelSlot,
        p: &Palette,
    ) -> Option<AnyElement> {
        let d = self.tool_drag.as_ref().filter(|d| d.started)?;
        if d.over != Some(slot) {
            return None;
        }
        // Самодроп на СВОЮ позицию (index == src или src+1) обнуляет
        // `overSlot` в самом dnd-сигнале (`activity-dnd.ts:69-71`), поэтому
        // `isOver` там false и цель НЕ подсвечивается. Правка ц.18 читала
        // только `useActivityDropTarget.ts:36` и была неверной (ревью ц.21).
        if d.src == slot && self.tool_drag_over_index(slot).is_none() {
            return None;
        }
        let blocked = d.src != slot && self.activity.state(slot).pinned.contains(&d.id);
        crate::ui::drop_hint::card_drop(!blocked, blocked, p)
    }

    /// id тула, который сейчас тащат ИЗ этого слота: исходная плитка/таб
    /// гаснет до .3 (`.tileDragging`/`.tabDragging` оригинала).
    pub(crate) fn tool_dragging_in(&self, slot: crate::activity::PanelSlot) -> Option<&str> {
        self.tool_drag
            .as_ref()
            .filter(|d| d.started && d.src == slot)
            .map(|d| d.id.as_str())
    }

    /// Индикатор вставки для стрипа слота: активный tool-drag над ним.
    pub(crate) fn tool_drag_over_index(&self, slot: crate::activity::PanelSlot) -> Option<usize> {
        let d = self
            .tool_drag
            .as_ref()
            .filter(|d| d.started && d.over == Some(slot))?;
        // Курсор в слоте, но не над плиткой → вставка В КОНЕЦ
        // (`activity-dnd.ts:44`), а не «плейсхолдера нет» (ревью ц.13)
        let idx = d
            .over_index
            .unwrap_or_else(|| self.activity.state(slot).pinned.len());
        // Дроп на собственное место — не цель: плейсхолдер рисовался поверх
        // самой перетаскиваемой плитки (`activity-dnd.ts:68-72`)
        if slot == d.src
            && let Some(from) = self.tab_index_of(slot, &d.id)
            && (idx == from || idx == from + 1)
        {
            return None;
        }
        Some(idx)
    }

    /// Завершение чип-жеста: клик без движения = активация; драг с целью =
    /// reorder (src встаёт на место over) + persist sessionOrder.
    pub(crate) fn commit_chip_drag(&mut self) {
        let Some(cd) = self.chip_drag.take() else {
            return;
        };
        if !cd.started {
            let _ = self.tx.try_send(ShellEvent::ActivateSession(cd.src));
            return;
        }
        if let Some(over) = cd.over
            && over != cd.src
            && let Some(snap) = self.sessions.as_ref()
        {
            // Текущий видимый порядок → src на место over → persist
            let mut ids: Vec<String> =
                crate::ui::session_tabs::ordered_chips(&snap.sessions, &self.chip_order)
                    .iter()
                    .map(|s| s.id.clone())
                    .collect();
            if let (Some(si), Some(di)) = (
                ids.iter().position(|i| *i == cd.src),
                ids.iter().position(|i| *i == over),
            ) {
                move_item(&mut ids, si, di);
                crate::layout_store::save_patch(serde_json::json!({ "sessionOrder": ids }));
                // Персист на ХОСТ (plan/99 п.39): sessions:reorder двигает
                // сессию в host-массиве — порядок переживает переустановку
                // локального layout.json и виден другим клиентам хоста.
                {
                    let moved = cd.src.clone();
                    let before = ids
                        .iter()
                        .position(|i| *i == moved)
                        .and_then(|i| ids.get(i + 1).cloned());
                    std::thread::spawn(move || {
                        if let Some(c) = crate::host_link::client() {
                            let _ = c.request(
                                "kamin:sessions:reorder",
                                vec![serde_json::json!(moved), serde_json::json!(before)],
                            );
                        }
                    });
                }
                self.chip_order = ids;
            }
        }
    }

    /// Спавн шелла по профилю; новый таб становится активным.
    pub(crate) fn spawn_shell(&mut self, profile: &crate::term::ShellProfile) {
        self.spawn_shell_in(profile, None);
    }

    /// То же с явным cwd (Open In ▸ Integrated Terminal); None = воркспейс.
    pub(crate) fn spawn_shell_in(
        &mut self,
        profile: &crate::term::ShellProfile,
        cwd: Option<String>,
    ) {
        let cwd = cwd.or_else(|| self.workspace.clone());
        match crate::term::TermSession::spawn(profile, cwd.as_deref(), self.tx.clone()) {
            Ok(t) => {
                self.term.terminals.push(t);
                self.term.term_active = self.term.terminals.len() - 1;
            }
            Err(e) => self.push_syslog("error", "terminal", &format!("spawn failed: {e}")),
        }
    }
}
