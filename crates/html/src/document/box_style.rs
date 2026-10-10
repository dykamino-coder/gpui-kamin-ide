//! Document wrappers retain box and counter scope metadata required by rendering.

use super::any_side;

/// Задаёт ли стиль обёртки её КОРОБКУ, а не только текст внутри.
///
/// `body { width: 600px; position: relative }` — обычный способ задать
/// систему координат странице, и снятая обёртка уносила её с собой: проценты
/// внутри считались от окна, а абсолютные дети — от другого предка.
pub(super) fn has_box_style(c: &crate::style::computed::Computed) -> bool {
    // Page margin counters read the document's counter scope. Removing these
    // wrappers would erase the directives before render_paged sees the tree.
    c.counter_reset.is_some()
        || c.counter_increment.is_some()
        || c.counter_set.is_some()
        || c.width.is_some()
        || c.height.is_some()
        || c.min_width.is_some()
        || c.min_height.is_some()
        || c.max_width.is_some()
        || c.max_height.is_some()
        || c.position.is_some()
        || c.display.is_some()
        || c.overflow_x.is_some()
        || c.overflow_y.is_some()
        // Вертикальное письмо задаёт ОСЬ ПОТОКА детей: без коробки её задать
        // некому, а дети об этом знают только через родителя. ЯВНОЕ
        // `horizontal-tb` — ось как у всех, коробки не требует: обёртка с
        // ним ломала схлопку полей p с body (wm-propagation-body-044).
        || c.vertical == Some(true)
        // Поля и внутренние отступы обёртки сдвигают ВСЁ содержимое: браузер
        // держит на `body` умолчание в 8 точек, и снятая обёртка уносила этот
        // сдвиг с собой — страница прижималась к краю окна.
        || any_side(&c.margin)
        || any_side(&c.padding)
        // Фон обёртки — её собственная краска: снятая обёртка уносила
        // `html { background: … }` с собой, и корневой фон пропадал
        // (страница выходила белой при пустом теле). Фоновая КАРТИНКА — та же
        // краска: область её раскладки — сама коробка корня
        // (background-size-document-root-vrl-*).
        || c.background.is_some()
        || c.gradient.is_some()
        || c.bg_image.is_some()
        || c.background_rcs.is_some()
}
