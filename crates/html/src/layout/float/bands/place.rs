//! Добавление флоата и буквицы в полосы (CSS 2.1 §9.5.1).

use super::{COORD_EPS, Exclusion, FloatBands};
use crate::layout::float::shapes::FloatShape;

impl FloatBands {
    /// Поставить флоат: `side` -1 слева / 1 справа, `w`/`h` — размеры
    /// margin-box в точках, `clear` — из стиля. Вернуть левый верхний угол
    /// margin-box в координатах контекста.
    ///
    /// Правила 1, 4, 8, 9 — прямо здесь; 3 и 7 — в `fits`; 5 и 6 — через
    /// потолок; правило 2 (лесенка) своего кода не имеет: его исполняет сама
    /// модель полос.
    pub fn add_float(&mut self, side: i8, w: f32, h: f32, clear: Option<i8>) -> (f32, f32) {
        // Отрицательные размеры в ЗАНЯТОСТЬ не идут (спека объявляет такую
        // позицию неопределённой), но в проверку места и в позицию — идут:
        // Blink ищет возможность с `minimum_inline_size = inline_size +
        // margins.InlineSum()` без отсечки (`floats_utils.cc:40-61`), а
        // прямоугольник исключения отсекает в ноль (`CreateExclusionArea`,
        // `:150-161`). Флоат `margin-left: -150px` шириной 50 за флоатом в
        // 150 встаёт на x=150 своего margin-box — его рамка на нуле
        // (`negative-margin-float-positioning`).
        let w_fit = w;
        // Правила 4, 5, 6 и §9.5.2.
        let ceiling = self.clearance(clear, self.ceiling());
        // Правило 8: как можно выше — спуск начинается с потолка. Правила 3
        // и 7: спускаться, пока не влезет.
        //
        // Проверяется только полоса верха, а не всё окно на высоту `h`, и это
        // законно: по правилу 5 потолок не выше верха любого более раннего
        // флоата, значит НИЖЕ потолка новых сужений не появляется — полосы
        // вниз только расширяются.
        let mut i = self.idx_at(ceiling);
        while !self.fits(i, side, w_fit) && self.bands[i + 1].top.is_finite() {
            i += 1;
        }
        let y = self.bands[i].top.max(ceiling);
        let (l, r) = self.walls(i);
        // Правила 1 и 9: вплотную к своей стороне, но не за край.
        let x = if side < 0 {
            l.max(self.cb.0)
        } else {
            r.min(self.cb.1) - w_fit
        };
        self.insert_float(side, x, y, w_fit, h);
        // Правило 5: следующий флоат не выше этого.
        self.ceil_floats = self.ceil_floats.max(y);
        (x, y)
    }

    /// Продолжение уже начатого float: opportunity читается на границе
    /// фрагмента, пройденная часть margin-box остаётся выше этой границы.
    /// Clear относится к первому фрагменту, поэтому здесь не повторяется.
    pub(crate) fn resume_float(
        &mut self,
        side: i8,
        w: f32,
        h: f32,
        top: f32,
        fragment_start: f32,
    ) -> (f32, f32) {
        if top + h.max(0.0) <= fragment_start {
            let x = if side < 0 { self.cb.0 } else { self.cb.1 - w };
            self.insert_float(side, x, top, w, h);
            return (x, top);
        }
        // Правило 5 CSS2 §9.5.1 действует и между продолжениями: поздний
        // узкий float не обгоняет ранний, отложенный занятой полосой.
        let start = fragment_start.max(self.ceil_floats);
        let mut i = self.idx_at(start);
        while !self.fits(i, side, w) && self.bands[i + 1].top.is_finite() {
            i += 1;
        }
        let delay = self.bands[i].top.max(start) - fragment_start;
        let (l, r) = self.walls(i);
        let x = if side < 0 {
            l.max(self.cb.0)
        } else {
            r.min(self.cb.1) - w
        };
        let y = top + delay;
        self.insert_float(side, x, y, w, h);
        self.ceil_floats = self.ceil_floats.max(fragment_start + delay);
        (x, y)
    }

    fn insert_float(&mut self, side: i8, x: f32, y: f32, w: f32, h: f32) {
        let near = if side < 0 { x } else { x + w };
        let (w, h) = (w.max(0.0), h.max(0.0));
        // Занятость: экстент — дальний от своей стороны край флоата.
        let edge = if side < 0 { x + w } else { x };
        let bot = y + h;
        self.exclusions.push(Exclusion {
            side,
            top: y,
            h,
            edge,
            near,
            shape: None,
        });
        let a = self.split_at(y);
        let b = self.split_at(bot);
        for band in &mut self.bands[a..b] {
            if side < 0 {
                band.left = Some(match band.left {
                    Some(old) => old.max(edge),
                    None => edge,
                });
            } else {
                band.right = Some(match band.right {
                    Some(old) => old.min(edge),
                    None => edge,
                });
            }
        }
        // §9.5.2 и §10.6.7 — нижний край стороны.
        if side < 0 {
            self.clear_l = self.clear_l.max(bot);
        } else {
            self.clear_r = self.clear_r.max(bot);
        }
    }

    /// Give the float placed last its `shape-outside` line exclusion
    /// (css-shapes-1 §2), measured from the float's own side of its margin
    /// box and from its top. Later floats and boxes that establish a
    /// formatting context keep avoiding the margin box (§1, CSS 2.1 §9.5).
    pub fn shape_last(&mut self, shape: FloatShape) {
        if let Some(e) = self.exclusions.last_mut() {
            e.shape = Some(shape);
        }
    }

    /// Поставить исключение буквицы (css-inline-3 §initial-letter-floats,
    /// шаг F11): margin-box `w`×`h` у начала строки, верх которой — `top`
    /// (поток), сторона `side`. Буквица — не флоат: правило 5 (не выше
    /// ранних флоатов) к ней не относится, она встаёт в первое окно от
    /// строки вниз, где влезает на всю свою высоту (Blink
    /// `PostPlaceInitialLetterBox`, `inline_layout_algorithm.cc`: исключение
    /// от позиции строки, «after floats» — за флоатами той же строки).
    /// Потолков флоатов и посадки флоатов с `clear` она не меняет; её низ
    /// входит только в `bottom` (`clear` блоков и высота нового контекста).
    pub fn add_initial_letter(&mut self, side: i8, w: f32, h: f32, top: f32) -> (f32, f32) {
        let (w, h) = (w.max(0.0), h.max(0.0));
        let mut y = top;
        loop {
            let (l, r) = self.available(y, h);
            if r - l + COORD_EPS >= w {
                break;
            }
            match self.next_edge(y) {
                Some(t) => y = t,
                None => break,
            }
        }
        let (l, r) = self.available(y, h);
        let x = if side < 0 { l } else { r - w };
        if side < 0 {
            self.letter.0 = self.letter.0.max(y + h);
        } else {
            self.letter.1 = self.letter.1.max(y + h);
        }
        let edge = if side < 0 { x + w } else { x };
        self.exclusions.push(Exclusion {
            side,
            top: y,
            h,
            edge,
            near: if side < 0 { x } else { x + w },
            shape: None,
        });
        let a = self.split_at(y);
        let b = self.split_at(y + h);
        for band in &mut self.bands[a..b] {
            if side < 0 {
                band.left = Some(band.left.map_or(edge, |old| old.max(edge)));
            } else {
                band.right = Some(band.right.map_or(edge, |old| old.min(edge)));
            }
        }
        (x, y)
    }
}
