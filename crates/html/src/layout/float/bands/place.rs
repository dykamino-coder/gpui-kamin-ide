//! Добавление флоата и буквицы в полосы (CSS 2.1 §9.5.1).

use super::{EPS, FloatBands};

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
        let (w, h) = (w.max(0.0), h.max(0.0));
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
        // Занятость: экстент — дальний от своей стороны край флоата.
        let edge = if side < 0 { x + w } else { x };
        let bot = y + h;
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
        // Правило 5: следующий флоат не выше этого.
        self.ceil_floats = self.ceil_floats.max(y);
        (x, y)
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
            if r - l + EPS >= w {
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
