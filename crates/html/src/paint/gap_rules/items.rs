//! Буфер геометрии элементов контейнера, собираемый пробами до покраски линеек промежутков:
//! border-box элементов с их хвостовыми полями и коробки колонок/рядов/spanner'ов
//! многоколонника. Каждый отрисованный экземпляр контейнера, включая копии одного DOM-узла в
//! разных фрагментах (CSS Gaps 1 §fragmentation), получает свой буфер под уникальным ключом.
use gpui::{Bounds, Pixels};

mod probe;
pub use probe::gap_item_probe;

/// Probed border boxes of one rendered container instance. Each instance
/// (including copies of one DOM node in separate fragmentainers, CSS Gaps 1
/// §fragmentation) creates its own buffer, so geometry never mixes.
#[derive(Default)]
pub struct GapBuffer {
    pub items: Vec<Bounds<Pixels>>,
    pub margins: Vec<[f32; 2]>,
    pub multicol: Option<MulticolGaps>,
}
impl std::ops::Deref for GapBuffer {
    type Target = Vec<Bounds<Pixels>>;
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
impl std::ops::DerefMut for GapBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}
#[derive(Default)]
pub struct MulticolGaps {
    pub columns: Vec<Bounds<Pixels>>,
    pub rows: Vec<(Bounds<Pixels>, usize)>,
    pub spanners: Vec<Bounds<Pixels>>,
}
