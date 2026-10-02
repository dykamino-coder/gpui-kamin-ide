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
//! Сторона — `i8`, как в стиле: -1 слева, 1 справа (`computed.rs` `float`);
//! `clear` — `Option<i8>`, где `Some(0)` значит `both`.

use crate::flow::FloatShape;
use std::sync::Arc;

/// Допуск сравнения точек: та же величина, что у ряда обтекания в
/// `render.rs` — иначе флоат ровно по ширине контейнера уезжает вниз на
/// ошибке округления.
const EPS: f32 = 0.01;

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
    clear_l: f32,
    /// То же для правых.
    clear_r: f32,
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
            clear_l: 0.0,
            clear_r: 0.0,
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

    /// Флоатов ещё не было: вызывающий может не строить хост-обёртку.
    pub fn is_empty(&self) -> bool {
        self.bands.len() == 2
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
            if b.left.is_some() && x + w > self.cb.1 + EPS {
                return false;
            }
            match b.right {
                None => true,
                Some(r) => w <= r - x + EPS,
            }
        } else {
            let x = b.right.unwrap_or(self.cb.1).min(self.cb.1);
            if b.right.is_some() && x - w < self.cb.0 - EPS {
                return false;
            }
            match b.left {
                None => true,
                Some(l) => w <= x - l + EPS,
            }
        }
    }

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
            Some(-1) => self.clear_l,
            Some(1) => self.clear_r,
            _ => self.clear_l.max(self.clear_r),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Правило 1: левый флоат встаёт вплотную к левому краю.
    #[test]
    fn left_hugs_edge() {
        let mut b = FloatBands::new(200.0);
        assert_eq!(b.add_float(-1, 50.0, 20.0, None), (0.0, 0.0));
        assert_eq!(b.available(0.0, 10.0), (50.0, 200.0));
        // Ниже флоата строка снова во всю ширину.
        assert_eq!(b.available(20.0, 10.0), (0.0, 200.0));
    }

    /// Правило 1 для правой стороны.
    #[test]
    fn right_hugs_edge() {
        let mut b = FloatBands::new(200.0);
        assert_eq!(b.add_float(1, 50.0, 20.0, None), (150.0, 0.0));
        assert_eq!(b.available(0.0, 10.0), (0.0, 150.0));
    }

    /// Правило 3: два левых флоата встают в ряд, третий не влезает и уходит
    /// вниз — та самая лесенка (правило 2), своего кода не имеющая.
    #[test]
    fn stair_step() {
        let mut b = FloatBands::new(100.0);
        assert_eq!(b.add_float(-1, 60.0, 10.0, None), (0.0, 0.0));
        // 60 + 60 > 100 — второй под первый.
        assert_eq!(b.add_float(-1, 60.0, 10.0, None), (0.0, 10.0));
    }

    /// Правило 3: левый и правый на одной полосе, пока хватает ширины.
    #[test]
    fn left_and_right_share_band() {
        let mut b = FloatBands::new(100.0);
        assert_eq!(b.add_float(-1, 40.0, 10.0, None), (0.0, 0.0));
        assert_eq!(b.add_float(1, 40.0, 10.0, None), (60.0, 0.0));
        assert_eq!(b.available(0.0, 5.0), (40.0, 60.0));
        // Третий не влезает между ними.
        assert_eq!(b.add_float(-1, 40.0, 10.0, None), (0.0, 10.0));
    }

    /// Правило 7: одинокий слишком широкий флоат вылезает, а не уезжает вниз.
    #[test]
    fn too_wide_alone_overflows() {
        let mut b = FloatBands::new(50.0);
        assert_eq!(b.add_float(-1, 80.0, 10.0, None), (0.0, 0.0));
    }

    /// Правило 5: новый флоат не выше верха более раннего.
    #[test]
    fn ceiling_never_rises() {
        let mut b = FloatBands::new(100.0);
        b.add_float(-1, 100.0, 10.0, None);
        // Второй влезть рядом не может — уходит под первый и остаётся там,
        // даже если третий узкий.
        assert_eq!(b.add_float(-1, 10.0, 10.0, None), (0.0, 10.0));
        assert_eq!(b.add_float(-1, 10.0, 10.0, None), (10.0, 10.0));
    }

    /// §9.5.2: `clear: left` проходит мимо правого флоата.
    #[test]
    fn clear_is_sided() {
        let mut b = FloatBands::new(100.0);
        b.add_float(1, 30.0, 40.0, None);
        assert_eq!(b.clearance(Some(-1), 0.0), 0.0);
        assert_eq!(b.clearance(Some(1), 0.0), 40.0);
        assert_eq!(b.clearance(Some(0), 0.0), 40.0);
    }

    /// Флоат нулевой высоты занятости не создаёт, но `clear` об него
    /// срабатывает.
    #[test]
    fn zero_height_float_still_clears() {
        let mut b = FloatBands::new(100.0);
        b.add_float(-1, 30.0, 0.0, None);
        assert_eq!(b.available(0.0, 10.0), (0.0, 100.0));
        assert_eq!(b.bottom(Some(-1)), 0.0);
    }

    /// §10.6.7: низ флоатов — вклад в высоту корня контекста.
    #[test]
    fn bottom_tracks_sides() {
        let mut b = FloatBands::new(100.0);
        b.add_float(-1, 10.0, 25.0, None);
        b.add_float(1, 10.0, 40.0, None);
        assert_eq!(b.bottom(Some(-1)), 25.0);
        assert_eq!(b.bottom(Some(1)), 40.0);
        assert_eq!(b.bottom(None), 40.0);
    }

    /// Формы для набора строк: экстент считается от своей стороны.
    #[test]
    fn shapes_are_side_relative() {
        let mut b = FloatBands::new(100.0);
        b.add_float(-1, 30.0, 20.0, None);
        b.add_float(1, 40.0, 20.0, None);
        let (l, r) = &*b.shapes(0.0);
        assert_eq!(
            l.as_slice(),
            &[FloatShape::Band {
                top: 0.0,
                h: 20.0,
                w: 30.0
            }]
        );
        assert_eq!(
            r.as_slice(),
            &[FloatShape::Band {
                top: 0.0,
                h: 20.0,
                w: 40.0
            }]
        );
    }

    /// Лесенка из четырёх флоатов — геометрия `CSS2/floats-clear/floats-005`
    /// (дюйм = 96 точек, содержащий блок 1.25in).
    ///
    /// Последняя строка ловит наследование экстента при разрезе: полоса
    /// [96, 120) несёт 91.2, а [120, 192) — 72.
    #[test]
    fn ladder() {
        let mut b = FloatBands::new(120.0);
        assert_eq!(b.add_float(-1, 115.2, 96.0, None), (0.0, 0.0));
        assert_eq!(b.add_float(-1, 72.0, 96.0, None), (0.0, 96.0));
        assert_eq!(b.add_float(-1, 19.2, 24.0, None), (72.0, 96.0));
        assert_eq!(b.add_float(-1, 48.0, 24.0, None), (72.0, 120.0));
    }

    /// Правило 3: узкий правый не влезает рядом с широким левым и уходит под
    /// него (`floats/floats-rule3-outside-left-002`).
    #[test]
    fn rule3_opposite() {
        let mut b = FloatBands::new(500.0);
        assert_eq!(b.add_float(-1, 475.0, 50.0, None), (0.0, 0.0));
        assert_eq!(b.add_float(1, 50.0, 50.0, None), (450.0, 50.0));
    }

    /// Разрез посередине чужого флоата: новая полоса наследует его экстент.
    #[test]
    fn split_inherits() {
        let mut b = FloatBands::new(200.0);
        b.add_float(-1, 40.0, 100.0, None);
        // Правый встаёт на своей стороне и режет левую полосу пополам.
        assert_eq!(b.add_float(1, 30.0, 50.0, None), (170.0, 0.0));
        assert_eq!(b.available(0.0, 10.0), (40.0, 170.0));
        // Ниже правого левый ещё держится.
        assert_eq!(b.available(60.0, 10.0), (40.0, 200.0));
    }

    /// `available` через границу двух полос берёт худшее из обеих.
    #[test]
    fn available_window() {
        let mut b = FloatBands::new(200.0);
        b.add_float(-1, 30.0, 20.0, None);
        b.add_float(1, 50.0, 100.0, None);
        // Окно [10, 40) задевает и полосу с левым флоатом, и следующую.
        assert_eq!(b.available(10.0, 30.0), (30.0, 150.0));
    }

    /// Смещение потребителя вжигается в верх формы.
    #[test]
    fn shapes_shift_by_consumer_top() {
        let mut b = FloatBands::new(100.0);
        b.add_float(-1, 30.0, 50.0, None);
        let (l, _) = &*b.shapes(20.0);
        assert_eq!(
            l.as_slice(),
            &[FloatShape::Band {
                top: -20.0,
                h: 50.0,
                w: 30.0
            }]
        );
    }

    /// §9.5, последний абзац: коробка со своим контекстом ищет ОКНО на всю
    /// свою высоту. Геометрия `CSS2/floats/floats-wrap-top-below-001l`.
    #[test]
    fn place_among_001l() {
        let mut b = FloatBands::new(400.0);
        assert_eq!(b.add_float(-1, 50.0, 75.0, Some(-1)), (0.0, 0.0));
        // `clear: left` сажает второй флоат под первый, а не рядом.
        assert_eq!(b.add_float(-1, 100.0, 75.0, Some(-1)), (0.0, 75.0));
        assert_eq!(b.place_among(200.0, 50.0, 0.0), (50.0, 0.0, 350.0));
        assert_eq!(b.place_among(200.0, 50.0, 50.0), (100.0, 50.0, 300.0));
    }

    /// `floats-wrap-top-below-002l`: правый 300 рядом не влезает (правило 3)
    /// и садится на 75; вторая коробка съезжает под оба флоата.
    #[test]
    fn place_among_002l() {
        let mut b = FloatBands::new(400.0);
        assert_eq!(b.add_float(-1, 150.0, 75.0, None), (0.0, 0.0));
        assert_eq!(b.add_float(1, 300.0, 75.0, None), (100.0, 75.0));
        assert_eq!(b.place_among(200.0, 50.0, 0.0), (150.0, 0.0, 250.0));
        assert_eq!(b.place_among(200.0, 50.0, 50.0), (0.0, 150.0, 400.0));
    }

    /// `floats-wrap-top-below-003l`: окно съезжает НА ОДНУ полосу, а не ниже
    /// всех флоатов.
    #[test]
    fn place_among_003l() {
        let mut b = FloatBands::new(400.0);
        assert_eq!(b.add_float(-1, 250.0, 75.0, None), (0.0, 0.0));
        assert_eq!(b.add_float(1, 250.0, 75.0, None), (150.0, 75.0));
        assert_eq!(b.place_among(100.0, 50.0, 0.0), (250.0, 0.0, 150.0));
        assert_eq!(b.place_among(100.0, 50.0, 50.0), (0.0, 75.0, 150.0));
    }
}
