//! Элемент `VerticalText` — строка вертикального письма (`vertical-rl`, `vertical-lr`,
//! `sideways-*`): сам меряет горизонтально набранное содержимое, отдаёт раскладке размер с
//! переставленными осями и на отрисовке поворачивает его на четверть оборота. Здесь — поля
//! элемента и реализация `Wrap`; фазы в `layout`, `prepaint`, `painting`, настройки в
//! `vertical_style`.

mod layout;
mod painting;
mod prepaint;

use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};

/// Строка вертикального письма: `writing-mode: vertical-rl` и `vertical-lr`.
///
/// Поворота мало: у повёрнутого текста меняются местами ширина и высота, и
/// раскладка обязана считать их поменянными — иначе строка занимает место как
/// горизонтальная и наезжает на соседей. Поэтому это свой элемент: он
/// измеряет текст сам, отдаёт раскладке перевёрнутый размер, а на отрисовке
/// разворачивает содержимое на четверть оборота по часовой стрелке.
pub struct VerticalText {
    pub(crate) inline_measure: Option<super::InlineMeasure>,
    pub(crate) paint_context: std::rc::Rc<super::VerticalPaint>,
    /// Own layout node: the unrounded box edge anchors the turned content.
    pub(crate) layout_id: Option<LayoutId>,
    /// Clockwise turn frame from prepaint: unrounded block extent and the
    /// pre-turn block-axis phase that keeps pixel snapping physical.
    pub(crate) turn: Option<(Pixels, Pixels)>,
    pub(crate) child: AnyElement,
    /// Естественный размер содержимого до поворота.
    pub(crate) natural: gpui::Size<Pixels>,
    /// Потолок заявляемой высоты (см. `claiming_height`).
    pub(crate) claim_cap: Option<Pixels>,
    /// Ключ замера в текущем документе (текст абзаца и номер вхождения).
    pub(crate) key: Option<u64>,
    /// Предел строки от родителя: если строка УЖЕ помещается, высота
    /// заявляется честно — иначе гибкая ячейка считает коробку нулевой и
    /// `justify-content` уводит рисунок из виду (table-cell-align-005).
    pub(crate) fit_limit: Option<Pixels>,
    pub(crate) inline_constraint: Option<crate::style::computed::orthogonal::InlineConstraint>,
    pub(crate) inline_keyword: Option<crate::style::computed::orthogonal::InlineKeyword>,
    /// `writing-mode: sideways-lr` — поворот ПРОТИВ часовой стрелки.
    /// css-writing-modes-4, таблица Abstract-Physical Mapping: у `sideways-lr`
    /// line-left = НИЗ, line-right = ВЕРХ, over = ЛЕВО (у всех остальных
    /// вертикальных письмён line-left = верх, over = право). Blink различает
    /// эти два случая ровно так же — `paint/line_relative_rect.cc`:
    /// `AffineTransform(0, 1, -1, 0, …)` против `AffineTransform(0, -1, 1, 0, …)`.
    pub(crate) ccw: bool,
    /// Ячейка вертикальной таблицы: мерить содержимое по МИНИМАЛЬНОМУ
    /// вдоль строки, а не по максимальному. Тогда заявленная высота
    /// повёрнутой коробки — вклад ячейки в меру её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), и дорожку считает решётка, а не
    /// инлайн-размер всего стола. Ставится из `render/paragraph/vertical.rs` (`col_min`).
    pub(crate) col_min: bool,
    /// `vertical-lr` при повороте по часовой: строки поданы снизу вверх, и
    /// первая строка — ЛЕВАЯ колонка (у `vertical-rl` — правая).
    pub(crate) lr: bool,
    /// Первая строка для базовой по оси x: шрифт, кегль, высота строки
    /// (`None` — `normal`) и центральная ли доминантная базовая.
    pub(crate) first_line: Option<(gpui::Font, Pixels, Option<Pixels>, bool)>,
    pub(crate) values: Option<crate::document::state::ValueDoc>,
}

impl Wrap for VerticalText {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        self.request_layout_impl(window, cx)
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.prepaint_impl(bounds, window, cx)
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.paint_impl(bounds, window, cx)
    }
}
