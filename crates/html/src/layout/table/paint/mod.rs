//! Покраска фонов ячеек и рамок таблицы.
// owner: A

use crate::text::clamp::forget_clamp_buffers;
use gpui::{Bounds, Pixels};
mod clipped;
mod clipped_paint;
pub use clipped::CellsClipped;
mod probe;
pub use probe::cell_rect_probe;
mod edge_painter;
mod edge_segments;
pub use edge_painter::EdgePainter;
mod cell_bg;
pub use cell_bg::CellBgPainter;
pub use cell_bg::CellBgs;
pub use cell_bg::GRID_BOX;
pub use cell_bg::cell_bg_probe;
pub use cell_bg::edge_probe;
pub use cell_bg::grid_probe;

/// Отрисовка ребёнка только в ПРЯМОУГОЛЬНИКАХ, снятых пробами прошлого кадра.
///
/// Фон ряда таблицы (css-tables-3 §drawing-backgrounds): картинка ряда
/// рисуется В ЯЧЕЙКАХ, непрерывно от начала ряда, а зазоры остаются чистыми.
/// Геометрию ячеек знает только раскладка — её снимают пробы в ячейках, а
/// ряд рисует своего ребёнка по разу на прямоугольник, обрезая маской.
/// Первый кадр пуст (пробы ещё не писали) — стенд и так ждёт устоявшийся.
/// Прямоугольник ячейки + флаг «точная»: точная лежит целиком в своём
/// ряду/колонке (span = 1), объединённая (rowspan/colspan) выходит за них.
/// Область фона считается ТОЛЬКО по точным — объединённая растягивала бы
/// градиент колонки на чужие дорожки; маски краски — по всем. Третье поле —
/// тот же прямоугольник без округления к точке устройства: от него
/// считается область позиционирования (CSS 2.1 §17.5.1, Blink — по
/// неокруглённой геометрии ячеек), маски остаются округлёнными.
pub type RowRects = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, bool, Bounds<Pixels>)>>>;

thread_local! {
    /// Буферы прямоугольников ПО РЯДАМ, переживающие перестройку дерева:
    /// каждый кадр стенд строит элементы заново, и Rc из прошлого кадра
    /// иначе терялся вместе с записями проб.
    static ROW_RECTS: std::cell::RefCell<std::collections::HashMap<u64, RowRects>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Буфер прямоугольников ряда по устойчивому номеру узла.
pub fn row_rects_for(node_id: u64) -> RowRects {
    ROW_RECTS.with(|m| m.borrow_mut().entry(node_id).or_default().clone())
}

/// Сброс буферов проб при смене документа.
///
/// Номера узлов считаются с нуля в каждом документе: без сброса полоса
/// нового документа забирала прямоугольники ячеек ПРЕЖНЕГО с тем же
/// номером, и первый кадр красил фон по чужим местам — а если ничего не
/// инвалидировало окно, грязный кадр оставался последним.
pub fn forget_row_rects() {
    ROW_RECTS.with(|m| m.borrow_mut().clear());
    CELL_EDGES.with(|m| m.borrow_mut().clear());
    BAND_RETRIES.with(|m| m.borrow_mut().clear());
    forget_clamp_buffers();
}

thread_local! {
    /// Сколько кадров полоса фона прождала своих проб. Ключ — адрес буфера
    /// проб.
    ///
    /// Полоса без единой ячейки (`<col>` без рядов, `<col>` за краем сетки)
    /// не дождётся их никогда, а запрос кадра без счётчика вертел бы окно
    /// вечно: документ не успокаивается, и стенд снимает его на таймауте.
    static BAND_RETRIES: std::cell::RefCell<std::collections::HashMap<usize, u8>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько кадров ждать пробы, прежде чем счесть полосу пустой.
const BAND_WAIT_FRAMES: u8 = 2;

/// Сросшиеся кромки таблицы: ячейка без собственных рамок отдаёт их
/// отдельному слою — кромки рисуются НА ЛИНИЯХ сетки поверх фонов
/// (css-tables-3 §drawing-borders), а конфликт «шире побеждает»
/// (CSS 2.1 §17.6.2.1) решается порядком: узкие раньше, широкие поверх.
pub struct EdgeCell {
    pub bounds: Bounds<Pixels>,
    /// Ширины кромок [верх, право, низ, лево] в точках.
    pub widths: [f32; 4],
    pub colors: [crate::style::values::value::Color; 4],
    /// Ранги стилей сторон (см. `Computed::border_side_styles`); 9 = solid.
    pub styles: [u8; 4],
    /// Ранг источника (CSS 2.1 §17.6.2.1 п.4), больше — сильнее: таблица 0,
    /// группа колонок 1, колонка 2, группа рядов 3, ряд 4, ячейка 5.
    pub source: u8,
    /// Порядок в документе: раньше = выше/левее, при равенстве побеждает.
    pub doc_ix: u32,
}

pub type CellEdges = std::rc::Rc<std::cell::RefCell<Vec<EdgeCell>>>;

thread_local! {
    static CELL_EDGES: std::cell::RefCell<std::collections::HashMap<u64, CellEdges>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn cell_edges_for(key: u64) -> CellEdges {
    CELL_EDGES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}
