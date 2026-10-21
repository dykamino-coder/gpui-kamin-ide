//! Запасные измерения для повёрнутых атомарных поддеревьев, принадлежащие документу.
//!
//! Нативные вертикальные абзацы и определённые ортогональные строчные ограничения уже
//! измеряются за один проход. Адаптер повёрнутого атома всё ещё заявляет свой блочный размер
//! раньше, чем его flex-родитель разрешит итоговую строчную длину. Prepaint раскладывает
//! плоского ребёнка на этой длине и записывает получившийся блочный размер для следующего
//! кадра. Эти запасные данные держат и пути claim-cap и fit-limit.
//!
//! Скопированный GPUI LayoutMeasurement может измерять, не заимствуя движок родителя. Чтобы
//! использовать его здесь, нужно заменить фиксированное заявление ширины измеряемым листом и
//! разрешать auto-высоту относительно содержащего блока или ICB (CSS Writing Modes 4, раздел 7.3),
//! а не по текущему растяжению родителя. Переносить собранный AnyElement во вложенный движок
//! нельзя: потомки хранят LayoutId исходного дерева. Модуль лишь изолирует существующий запасной
//! путь; контракт размеров от этого однопроходным не становится.
//! Хранилище и нумерация вхождений принадлежат одному документу, включая iframe.

use gpui::Pixels;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct VerticalMeasures {
    sizes: RefCell<HashMap<u64, Pixels>>,
    occurrences: RefCell<HashMap<u64, u64>>,
}

impl VerticalMeasures {
    pub(crate) fn begin_frame(&self) {
        self.occurrences.borrow_mut().clear();
    }

    pub(crate) fn next_key(&self, base: u64) -> u64 {
        let mut counts = self.occurrences.borrow_mut();
        let n = counts.entry(base).or_default();
        *n += 1;
        base ^ n.wrapping_mul(0x517C_C1B7_2722_0A95)
    }

    pub(crate) fn get(&self, key: u64) -> Option<Pixels> {
        self.sizes.borrow().get(&key).copied()
    }

    pub(crate) fn record(&self, key: u64, size: Pixels) {
        let mut sizes = self.sizes.borrow_mut();
        if sizes.len() >= 256 {
            sizes.clear();
        }
        sizes.insert(key, size);
    }
}

#[cfg(test)]
mod tests {
    use crate::document::state::DocState;
    use gpui::px;

    #[test]
    fn measures_follow_document_scope_and_occurrences_survive_frames() {
        let outer = DocState::new(Default::default());
        let inner = DocState::new(Default::default());
        let a = outer.vertical_measures.next_key(7);
        let b = outer.vertical_measures.next_key(7);
        assert_ne!(a, b);
        outer.vertical_measures.record(a, px(20.));
        outer.vertical_measures.record(b, px(40.));
        {
            assert_eq!(inner.vertical_measures.next_key(7), a);
            assert_eq!(inner.vertical_measures.get(a), None);
            inner.vertical_measures.record(a, px(80.));
        }
        outer.vertical_measures.begin_frame();
        assert_eq!(outer.vertical_measures.next_key(7), a);
        assert_eq!(outer.vertical_measures.get(a), Some(px(20.)));
        assert_eq!(outer.vertical_measures.next_key(7), b);
        assert_eq!(outer.vertical_measures.get(b), Some(px(40.)));
        assert_eq!(inner.vertical_measures.get(a), Some(px(80.)));
    }
}
