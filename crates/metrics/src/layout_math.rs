//! Порт layout-ratios.ts (1:1): file-panel и его нижняя панель масштабируются
//! с вьюпортом — храним RATIO от side-area, в пиксели на лету.

/// Bridge CHAT_MIN_WIDTH — ниже этой ширины чата панели перестают
/// пропорционально расти (layout-ratios.ts:17).
pub const CHAT_MIN_WIDTH_PX: f32 = 550.0;
pub const VIEWPORT_REFERENCE_RATIO_FLOOR: f32 = 1.0;

/// sideArea = max(SIDEBAR_MIN 100, viewportW − CHAT_MIN 550)
pub fn side_area(viewport_w: f32) -> f32 {
    (viewport_w - CHAT_MIN_WIDTH_PX).max(crate::PANEL_MIN_SIZE)
}

/// width = max(minPx, round(ratio · sideArea))
pub fn width_from_ratio(ratio: f32, min_px: f32, viewport_w: f32) -> f32 {
    (ratio * side_area(viewport_w)).round().max(min_px)
}

/// ratio = clamp(width / sideArea, 0.05..0.6)
pub fn ratio_from_width(width_px: f32, viewport_w: f32) -> f32 {
    let area = side_area(viewport_w).max(VIEWPORT_REFERENCE_RATIO_FLOOR);
    (width_px / area).clamp(crate::FILE_RATIO_MIN, crate::FILE_RATIO_MAX)
}

/// height = max(minPx, round(ratio · available))
pub fn height_from_ratio(ratio: f32, available: f32, min_px: f32) -> f32 {
    (ratio * available).round().max(min_px)
}

/// clampGrowth (centre-column-width.ts): предел роста панели — центральной
/// колонке должно остаться >= MAIN_MIN (PANEL_MIN_SIZE = 100), а НЕ CHAT_MIN
/// 550: 550 участвует только в ratio-математике (side_area), но не ограничивает
/// драг. `occupied_px` — сумма ширин ОСТАЛЬНЫХ фиксированных панелей.
pub fn max_growth_width(viewport_w: f32, occupied_px: f32) -> f32 {
    let chrome = crate::BODY_GUTTER_X * 2.0 + crate::ACTIVITY_BAR_WIDTH;
    (viewport_w - chrome - occupied_px - crate::PANEL_MIN_SIZE)
        .max(crate::PANEL_MIN_SIZE)
        .round()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_area_floors_at_panel_min() {
        assert_eq!(side_area(1400.0), 850.0);
        assert_eq!(side_area(600.0), 100.0); // 600-550=50 → floor 100
    }

    #[test]
    fn width_round_trips_through_ratio() {
        let vw = 1400.0;
        let w = width_from_ratio(0.478, crate::PANEL_MIN_SIZE, vw);
        assert_eq!(w, (0.478f32 * 850.0).round());
        let r = ratio_from_width(w, vw);
        assert!((r - 0.478).abs() < 0.001);
    }

    #[test]
    fn growth_limit_is_centre_min_not_chat_min() {
        // vw 1400, заняты sidebar 256 + right 348 → центру остаётся 100
        let max_file = max_growth_width(1400.0, 256.0 + 348.0);
        assert_eq!(max_file, 1400.0 - 56.0 - 604.0 - 100.0); // 640
        // Ключ: предел НЕ равен старому «vw − CHAT_MIN 550»
        assert!(max_file > 1400.0 - 550.0 - 604.0);
        // Центр реально может ужаться до 100: панель + остальные + chrome
        let centre = 1400.0 - 56.0 - 604.0 - max_file;
        assert_eq!(centre, crate::PANEL_MIN_SIZE);
    }

    #[test]
    fn growth_limit_floors_at_panel_min() {
        // Тесное окно: предел не уходит ниже PANEL_MIN_SIZE
        assert_eq!(max_growth_width(400.0, 300.0), crate::PANEL_MIN_SIZE);
    }

    #[test]
    fn ratio_clamps_to_bounds() {
        assert_eq!(ratio_from_width(10.0, 1400.0), crate::FILE_RATIO_MIN);
        assert_eq!(ratio_from_width(2000.0, 1400.0), crate::FILE_RATIO_MAX);
    }
}
