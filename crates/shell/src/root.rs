//! Корневой вид: собственный титлбар 42px + тело (activity | resizable-колонки
//! sidebar/file/main/right) + статус-бар 24px.
//! Каждый регион трекается probe_area → kamin-probe tree/metric.

pub(crate) use crate::state::consts::{SUB_CLOSE_DELAY_MS, TERM_CELL_H, TERM_CELL_W, UI_FONT};
pub use crate::state::drag::{ChipDrag, DragState, TabDrag, ToolDrag};
pub use crate::state::model::RootView;
pub(crate) use crate::ui::webview_body::webview_body_dyn;

use gpui::{AnyElement, IntoElement};

use kamin_theme::{Palette, ThemeKind};

/// Какая панель растёт при клампе роста (clampGrowth оригинала).
#[derive(Clone, Copy)]
pub enum PanelSide {
    Sidebar,
    File,
}

/// Какой сплиттер тянут (drag-state в RootView).
#[derive(Clone, Copy, PartialEq)]
pub enum DragKind {
    /// Граница sidebar|main → ширина сайдбара
    Sidebar,
    /// Граница main|file → ширина file-колонки (у main остаток)
    MainFile,
    /// Граница file|right → трейд file↔right (main не трогаем)
    FileRight,
    /// Низ файловой колонки (centralBottom ratio)
    FileBottom,
    /// Низ колонки чата (mainSplit)
    MainBottom,
    /// Сплит правой колонки (не персистится — как в оригинале)
    RightSplit,
}

/// OS-тема (follow the OS): gpui window_appearance → ThemeKind.
pub fn system_theme(cx: &gpui::App) -> ThemeKind {
    match cx.window_appearance() {
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ThemeKind::Dark,
        gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => ThemeKind::Light,
    }
}

impl RootView {
    /// Тот же слой, но `deferred`: CSS-outline красится ПОСЛЕ потомков, а
    /// обычный первый ребёнок уходил под тело карты (ревью ц.14).
    pub(crate) fn card_drop_hint_top(
        &self,
        slot: crate::activity::PanelSlot,
        p: &Palette,
    ) -> Option<AnyElement> {
        self.card_drop_hint(slot, p)
            .map(|el| gpui::deferred(el).with_priority(20).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use crate::state::drag::move_item;

    #[test]
    fn move_right_takes_hovered_index() {
        let mut v = vec!["a", "b", "c", "d"];
        assert_eq!(move_item(&mut v, 0, 2), 2);
        assert_eq!(v, vec!["b", "c", "a", "d"]);
    }

    #[test]
    fn move_left_takes_hovered_index() {
        let mut v = vec!["a", "b", "c", "d"];
        assert_eq!(move_item(&mut v, 3, 1), 1);
        assert_eq!(v, vec!["a", "d", "b", "c"]);
    }

    #[test]
    fn name_validation() {
        use crate::file_names::file_name_error;
        assert!(file_name_error("ok.txt").is_none());
        assert!(file_name_error("").is_some());
        assert!(file_name_error("a/b").is_some());
        assert!(file_name_error("..").is_some());
        // `nameError` оригинала запрещённые символы и device-имена НЕ
        // отбивает — они проходят в fs (ревью ц.23)
        assert!(file_name_error("CON").is_none());
        assert!(file_name_error("what?").is_none());
        // …но при удалении такое имя идёт мимо корзины
        assert!(crate::file_names::is_reserved_name("CON"));
        assert!(crate::file_names::is_reserved_name("com3.txt"));
        assert!(!crate::file_names::is_reserved_name("common.txt"));
        assert!(file_name_error("COM0").is_none()); // COM0 не reserved
    }

    #[test]
    fn move_clamps_dst() {
        let mut v = vec!["a", "b"];
        assert_eq!(move_item(&mut v, 0, 99), 1);
        assert_eq!(v, vec!["b", "a"]);
    }
}
