//! Собственная краска коробки с `box-decoration-break: clone` в записи главной копии
//! разрезанного ребёнка стопки: фон, рамка и слои украшений коробки принадлежат каждому
//! её фрагменту отдельно (css-break-4 §break-decoration: «each box fragment is
//! independently wrapped with the border, padding … The background is drawn
//! independently in each fragment»), их рисует стопка по кускам; непрерывная коробка
//! главной копии в записи свою краску не даёт. Вне записи коробка рисуется как обычно.

use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, Pixels, Window};
use std::collections::HashSet;
use std::rc::Rc;

/// Узлы, чья собственная краска в этой записи скрыта.
struct Hidden(Rc<HashSet<u64>>);

fn hidden(id: u64, window: &Window) -> bool {
    window
        .render_context::<Hidden>()
        .is_some_and(|h| h.0.contains(&id))
}

/// Запись `record`, в которой собственная краска узлов `ids` скрыта.
pub(crate) fn with_hidden<R>(
    ids: Rc<HashSet<u64>>,
    window: &mut Window,
    record: impl FnOnce(&mut Window) -> R,
) -> R {
    window.with_render_context(Some(Rc::new(Hidden(ids))), record)
}

/// Фон и рамка коробки `id`: в записи со скрытием не рисуются.
pub(crate) fn box_painter(id: u64) -> gpui::BoxPainter {
    gpui::BoxPainter(Rc::new(move |_, _, window, _| hidden(id, window)))
}

/// Слой украшения коробки `id` (тень, контур, картинки фона): в записи со скрытием не рисуется.
pub(crate) fn layer(id: u64, child: AnyElement) -> AnyElement {
    Layer { id, child }.hooked()
}

struct Layer {
    id: u64,
    child: AnyElement,
}

impl Wrap for Layer {
    fn paint_boundary(&self) -> bool {
        false
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if !hidden(self.id, window) {
            self.child.paint(window, cx);
        }
    }
}
