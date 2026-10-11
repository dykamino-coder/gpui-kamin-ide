//! Line state for clamp; split out to keep the owning module within 250 lines.

use super::{PARA_BUDGET, PARA_ROWS};
use crate::text::clamp::PARA_TAG;
use gpui::{Bounds, Pixels};

/// Бюджет строк обрезки (`line-clamp`, css-overflow-3/4): точка среза —
/// низ N-й СЧИТАЕМОЙ строки. Строки потомков в собственном контексте
/// форматирования (BFC: overflow, флоат, корень потока) видимы, но НЕ
/// считаются; блок, пересекающий точку среза, прячется целиком — срез
/// поднимается к его верху. Точка меряется пробами построенного кадра и
/// применяется потолком высоты на СЛЕДУЮЩЕМ (перестройка каждый кадр).
#[derive(Clone)]
pub struct ClampEntry {
    pub bounds: Bounds<Pixels>,
    /// Высота строки в точках; 0 — блок без собственного текста.
    pub line: f32,
    /// Строки не считаются (элемент внутри вложенного BFC).
    pub skip_count: bool,
    /// Коробка с ЗАДАННОЙ высотой: фрагментировать нечего, пересечённая
    /// точкой среза она прячется целиком.
    pub fixed_height: bool,
    /// Нижние рамка и паддинг коробки в точках. Проба меряет ПАДДИНГ-БОКС,
    /// а фрагментированная коробка своих нижних рамки и паддинга не теряет
    /// (css-overflow-4 §5.3): на них укорачивается бюджет строк и на них же
    /// удлиняется итоговый срез. Для строчных проб — ноль.
    pub bp_after: f32,
    /// Порядковый номер абзаца ВНУТРИ клэмп-контейнера, выданный при
    /// построении. `None` — проба коробки, а не абзаца: знак обрыва на
    /// неё не садится. Номер, а не совпадение по геометрии, потому что
    /// сопоставлять надо КАДРЫ: срез считается по прошлому кадру, а
    /// применяется на следующем.
    pub seq: Option<u32>,
    /// Сколько строк этому абзацу оставлено УЖЕ на этом кадре. Признак
    /// того, что абзац укорочен нами: его собственный низ за срез больше
    /// не выходит, и без этой защёлки «что-то срезано» пропало бы, а
    /// кадр запросил бы себя заново (css-overflow-4 §5.3: вставка знака
    /// «must not cause a reevaluation of the effects of `continue`»).
    pub clamped: Option<usize>,
    /// Пустая поточная блочная коробка (без содержимого): сама по себе —
    /// возможная точка среза МЕЖДУ блоками (css-overflow-4 §5.3).
    pub empty: bool,
}

pub type ClampLines = std::rc::Rc<std::cell::RefCell<Vec<ClampEntry>>>;

thread_local! {
    pub(super) static CLAMP_LINES: std::cell::RefCell<std::collections::HashMap<u64, ClampLines>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Вычисленные точки среза (высота от верха контейнера) прошлого кадра.
    pub(super) static CLAMP_CUTS: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Стек активных clamp-контейнеров при ПОСТРОЕНИИ дерева:
    /// (ключ, граница BFC уже пройдена).
    pub(super) static CLAMP_STACK: std::cell::RefCell<Vec<(u64, bool)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Знак обрыва АВТО-режима, посчитанный на прошлом кадре:
    /// ключ контейнера → (номер абзаца, сколько его строк остаётся).
    /// Абзац в контейнере ровно один — тот, на чьей последней строке
    /// перед точкой среза стоит знак (css-overflow-4 §5.3).
    pub(super) static CLAMP_PARA: std::cell::RefCell<std::collections::HashMap<u64, (u32, usize)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Счётчик абзацев контейнера при ПОСТРОЕНИИ поддерева.
    pub(super) static CLAMP_SEQ: std::cell::RefCell<std::collections::HashMap<u64, u32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn clamp_lines_for(key: u64) -> ClampLines {
    CLAMP_LINES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

/// Выдать следующему абзацу клэмп-контейнера его номер. Счётчик сбрасывает
/// вход в сам контейнер (`ClampGuard::enter`), поэтому нумерация одна и та
/// же на каждом кадре, пока не меняется дерево.
pub fn clamp_next_seq(key: u64) -> u32 {
    CLAMP_SEQ.with(|m| {
        let mut m = m.borrow_mut();
        let n = m.entry(key).or_insert(0);
        let v = *n;
        *n += 1;
        v
    })
}

pub fn clamp_cut(key: u64) -> Option<f32> {
    CLAMP_CUTS.with(|m| m.borrow().get(&key).copied())
}

/// Бюджет строк абзаца со знаком обрыва: (номер абзаца, сколько строк
/// оставить). Считает `ClampCut::paint` прошлого кадра.
pub fn clamp_para(key: u64) -> Option<(u32, usize)> {
    CLAMP_PARA.with(|m| m.borrow().get(&key).copied())
}

pub fn forget_clamp_buffers() {
    CLAMP_LINES.with(|m| m.borrow_mut().clear());
    CLAMP_CUTS.with(|m| m.borrow_mut().clear());
    CLAMP_STACK.with(|st| st.borrow_mut().clear());
    CLAMP_PARA.with(|m| m.borrow_mut().clear());
    CLAMP_SEQ.with(|m| m.borrow_mut().clear());
    PARA_BUDGET.with(|c| c.set(None));
    PARA_ROWS.with(|m| m.borrow_mut().clear());
    PARA_TAG.with(|c| c.set(None));
}

/// Сторож стека clamp-контекста на время построения поддерева.
pub struct ClampGuard(pub(crate) bool);

impl ClampGuard {
    /// Вход в сам clamp-контейнер.
    pub fn enter(key: u64) -> Self {
        CLAMP_STACK.with(|st| st.borrow_mut().push((key, false)));
        // Нумерация абзацев контейнера начинается заново на каждом кадре:
        // иначе номер рос бы от кадра к кадру и бюджет прошлого кадра
        // никогда не находил бы своего абзаца.
        CLAMP_SEQ.with(|m| {
            m.borrow_mut().insert(key, 0);
        });
        ClampGuard(true)
    }

    /// Вход в элемент с собственным контекстом форматирования: строки
    /// глубже не считаются.
    pub fn enter_bfc() -> Self {
        let pushed = CLAMP_STACK.with(|st| {
            let mut st = st.borrow_mut();
            match st.last().copied() {
                Some((key, false)) => {
                    st.push((key, true));
                    true
                }
                _ => false,
            }
        });
        ClampGuard(pushed)
    }
}

impl Drop for ClampGuard {
    fn drop(&mut self) {
        if self.0 {
            CLAMP_STACK.with(|st| {
                st.borrow_mut().pop();
            });
        }
    }
}

/// Текущий clamp-контекст построения: (ключ, внутри вложенного BFC).
pub fn clamp_context() -> Option<(u64, bool)> {
    CLAMP_STACK.with(|st| st.borrow().last().copied())
}
