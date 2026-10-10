//! Запросы к полосам: доступная ширина, края, зазор, низ, формы, место среди флоатов.

use super::{EPS, FloatBands};
use crate::layout::float::shapes::FloatShape;
use std::sync::Arc;

impl FloatBands {
    /// Свободный инлайн-отрезок на полосе `[y, y + h)`: левая и правая
    /// стенки в координатах контекста.
    ///
    /// Это §9.5: «The current and subsequent line boxes created next to the
    /// float are shortened as necessary». Максимум левых и минимум правых по
    /// всем задетым полосам.
    pub fn available(&self, y: f32, h: f32) -> (f32, f32) {
        let (mut l, mut r) = self.cb;
        let bot = y + h.max(0.0);
        let mut i = self.idx_at(y);
        loop {
            if let Some(v) = self.bands[i].left {
                l = l.max(v);
            }
            if let Some(v) = self.bands[i].right {
                r = r.min(v);
            }
            let next = self.bands[i + 1].top;
            // Нулевая высота (строка-щуп) обрывает обход на первой полосе.
            if next >= bot || !next.is_finite() {
                break;
            }
            i += 1;
        }
        (l, r.max(l))
    }

    /// Ближайшая граница полос строго ниже `y` — следующий кандидат верха
    /// при переборе возможностей (Blink `ExclusionSpace::AllLayoutOpportunities`,
    /// `exclusion_space.h:633-646`; Servo `PlacementAmongFloats::place`,
    /// `flow/float.rs:227-252`). `None` — ниже `y` полос больше нет, место
    /// свободно до конца контекста.
    pub fn next_edge(&self, y: f32) -> Option<f32> {
        let i = self.idx_at(y);
        let t = self.bands[i + 1].top;
        t.is_finite().then_some(t)
    }

    /// Позиция, ниже которой обязан начаться блок с данным `clear` (§9.5.2);
    /// `y` — гипотетическая позиция без clearance.
    ///
    /// Отдаётся именно позиция, а не величина clearance: сама величина —
    /// максимум из двух (нижний край флоатов против восстановления обычного
    /// потока) и требует схлопнутых полей, которых полосы не знают.
    pub fn clearance(&self, clear: Option<i8>, y: f32) -> f32 {
        match clear {
            None => y,
            Some(-1) => y.max(self.clear_l),
            Some(1) => y.max(self.clear_r),
            _ => y.max(self.clear_l).max(self.clear_r),
        }
    }

    /// Нижний край флоатов стороны; `None` — обеих.
    ///
    /// Это же значение §10.6.7 требует включить в высоту коробки,
    /// устанавливающей НОВЫЙ контекст: обычный блок в потоке флоатами не
    /// растёт — они из него вываливаются.
    pub fn bottom(&self, side: Option<i8>) -> f32 {
        match side {
            Some(-1) => self.clear_l.max(self.letter.0),
            Some(1) => self.clear_r.max(self.letter.1),
            _ => self
                .clear_l
                .max(self.clear_r)
                .max(self.letter.0)
                .max(self.letter.1),
        }
    }

    /// Вырезы для потребителя, чей верх стоит на `from_y`: пара списков форм
    /// (левые, правые) для набора строк.
    ///
    /// Здесь и только здесь координаты переводятся в систему `FloatShape`:
    /// экстент от СВОЕЙ стороны, верх — от верха потребителя. Смещение
    /// `from_y` вжигается в `top`, поэтому потребителю знать о нём нечего.
    pub fn shapes(&self, from_y: f32) -> Arc<(Vec<FloatShape>, Vec<FloatShape>)> {
        let mut left = vec![];
        let mut right = vec![];
        for w in self.bands.windows(2) {
            let (b, next) = (w[0], w[1]);
            // Сторож `-∞` занятости не несёт, а полоса, упирающаяся в сторож
            // `+∞`, — это остаток контекста ниже последнего флоата: там уже
            // свободно.
            if !b.top.is_finite() || !next.top.is_finite() {
                continue;
            }
            // Полоса, кончившаяся ВЫШЕ потребителя, его строк не режет:
            // `cut` на ней и так дал бы ноль, но и держать её незачем.
            if next.top <= from_y {
                continue;
            }
            let (top, h) = (b.top - from_y, next.top - b.top);
            if h <= 0.0 {
                continue;
            }
            if let Some(edge) = b.left {
                left.push(FloatShape::Band {
                    top,
                    h,
                    w: (edge - self.cb.0).max(0.0),
                });
            }
            if let Some(edge) = b.right {
                right.push(FloatShape::Band {
                    top,
                    h,
                    w: (self.cb.1 - edge).max(0.0),
                });
            }
        }
        Arc::new((left, right))
    }

    /// Место для коробки `w` × `h`, которая флоаты НЕ перекрывает: замещаемый
    /// элемент, таблица, блок со СВОИМ контекстом форматирования (§9.5,
    /// последний абзац). Возвращает (левая стенка, верх, доступная ширина).
    ///
    /// Одной полосы мало — нужно ОКНО на всю высоту коробки, и окно скользит
    /// вниз, пока не найдётся место (servo `PlacementAmongFloats::place`).
    /// Правило 5 здесь не работает: коробка не флоат, ниже потолка сужения
    /// появляются, и проверять надо каждую задетую полосу, а не только
    /// верхнюю. Не нашлось нигде — ведём себя как `clear: both`.
    pub fn place_among(&self, w: f32, h: f32, ceiling: f32) -> (f32, f32, f32) {
        let mut top = ceiling;
        loop {
            let first = self.idx_at(top);
            let (mut l, mut r) = self.cb;
            let mut i = first;
            let mut bot;
            loop {
                if let Some(v) = self.bands[i].left {
                    l = l.max(v);
                }
                if let Some(v) = self.bands[i].right {
                    r = r.min(v);
                }
                bot = self.bands[i + 1].top;
                if bot - top >= h || !bot.is_finite() {
                    break;
                }
                i += 1;
            }
            if r - l >= w - EPS {
                return (l, top, r - l);
            }
            if !bot.is_finite() {
                break;
            }
            // Верхняя полоса окна выброшена — искать со следующей. Верх строго
            // растёт, поэтому цикл конечен.
            top = self.bands[first + 1].top;
        }
        (
            self.cb.0,
            ceiling.max(self.clear_l).max(self.clear_r),
            self.cb.1 - self.cb.0,
        )
    }
}
