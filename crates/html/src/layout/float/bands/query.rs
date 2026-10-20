//! Запросы к полосам: доступная ширина, края, зазор, низ, формы.

use super::FloatBands;
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
        // Opposing exclusions on different bands may leave no common interval.
        // Preserve crossed edges so even a zero-width BFC must move down
        // (CSS 2.1 section 9.5; Servo PlacementAmongFloats).
        (l, r)
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
        for e in &self.exclusions {
            // An exclusion that ended above the consumer cuts none of its
            // lines; a zero-height one still cuts the line it falls in.
            let bottom = e.top + e.h;
            if bottom < from_y || (e.h > 0.0 && bottom <= from_y) {
                continue;
            }
            let top = e.top - from_y;
            let shape = match &e.shape {
                Some(s) => {
                    let off = if e.side < 0 {
                        e.near - self.cb.0
                    } else {
                        self.cb.1 - e.near
                    };
                    let mut s = s.offset(off);
                    s.shift_top(e.top - from_y);
                    s
                }
                None => FloatShape::Band {
                    top,
                    h: e.h,
                    w: if e.side < 0 {
                        (e.edge - self.cb.0).max(0.0)
                    } else {
                        (self.cb.1 - e.edge).max(0.0)
                    },
                },
            };
            if e.side < 0 {
                left.push(shape);
            } else {
                right.push(shape);
            }
        }
        Arc::new((left, right))
    }
}
