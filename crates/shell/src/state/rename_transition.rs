//! Атомарный переход hover → inline rename (BR-29).
//!
//! Раньше `BeginRename` закрывал только контекст-меню и ставил
//! `renaming_session`, а hover-панель действий жила своей жизнью: rename-ветка
//! `session_row()` не вешает `on_hover`/`anchor_probe`, поэтому заменённый
//! hovered node мог не прислать leave, `hover_pill` оставался, а overlay
//! продолжал рисовать пилюлю по устаревшей geometry якоря поверх инпута.
//!
//! Теперь один helper владеет teardown: перед показом инпута он
//! инвалидирует pending close generation, очищает оба hover source и
//! связанную geometry якоря, а hover-enter для переименовываемой строки
//! игнорируется до commit/cancel. Rename не зависит от случайного
//! `mouseleave` или глобального mouse-down listener.

use crate::state::model::RootView;

/// Ссылки на составное hover-состояние `RootView` (без gpui-зависимостей —
/// тестируемо чистыми функциями).
pub(crate) struct HoverSlots<'a> {
    pub visible: &'a mut Option<String>,
    pub anchor: &'a mut Option<String>,
    pub panel: &'a mut Option<String>,
    pub generation: &'a mut u64,
}

/// Снять hover-состояние атомарно. Возвращает новое поколение: отложенное
/// закрытие (grace-таймер) со старым поколением больше ничего не трогает.
pub(crate) fn dismiss_hover(slots: HoverSlots<'_>) -> u64 {
    *slots.visible = None;
    *slots.anchor = None;
    *slots.panel = None;
    *slots.generation = slots.generation.wrapping_add(1);
    *slots.generation
}

/// Hover-enter для строки, которая сейчас переименовывается, игнорируется:
/// у rename-строки нет якоря, и запоздавший enter соседнего hitbox не должен
/// вернуть пилюлю поверх инпута. Leave и чужие строки проходят как обычно.
pub(crate) fn hover_allowed(renaming: Option<&str>, id: &str, hovered: bool) -> bool {
    !(hovered && renaming == Some(id))
}

impl RootView {
    /// Закрыть hover-панель действий и сбросить geometry якоря.
    pub(crate) fn dismiss_hover_pill(&mut self) -> u64 {
        crate::ui::sessions::pill::clear_pill_anchor();
        dismiss_hover(HoverSlots {
            visible: &mut self.hover_pill,
            anchor: &mut self.hover_pill_anchor,
            panel: &mut self.hover_pill_panel,
            generation: &mut self.hover_pill_gen,
        })
    }

    /// Перейти в inline rename строки `id`: сначала teardown hover
    /// (generation, оба source, geometry), затем показать инпут. Все входы —
    /// кнопка fly-out, double-click, F2, контекст-меню и probe — идут сюда.
    pub(crate) fn begin_rename(&mut self, id: String) {
        self.dismiss_hover_pill();
        self.session_menu = None;
        self.renaming_session = Some(id);
        self.rename_input = None; // создаётся лениво в render (нужен window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_link::HoverPillSource;
    use crate::state::hover_pill::{HoverPillUpdate, update_hover_pill_state};

    #[derive(Default)]
    struct Hover {
        visible: Option<String>,
        anchor: Option<String>,
        panel: Option<String>,
        generation: u64,
    }

    impl Hover {
        fn hover(&mut self, id: &str, source: HoverPillSource, hovered: bool) -> HoverPillUpdate {
            update_hover_pill_state(
                &mut self.visible,
                &mut self.anchor,
                &mut self.panel,
                &mut self.generation,
                id.into(),
                source,
                hovered,
            )
        }
        fn dismiss(&mut self) -> u64 {
            dismiss_hover(HoverSlots {
                visible: &mut self.visible,
                anchor: &mut self.anchor,
                panel: &mut self.panel,
                generation: &mut self.generation,
            })
        }
    }

    #[test]
    fn rename_from_flyout_clears_both_sources_and_bumps_generation() {
        let mut h = Hover::default();
        h.hover("s1", HoverPillSource::Anchor, true);
        h.hover("s1", HoverPillSource::Panel, true); // курсор на кнопке пилюли
        let before = h.generation;
        let generation = h.dismiss();
        assert_eq!(h.visible, None);
        assert_eq!(h.anchor, None);
        assert_eq!(h.panel, None);
        assert_ne!(generation, before);
    }

    #[test]
    fn delayed_leave_of_replaced_node_cannot_reopen_or_close_anything() {
        let mut h = Hover::default();
        h.hover("s1", HoverPillSource::Anchor, true);
        h.dismiss();
        // Заменённый node присылает leave уже после перехода в rename.
        assert_eq!(
            h.hover("s1", HoverPillSource::Anchor, false),
            HoverPillUpdate::Ignored
        );
        assert_eq!(h.visible, None);
    }

    #[test]
    fn pending_grace_close_with_old_generation_is_stale() {
        let mut h = Hover::default();
        h.hover("s1", HoverPillSource::Anchor, true);
        let closing = h.hover("s1", HoverPillSource::Anchor, false);
        let HoverPillUpdate::Inactive { generation, .. } = closing else {
            panic!("expected a pending close");
        };
        h.hover("s1", HoverPillSource::Anchor, true); // вернулся до grace
        h.dismiss(); // F2 / double-click
        assert_ne!(
            h.generation, generation,
            "grace-таймер старого поколения не должен сработать"
        );
    }

    #[test]
    fn hover_enter_on_the_renaming_row_is_ignored_until_commit_or_cancel() {
        assert!(!hover_allowed(Some("s1"), "s1", true));
        assert!(hover_allowed(Some("s1"), "s1", false));
        assert!(hover_allowed(Some("s1"), "s2", true));
        assert!(hover_allowed(None, "s1", true));
    }

    #[test]
    fn switching_session_mid_transition_keeps_the_hover_state_clean() {
        let mut h = Hover::default();
        h.hover("s1", HoverPillSource::Anchor, true);
        h.dismiss();
        // Другая строка после rename: обычный hover снова работает.
        assert_eq!(
            h.hover("s2", HoverPillSource::Anchor, true),
            HoverPillUpdate::Active
        );
        assert_eq!(h.visible.as_deref(), Some("s2"));
    }
}
