//! Ховер ручек сплиттеров: кадр окна только на настоящую смену показа.
//!
//! Ручки помнят ховер в `hovered_handle`, а показывает ручку `handle_show`:
//! под драгом — сам драг, вне драга — ховер. Прежде каждый `on_hover` ручки
//! безусловно звал `notify`. Под драгом ручка едет за мышью и то попадает
//! под курсор, то нет: ховер переключался на каждом кадре, и каждый щелчок
//! давал окну лишний кадр — до ~60/с от одной ручки (INC-2026-0001, замер
//! драга: `notify@state/columns/wraps.rs` в `[dirty]`).

use crate::root::RootView;

impl RootView {
    /// Запомнить ховер ручки `id`; `true` — показ ручки изменился, нужен кадр.
    pub(crate) fn hover_handle(&mut self, id: &'static str, hovered: bool) -> bool {
        let next = next_hovered(self.hovered_handle, id, hovered);
        if next == self.hovered_handle {
            return false;
        }
        self.hovered_handle = next;
        self.drag.is_none()
    }
}

/// Ховер после события ручки `id`. Уход с ручки снимает ховер, только если
/// он принадлежал ей: поздний `false` соседней ручки не гасит текущую.
fn next_hovered(
    current: Option<&'static str>,
    id: &'static str,
    hovered: bool,
) -> Option<&'static str> {
    match (hovered, current) {
        (true, _) => Some(id),
        (false, Some(cur)) if cur == id => None,
        (false, cur) => cur,
    }
}

#[cfg(test)]
mod tests {
    use super::next_hovered;

    #[test]
    fn hover_follows_the_handle_and_ignores_a_late_leave_of_another() {
        assert_eq!(next_hovered(None, "a", true), Some("a"));
        assert_eq!(next_hovered(Some("a"), "a", true), Some("a"));
        assert_eq!(next_hovered(Some("a"), "a", false), None);
        assert_eq!(next_hovered(Some("b"), "a", false), Some("b"));
        assert_eq!(next_hovered(None, "a", false), None);
    }
}
