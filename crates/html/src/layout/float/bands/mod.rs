//! Полосы занятости плавающими блоками (CSS 2.1 §9.5.1, §9.5.2, §10.6.7).
//!
//! Модель — плоский список полос по образцу servo
//! (`components/layout/flow/float.rs:536`, `FloatBand`), но без AA-дерева:
//! персистентность нужна там снимкам раскладки, а флоатов на странице
//! единицы, и вставка в `Vec` дешевле обхода дерева.
//!
//! Все координаты логические и отсчитываются от угла форматирующего
//! контекста: X — от инлайн-начала, ОБЕ границы полосы; Y — от верха.
//! Перевод в `FloatShape` (экстент от СВОЕЙ стороны) делает `shapes()`.
//!
//! Сторона — `i8`, как в стиле: -1 слева, 1 справа (`style::computed` `float`);
//! `clear` — `Option<i8>`, где `Some(0)` значит `both`.

use crate::layout::float::shapes::FloatShape;
pub(super) use crate::tolerance::COORD_EPS;

mod place;
mod query;

/// Полоса занятости — горизонтальный срез контекста, внутри которого набор
/// флоатов постоянен. Полоса `i` покрывает `[bands[i].top, bands[i+1].top)`;
/// последняя в списке — сторож с `top = +∞`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Верх полосы.
    pub top: f32,
    /// Правый край левых флоатов полосы. `None` — левых флоатов на полосе
    /// нет ВОВСЕ.
    ///
    /// Отличать `None` от `Some(0.0)` обязательно: `clear: left` обязан
    /// сработать и об флоат нулевой ширины. Экстент `0.0` — «флоат есть, он
    /// пустой», `None` — «флоата нет».
    pub left: Option<f32>,
    /// Левый край правых флоатов полосы — тоже от инлайн-начала, а не от
    /// правого края.
    pub right: Option<f32>,
}

/// One placed float or initial letter as a line exclusion: its side, block
/// range and inline edges, in context coordinates. Line layout reads the
/// exclusions per float rather than off the bands: a float of zero block
/// size occupies no band, yet it still shortens the line box it falls in
/// (CSS 2.1 §9.5, `floats-zero-height-wrap-*`); and a float with
/// `shape-outside` shortens lines by its shape, not by its margin box
/// (css-shapes-1 §1), like Blink's `ExclusionArea::shape_data`.
#[derive(Clone, Debug, PartialEq)]
struct Exclusion {
    side: i8,
    top: f32,
    h: f32,
    /// The margin edge facing the content (right edge of a left float).
    edge: f32,
    /// The margin edge on the float's own side.
    near: f32,
    /// `shape-outside`, measured from `near` and from `top`.
    shape: Option<FloatShape>,
}

/// Полосы занятости одного блочного форматирующего контекста.
///
/// Живёт ровно столько, сколько раскладывается контекст, и знает только
/// числа: ничего не мерит и ни в какое дерево не смотрит.
#[derive(Clone, Debug)]
pub struct FloatBands {
    /// Полосы по возрастанию `top`. Первая — сторож `-∞`, последняя `+∞`.
    bands: Vec<Band>,
    /// Инлайн-стенки содержащего блока в координатах контекста.
    ///
    /// Пара, а не одна ширина, потому что содержащий блок бывает у́же
    /// контекста: флоат внутри вложенного блока обязан спотыкаться о флоат
    /// снаружи (правило 3).
    cb: (f32, f32),
    /// Потолок от уже поставленных флоатов: правило 5 — новый флоат не выше
    /// верха любого более раннего.
    ceil_floats: f32,
    /// Потолок от поточного содержимого: низ предыдущего блока (правило 5) и
    /// верх текущей строчной коробки (правило 6). Ставится снаружи.
    ceil_flow: f32,
    /// Нижний край margin-box самого нижнего ЛЕВОГО флоата: позиция для
    /// `clear: left` (§9.5.2) и вклад в высоту корня контекста (§10.6.7).
    /// Отсутствие — −∞: ноль был бы ложным препятствием отрицательному margin.
    clear_l: f32,
    /// То же для правых.
    clear_r: f32,
    /// Block-end of the lowest initial letter on each side (left, right).
    /// Not a float edge for float placement, but `clear` and a new formatting
    /// context's auto height include it (css-inline-3 §clearing initial
    /// letters; Blink `ClearanceOffsetIncludingInitialLetter`).
    letter: (f32, f32),
    /// Every placed float and initial letter, in placement order.
    exclusions: Vec<Exclusion>,
}

impl FloatBands {
    /// Пустой контекст шириной содержащего блока `cb_w`.
    pub fn new(cb_w: f32) -> Self {
        FloatBands {
            bands: vec![
                Band {
                    top: f32::NEG_INFINITY,
                    left: None,
                    right: None,
                },
                Band {
                    top: f32::INFINITY,
                    left: None,
                    right: None,
                },
            ],
            cb: (0.0, cb_w.max(0.0)),
            ceil_floats: 0.0,
            ceil_flow: 0.0,
            clear_l: f32::NEG_INFINITY,
            clear_r: f32::NEG_INFINITY,
            letter: (f32::NEG_INFINITY, f32::NEG_INFINITY),
            exclusions: Vec::new(),
        }
    }

    /// Поставить инлайн-стенки содержащего блока; вернуть прежние.
    ///
    /// Содержащий блок бывает у́же контекста: флоат внутри вложенного блока
    /// с полем обязан спотыкаться о флоат снаружи (правила 3 и 7). Servo —
    /// `replace_containing_block_position_info` (`flow/float.rs:971-977`).
    pub fn set_walls(&mut self, l: f32, r: f32) -> (f32, f32) {
        std::mem::replace(&mut self.cb, (l, r.max(l)))
    }

    /// Опустить потолок поточного содержимого: низ предыдущего блока
    /// (правило 5) или верх набираемой строки (правило 6).
    ///
    /// Именно `set`, а не `max`: содержимое, вылезшее за контейнер, потолок
    /// поднимает.
    pub fn set_flow_ceiling(&mut self, y: f32) {
        self.ceil_flow = y;
    }

    /// Потолок размещения нового флоата — правила 4, 5, 6.
    fn ceiling(&self) -> f32 {
        self.ceil_floats.max(self.ceil_flow)
    }

    /// Номер полосы, накрывающей `y`.
    ///
    /// Сторож `-∞` не больше конечного `y`, поэтому результат всегда ≥ 0;
    /// сторож `+∞` не ≤ конечного `y`, поэтому результат ≤ `len - 2` —
    /// обращение к `bands[i + 1]` законно в любом методе.
    fn idx_at(&self, y: f32) -> usize {
        self.bands.partition_point(|b| b.top <= y) - 1
    }

    /// Свободные инлайн-стенки полосы: экстенты флоатов, а где их нет —
    /// стенки содержащего блока.
    fn walls(&self, i: usize) -> (f32, f32) {
        (
            self.bands[i].left.unwrap_or(self.cb.0),
            self.bands[i].right.unwrap_or(self.cb.1),
        )
    }

    /// Разрезать список так, чтобы граница полосы прошла ровно по `y`;
    /// вернуть номер полосы, начинающейся с `y`.
    ///
    /// Разрез — вставка копии накрывающей полосы с новым верхом: экстенты
    /// наследуются, занятость не меняется.
    fn split_at(&mut self, y: f32) -> usize {
        let i = self.idx_at(y);
        if self.bands[i].top == y {
            return i;
        }
        let mut cut = self.bands[i];
        cut.top = y;
        self.bands.insert(i + 1, cut);
        i + 1
    }

    /// Влезает ли флоат шириной `w` стороной `side` в полосу `i`.
    ///
    /// Правило 7 — не вылезти за дальний край, но ТОЛЬКО когда на полосе уже
    /// есть флоат своей стороны (одинокий слишком широкий вылезать обязан);
    /// правило 3 — не столкнуться с флоатом противоположной стороны.
    fn fits(&self, i: usize, side: i8, w: f32) -> bool {
        let b = self.bands[i];
        if side < 0 {
            let x = b.left.unwrap_or(self.cb.0).max(self.cb.0);
            if b.left.is_some() && x + w > self.cb.1 + COORD_EPS {
                return false;
            }
            match b.right {
                None => true,
                Some(r) => w <= r - x + COORD_EPS,
            }
        } else {
            let x = b.right.unwrap_or(self.cb.1).min(self.cb.1);
            if b.right.is_some() && x - w < self.cb.0 - COORD_EPS {
                return false;
            }
            match b.left {
                None => true,
                Some(l) => w <= x - l + COORD_EPS,
            }
        }
    }
}

#[cfg(test)]
mod tests;
