//! Общие callbacks: геометрия и метаданные явные, правила принадлежат хозяину.

use crate::{AnyElement, App, Bounds, Pixels, Style, Window};
use std::{any::Any, rc::Rc};

/// Геометрия и непрозрачные метаданные в обычной точке рисования детей бокса.
pub struct ChildPaintContext<'a> {
    /// Итоговый стиль рисуемого бокса.
    pub style: &'a Style,
    /// Границы размещения бокса.
    pub bounds: Bounds<Pixels>,
    /// Округлённые и точные границы перед входом в потомков.
    pub exact_bounds: Option<(Bounds<Pixels>, Bounds<Pixels>)>,
    /// Непрозрачные метаданные хозяина, прикреплённые к боксу.
    pub metadata: Option<&'a dyn Any>,
}

type ChildrenCallback = dyn Fn(
    &ChildPaintContext<'_>,
    &mut dyn Iterator<Item = &mut AnyElement>,
    &mut Window,
    &mut App,
);
type BoundaryCallback = dyn Fn(&mut Window) -> Box<dyn Any>;
type FillBoundsCallback = dyn Fn(&Style, Bounds<Pixels>, &Window) -> Bounds<Pixels>;

type FinishRecording = dyn Fn(&mut dyn Any, &mut Window, &mut App);

/// Host-owned work that must finish inside a source command recording scope.
/// GPUI only pairs entry, completion and guard destruction; it owns no queue.
#[derive(Clone)]
pub struct PaintRecording {
    /// Open the host collector after entering the fresh recording scope.
    pub begin: Rc<BoundaryCallback>,
    /// Finish deferred host work before that scope and its resources close.
    pub finish: Rc<FinishRecording>,
}

/// Обход детей в обычной точке paint, не более раза, без повторного prepaint.
#[derive(Clone)]
pub struct ChildPainter(pub Rc<ChildrenCallback>);

/// Правила хозяина, установленные через `Window::with_render_context`.
pub struct PaintTraversal {
    /// Вход в непрозрачный элемент; GPUI не интерпретирует возвращаемый guard.
    pub enter_boundary: Rc<BoundaryCallback>,
    /// Обычный обход детей для боксов без локальной замены.
    pub children: ChildPainter,
    /// Выбор явной геометрии заливки перед выдачей существующего quad.
    pub fill_bounds: Rc<FillBoundsCallback>,
    /// Optional lifecycle for source recording; playback never invokes it.
    pub recording: Option<PaintRecording>,
}

/// Наблюдение в обычной точке prepaint детей, внутри clip их бокса.
/// Хозяин сохраняет непрозрачные scopes; GPUI не повторяет обход детей.
pub struct PrepaintTraversal {
    /// Вызывается перед детьми после применения overflow и scroll offset.
    pub enter_children: Rc<dyn Fn(&Window)>,
}
