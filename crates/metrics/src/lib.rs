//! Единственный источник размеров (правило №0, plan/22).
//! Значения ДОСЛОВНО из kamin-ide 0.2.87: layout-tokens.css + constants.ts (plan/20 §6, plan/30).
//! Ни одного магического числа в компонентах — только ссылки сюда.

pub mod layout_math;

// ── Окно (plan/10) ──────────────────────────────────────────────
pub const WINDOW_DEFAULT_WIDTH: f32 = 1400.0;
pub const WINDOW_DEFAULT_HEIGHT: f32 = 900.0;
pub const WINDOW_MIN_WIDTH: f32 = 800.0;
pub const WINDOW_MIN_HEIGHT: f32 = 600.0;

// ── Регионы (layout-tokens.css) ─────────────────────────────────
pub const TITLEBAR_HEIGHT: f32 = 42.0;
pub const ACTIVITY_BAR_WIDTH: f32 = 48.0; // --layout-activity-bar-width (ревью ц.1: было 44)
pub const PRIMARY_SIDEBAR_WIDTH: f32 = 280.0;
pub const PRIMARY_SIDEBAR_MIN_WIDTH: f32 = 200.0;
/// Пол вьюпорта адаптера (`layout-autosave.ts:170-171`): свёрнутое окно
/// рапортует крошечные размеры, и такой кадр обрабатывать нельзя.
pub const VIEWPORT_MIN_FLOOR_W: f32 = 500.0;
pub const VIEWPORT_MIN_FLOOR_H: f32 = 400.0;
/// `SIDEBAR_MIN_WIDTH_PX` — минимум, ниже которого адаптер сайдбар не жмёт.
pub const SIDEBAR_MIN_WIDTH: f32 = 100.0;
/// `RESIZE_SETTLE_MS` — адаптер реагирует на УСТОЯВШИЙСЯ размер окна.
pub const RESIZE_SETTLE: std::time::Duration = std::time::Duration::from_millis(80);

pub const AUXILIARY_BAR_WIDTH: f32 = 280.0;
pub const PANEL_HEIGHT: f32 = 220.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
pub const SECTION_HEADER_HEIGHT: f32 = 36.0;
pub const PANEL_TABS_HEIGHT: f32 = 32.0;
pub const PALETTE_WIDTH: f32 = 640.0;
pub const PALETTE_TOP_OFFSET: f32 = 84.0;
pub const ICON_BUTTON_ROUND: f32 = 28.0;
pub const ICON_BUTTON_SQUARE: f32 = 22.0;
pub const ICON_BUTTON_TITLEBAR: f32 = 36.0;
pub const EDITOR_TABS_HEIGHT: f32 = 35.0;
pub const EDITOR_TAB_HEIGHT: f32 = 30.0;

// ── Меж-панельная геометрия (.body, plan/20 §6) ─────────────────
pub const BODY_GAP: f32 = 8.0;
pub const BODY_GUTTER_X: f32 = 4.0;

// ── Радиусы ─────────────────────────────────────────────────────
pub const RADIUS_XS: f32 = 4.0;
pub const RADIUS_SM: f32 = 8.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_LG: f32 = 16.0;

// ── Шрифтовая шкала (plan/20 §4) ────────────────────────────────
pub const FS_XS: f32 = 11.0;
pub const FS_SM: f32 = 12.0;
pub const FS_MD: f32 = 13.0;
pub const FS_LG: f32 = 16.0;
pub const FS_XL: f32 = 22.0;
pub const EDITOR_FONT_SIZE: f32 = 13.0;

// ── Spacing-шкала ───────────────────────────────────────────────
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 20.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_7: f32 = 28.0;

// ── Сплиты/клампы (constants.ts, plan/30) ───────────────────────
pub const PANEL_MIN_SIZE: f32 = 100.0;
pub const SIDEBAR_DEFAULT: f32 = 270.0;
pub const FILE_PANEL_DEFAULT: f32 = 360.0;
pub const FILE_BOTTOM_DEFAULT: f32 = 180.0;
pub const RIGHT_PANEL_DEFAULT: f32 = 280.0;
pub const MAIN_SPLIT_DEFAULT: f32 = 0.7;
pub const MAIN_SPLIT_MIN: f32 = 0.2;
pub const MAIN_SPLIT_MAX: f32 = 0.85;
pub const RIGHT_SPLIT_DEFAULT: f32 = 0.55;
pub const RIGHT_SPLIT_MIN: f32 = 0.15;
pub const RIGHT_SPLIT_MAX: f32 = 0.85;
pub const FILE_RATIO_MIN: f32 = 0.05;
pub const FILE_RATIO_MAX: f32 = 0.6;
pub const BOTTOM_RATIO_MIN: f32 = 0.1;
/// `BOTTOM_PANE_MIN_HEIGHT_PX` (`constants.ts`) — жёсткий минимум нижней
/// карты файловой колонки в ПИКСЕЛЯХ (у нас была только доля, ревью ц.19)
pub const BOTTOM_PANE_MIN_HEIGHT: f32 = 100.0;
pub const BOTTOM_RATIO_MAX: f32 = 0.8;

#[cfg(test)]
mod tests {
    use super::*;

    /// Смоук против plan/22 «что переносится дословно».
    #[test]
    fn layout_tokens_verbatim() {
        assert_eq!(TITLEBAR_HEIGHT, 42.0);
        assert_eq!(ACTIVITY_BAR_WIDTH, 48.0);
        assert_eq!(PRIMARY_SIDEBAR_WIDTH, 280.0);
        assert_eq!(STATUS_BAR_HEIGHT, 24.0);
        assert_eq!(RADIUS_LG, 16.0);
        assert_eq!(BODY_GAP, 8.0);
        assert_eq!(BODY_GUTTER_X, 4.0);
        assert_eq!(MAIN_SPLIT_DEFAULT, 0.7);
        assert_eq!(RIGHT_SPLIT_DEFAULT, 0.55);
    }
}
