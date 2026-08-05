//! Машина состояний hover-панели сессии/проекта.
//!
//! Строка и вынесенная панель — два независимых hitbox. Их enter/leave могут
//! приходить в любом порядке, поэтому общий `Option<id>` недостаточен.

use crate::host_link::HoverPillSource;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HoverPillUpdate {
    Ignored,
    Active,
    Inactive { id: String, generation: u64 },
}

/// Обновить одну из двух частей составного hover. Id сохраняется и на leave,
/// поэтому порядок `panel enter → anchor leave` не отличается от обратного.
pub(crate) fn update_hover_pill_state(
    visible: &mut Option<String>,
    anchor: &mut Option<String>,
    panel: &mut Option<String>,
    generation: &mut u64,
    id: String,
    source: HoverPillSource,
    hovered: bool,
) -> HoverPillUpdate {
    let (slot, other) = match source {
        HoverPillSource::Anchor => (anchor, panel),
        HoverPillSource::Panel => (panel, anchor),
    };

    if hovered {
        // Старый source мог исчезнуть с перерисованным элементом без leave.
        if other
            .as_deref()
            .is_some_and(|other_id| other_id != id.as_str())
        {
            *other = None;
        }
        if slot.as_deref() != Some(id.as_str()) {
            *slot = Some(id.clone());
        }
        *generation = (*generation).wrapping_add(1);
        *visible = Some(id);
        return HoverPillUpdate::Active;
    }

    // Запоздавший leave старого элемента не должен очищать новый source.
    if slot.as_deref() != Some(id.as_str()) {
        return HoverPillUpdate::Ignored;
    }
    *slot = None;
    *generation = (*generation).wrapping_add(1);

    let still_hovered =
        slot.as_deref() == Some(id.as_str()) || other.as_deref() == Some(id.as_str());
    if visible.as_deref() == Some(id.as_str()) && !still_hovered {
        HoverPillUpdate::Inactive {
            id,
            generation: *generation,
        }
    } else {
        HoverPillUpdate::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct State {
        visible: Option<String>,
        anchor: Option<String>,
        panel: Option<String>,
        generation: u64,
    }

    impl State {
        fn apply(&mut self, id: &str, source: HoverPillSource, hovered: bool) -> HoverPillUpdate {
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
    }

    #[test]
    fn panel_enter_before_anchor_leave_keeps_popover_open() {
        let mut state = State::default();
        state.apply("session-a", HoverPillSource::Anchor, true);
        state.apply("session-a", HoverPillSource::Panel, true);

        let result = state.apply("session-a", HoverPillSource::Anchor, false);

        assert_eq!(result, HoverPillUpdate::Active);
        assert_eq!(state.visible.as_deref(), Some("session-a"));
        assert_eq!(state.anchor, None);
        assert_eq!(state.panel.as_deref(), Some("session-a"));
    }

    #[test]
    fn anchor_leave_then_panel_enter_cancels_close_generation() {
        let mut state = State::default();
        state.apply("session-a", HoverPillSource::Anchor, true);
        let closing = state.apply("session-a", HoverPillSource::Anchor, false);
        let closing_generation = match closing {
            HoverPillUpdate::Inactive { generation, .. } => generation,
            other => panic!("expected inactive update, got {other:?}"),
        };

        state.apply("session-a", HoverPillSource::Panel, true);

        assert_ne!(state.generation, closing_generation);
        assert_eq!(state.visible.as_deref(), Some("session-a"));
        assert_eq!(state.panel.as_deref(), Some("session-a"));
    }

    #[test]
    fn panel_leave_does_not_close_while_anchor_is_hovered() {
        let mut state = State::default();
        state.apply("session-a", HoverPillSource::Anchor, true);
        state.apply("session-a", HoverPillSource::Panel, true);

        let result = state.apply("session-a", HoverPillSource::Panel, false);

        assert_eq!(result, HoverPillUpdate::Active);
        assert_eq!(state.visible.as_deref(), Some("session-a"));
        assert_eq!(state.anchor.as_deref(), Some("session-a"));
        assert_eq!(state.panel, None);
    }

    #[test]
    fn stale_anchor_leave_cannot_close_new_row() {
        let mut state = State::default();
        state.apply("session-a", HoverPillSource::Anchor, true);
        state.apply("session-b", HoverPillSource::Anchor, true);
        let before_stale_leave = state.generation;

        let result = state.apply("session-a", HoverPillSource::Anchor, false);

        assert_eq!(result, HoverPillUpdate::Ignored);
        assert_eq!(state.generation, before_stale_leave);
        assert_eq!(state.visible.as_deref(), Some("session-b"));
        assert_eq!(state.anchor.as_deref(), Some("session-b"));
    }
}
