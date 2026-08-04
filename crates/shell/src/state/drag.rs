//! Состояния перетаскивания: сплиттеры раскладки, плитки тулов, табы, чипы.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

/// Драг сплиттера: что тащим и от каких размеров считаем.
pub struct DragState {
    pub(crate) kind: crate::root::DragKind,
    /// Координата мыши в начале драга (x для вертикальных, y для горизонтальных)
    pub(crate) start: f32,
    /// Начальные размеры: (a, b) — пары зависят от kind
    pub(crate) init: (f32, f32),
}

/// Drag плитки тула между слотами.
pub struct ToolDrag {
    pub src: crate::activity::PanelSlot,
    pub id: String,
    pub start: (f32, f32),
    pub pos: (f32, f32),
    pub started: bool,
    pub over: Option<crate::activity::PanelSlot>,
    /// Индекс вставки внутри over-слота (ховер таба стрипа); None = в конец.
    pub over_index: Option<usize>,
}

/// Drag-reorder файл-таба (тот же порог 4px, цель — индекс таба под курсором).
pub struct TabDrag {
    pub src: usize,
    pub start: (f32, f32),
    pub started: bool,
    pub over: Option<usize>,
}

/// Drag-reorder чипа сессии (id-адресация — список приходит от хоста).
pub struct ChipDrag {
    pub src: String,
    pub start: (f32, f32),
    pub started: bool,
    pub over: Option<String>,
}

/// Перенос элемента: src занимает индекс dst (таб «встаёт на место»
/// наведённого, остальные сдвигаются). Возвращает новый индекс.
pub fn move_item<T>(v: &mut Vec<T>, src: usize, dst: usize) -> usize {
    let item = v.remove(src);
    let dst = dst.min(v.len());
    v.insert(dst, item);
    dst
}
