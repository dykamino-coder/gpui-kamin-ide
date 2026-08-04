//! Геометрия миникарты: метрики строк и прыжок по клику.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use gpui::px;
use gpui_component::input::InputState;

/// `editor.rs:310`: `MINIMAP_FONT_SIZE: AbsoluteLength = px(2.)`.
pub(crate) const MM_FONT: f32 = 2.0;
/// Кегль редактора: `Input` при `Size::Medium` даёт `text_sm` = 0.875rem = 14px.
const ED_FONT: f32 = kamin_metrics::EDITOR_FONT_SIZE;
/// Высота строки редактора: `Input` ставит `LINE_HEIGHT = Rems(1.25)` = 20px
/// при rem 16 (`input.rs:256`).
pub(crate) const ED_LINE_H: f32 = 20.0;
/// Высота строки минимапы — НЕ на глаз, а формула Zed
/// `get_minimap_line_height`: берётся тот же `text_style` редактора, в нём
/// подменяется только `font_size` на `MINIMAP_FONT_SIZE`, и line-height
/// пересчитывается из него. То есть отношение «строка/кегль» сохраняется:
/// `mm_line = MM_FONT × (ED_LINE_H / ED_FONT)` = 2 × 20/13 ≈ 3.08px.
pub(crate) const MM_LINE_H: f32 = MM_FONT * (ED_LINE_H / ED_FONT);
pub(crate) const MM_WIDTH: f32 = 67.0;
pub(crate) const MIN_THUMB: f32 = 25.0;
pub(crate) struct Geom {
    pub total: f32,
    pub line_h: f32,
    pub vis_ed: f32,
    pub vis_mm: f32,
    pub mm_top: f32,
    pub scroll_row: f32,
    pub max_scroll: f32,
}
pub(crate) fn geom(st: &InputState, height: f32) -> Geom {
    let (total, lh) = st.minimap_metrics();
    let line_h = lh.map(f32::from).unwrap_or(20.0).max(1.0);
    let total = total as f32;
    let vis_ed = height / line_h;
    let vis_mm = height / MM_LINE_H;
    let scroll_row = (-f32::from(st.scroll_handle.offset().y) / line_h).max(0.0);
    // Предел прокрутки берём у САМОГО редактора: его контент выше текста на
    // нижний запас (`BOTTOM_MARGIN_ROWS`), поэтому `total − vis_ed` меньше
    // настоящего максимума — палка упиралась в низ, пока колесо ещё ехало, а
    // драг минимапы не доводил до конца файла.
    let content_h = f32::from(st.scroll_content_height());
    let max_scroll = ((content_h - height).max(0.0) / line_h).max(0.0);
    // Zed: minimap_top = scroll_pct * (total - visible_minimap_lines)
    let mm_top = if max_scroll > 0.0 && total > vis_mm {
        (scroll_row / max_scroll) * (total - vis_mm).max(0.0)
    } else {
        0.0
    };
    Geom {
        total,
        line_h,
        vis_ed,
        vis_mm,
        mm_top,
        scroll_row,
        max_scroll,
    }
}
/// Прыжок скролла по y-координате внутри минимапы (клик/драг): кликнутая
/// строка центрируется во вьюпорте (как Zed paint_minimap).
pub(crate) fn jump_to(st: &InputState, local_y: f32, height: f32) -> gpui::Point<gpui::Pixels> {
    let g = geom(st, height);
    let row = g.mm_top + local_y / MM_LINE_H - g.vis_ed / 2.0;
    let target = row.clamp(0.0, g.max_scroll);
    gpui::point(px(0.0), px(-(target * g.line_h)))
}
