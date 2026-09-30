//! Строчный поток атомов с вырезами (`shape-outside`).
//!
//! Обтекание плавающего блока ФОРМОЙ не выразить рядом-колонкой (наша
//! механика флоатов) и не выразить flex-переносом: у формы ширина выреза
//! своя НА КАЖДОЙ СТРОКЕ. Этот элемент раскладывает готовые коробки
//! построчно сам: курсор, перенос, вырезы по полосам — и рисует детей со
//! смещениями. Дети приходят с ИЗВЕСТНЫМИ размерами (инлайн-блоки с
//! заданными сторонами — ровно то, чем WPT рисует картину обтекания).

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Size, Window, point, px, size,
};

/// Полоса выреза: на строках, пересекающих [y0, y1), начало (или конец)
/// строки занято на `left`/`right` точек.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExBand {
    pub y0: f32,
    pub y1: f32,
    pub left: f32,
    pub right: f32,
}

/// Ребёнок потока: элемент и его известный размер.
pub struct FlowChild {
    pub el: AnyElement,
    pub w: f32,
    pub h: f32,
}

pub struct FlowRow {
    children: Vec<FlowChild>,
    shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
    /// Направление письма: rtl кладёт коробки от правого края.
    rtl: bool,
    /// `writing-mode: vertical-rl`: строки — колонки справа налево, поток в
    /// колонке — сверху вниз. Раскладка идёт в ТРАНСПОНИРОВАННОМ мире
    /// (инлайн-ось строкой), физика восстанавливается при укладке.
    vertical_rl: bool,
    /// Позиции детей, вычисленные замером (в точках от угла коробки).
    slots: std::cell::RefCell<Vec<(f32, f32)>>,
}

impl FlowRow {
    pub fn new(
        children: Vec<FlowChild>,
        shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
        rtl: bool,
    ) -> Self {
        FlowRow {
            children,
            shapes,
            rtl,
            vertical_rl: false,
            slots: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn vertical_rl(mut self) -> Self {
        self.vertical_rl = true;
        self
    }

    /// Размер ребёнка в осях раскладки: в вертикальном письме инлайн-ось —
    /// физическая высота.
    fn tdims(&self, c: &FlowChild) -> (f32, f32) {
        if self.vertical_rl {
            (c.h, c.w)
        } else {
            (c.w, c.h)
        }
    }

    /// Вырез на полосе [y, y+h): точный экстент форм с обеих сторон.
    fn cut(&self, y: f32, h: f32) -> (f32, f32) {
        let l = self
            .shapes
            .0
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        let r = self
            .shapes
            .1
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        (l, r)
    }

    /// Нижний край всех форм: ниже него вырезов нет.
    fn shapes_bottom(&self) -> f32 {
        self.shapes
            .0
            .iter()
            .chain(self.shapes.1.iter())
            .map(|f| match *f {
                FloatShape::Band { top, h, .. } => top + h,
                FloatShape::Circle { top, cy, r, .. } => top + cy + r,
                FloatShape::Ellipse { top, cy, ry, .. } => top + cy + ry,
                FloatShape::Poly { top, ref pts } => {
                    top + pts.iter().map(|p| p.1).fold(0.0f32, f32::max)
                }
                FloatShape::Profile { top, ref ext } => top + ext.len() as f32,
            })
            .fold(0.0f32, f32::max)
    }

    /// Разложить детей в ширину `limit`; вернуть высоту и позиции.
    fn layout(&self, limit: f32) -> (f32, Vec<(f32, f32)>) {
        let mut slots = Vec::with_capacity(self.children.len());
        let mut y = 0.0f32;
        let mut x = 0.0f32;
        let mut line_h = 0.0f32;
        let mut cut = self.cut(0.0, 1.0);
        for c in &self.children {
            let (cw, ch) = self.tdims(c);
            let avail = (limit - cut.0 - cut.1).max(0.0);
            // ЗАМЕРЕНО И ОТКАЧЕНО: гасить перенос при `white-space: nowrap`
            // (поля `nowrap` у ряда не было вовсе). Полный свод CSS3: 0 и 0.
            // Заявленные пары в своде не нашлись под названными именами —
            // полосный ряд включается только при обтекании, а тесты гибкой
            // раскладки флоатов не содержат.
            // Не влезает — новая строка; коробка шире строки стоит одна.
            if x + cw > avail + 0.01 && x > 0.0 {
                y += line_h;
                x = 0.0;
                line_h = 0.0;
                cut = self.cut(y, ch.max(1.0));
            } else if x == 0.0 {
                cut = self.cut(y, ch.max(1.0));
            }
            // Коробка не помещается даже в начале строки — строка съезжает
            // ниже, пока вырез не отпустит (float-retry-push): плавающий
            // блок толкает СЛИШКОМ ШИРОКОЕ содержимое под себя.
            if x == 0.0 && cw > (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                let bottom = self.shapes_bottom();
                while y < bottom {
                    y += 1.0;
                    cut = self.cut(y, ch.max(1.0));
                    if cw <= (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                        break;
                    }
                }
            }
            // Вырез мог смениться выше по строке — пересчитать после переноса.
            let (sx, sy) = if self.rtl {
                (limit - cut.1 - x - cw, y)
            } else {
                (cut.0 + x, y)
            };
            slots.push((sx, sy));
            x += cw;
            line_h = line_h.max(ch);
        }
        (y + line_h, slots)
    }
}

impl Element for FlowRow {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let sizes: Vec<(f32, f32)> = self.children.iter().map(|c| (c.w, c.h)).collect();
        let shapes = self.shapes.clone();
        let rtl = self.rtl;
        let vertical_rl = self.vertical_rl;
        let id = window.request_measured_layout(
            gpui::Style::default(),
            move |known, available, _window, _cx| {
                // Предел инлайн-оси: в вертикальном письме это ВЫСОТА.
                let pick = |k: Option<gpui::Pixels>, a: gpui::AvailableSpace| {
                    k.map(f32::from).or(match a {
                        gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                        _ => None,
                    })
                };
                let limit = if vertical_rl {
                    pick(known.height, available.height)
                } else {
                    pick(known.width, available.width)
                }
                .unwrap_or(0.0);
                // Тот же обход, что и в layout(): без детей-элементов.
                let probe = FlowRow {
                    children: sizes
                        .iter()
                        .map(|&(w, h)| FlowChild {
                            el: gpui::Empty.into_any_element(),
                            w,
                            h,
                        })
                        .collect(),
                    shapes: shapes.clone(),
                    rtl,
                    vertical_rl,
                    slots: std::cell::RefCell::new(Vec::new()),
                };
                let (h, _) = probe.layout(limit);
                if vertical_rl {
                    // Блок-прогресс — ширина (колонки), инлайн — высота.
                    size(px(h), px(limit))
                } else {
                    size(px(limit), px(h))
                }
            },
        );
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Раскладка и подготовка детей — здесь: замер поддеревьев в фазе
        // отрисовки запрещён самим окном.
        let limit = if self.vertical_rl {
            f32::from(bounds.size.height)
        } else {
            f32::from(bounds.size.width)
        };
        let bw = f32::from(bounds.size.width);
        let vertical_rl = self.vertical_rl;
        let (_, slots) = self.layout(limit);
        let slots: Vec<(f32, f32)> = self
            .children
            .iter()
            .zip(slots)
            .map(|(c, (sx, sy))| {
                if vertical_rl {
                    // t-мир → физика: колонка sy идёт от ПРАВОГО края.
                    (bw - sy - c.w, sx)
                } else {
                    (sx, sy)
                }
            })
            .collect();
        for (c, (sx, sy)) in self.children.iter_mut().zip(slots.iter()) {
            let origin = point(bounds.origin.x + px(*sx), bounds.origin.y + px(*sy));
            c.el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(c.w)),
                    gpui::AvailableSpace::Definite(px(c.h)),
                ),
                window,
                cx,
            );
            c.el.prepaint_at(origin, window, cx);
        }
        *self.slots.borrow_mut() = slots;
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        for c in self.children.iter_mut() {
            c.el.paint(window, cx);
        }
    }
}

impl IntoElement for FlowRow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Форма обтекания плавающего блока — в координатах от НАЧАЛА стороны
/// обтекания (для правого флоата вызывающий зеркалит X заранее).
///
/// `cut(y0, y1)` — насколько занята строка-полоса [y0, y1): максимальный
/// горизонтальный экстент формы на этой полосе, в точках от края.
#[derive(Clone, Debug, PartialEq)]
pub enum FloatShape {
    /// Прямоугольник: занято `w` на высоте [top, top+h).
    Band { top: f32, h: f32, w: f32 },
    /// Круг с центром (cx, cy) от верха полосы флоата.
    Circle { top: f32, cx: f32, cy: f32, r: f32 },
    Ellipse {
        top: f32,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    /// Многоугольник: вершины в точках от начала стороны; экстент полосы —
    /// максимум X рёбер в её диапазоне.
    Poly {
        top: f32,
        pts: std::sync::Arc<Vec<(f32, f32)>>,
    },
    /// Профиль из картинки (`shape-outside: url(...)`): экстент на каждую
    /// точку высоты, от начала стороны.
    Profile {
        top: f32,
        ext: std::sync::Arc<Vec<f32>>,
    },
}

impl FloatShape {
    /// Сдвинуть форму вниз: флоат перенесён на следующую полосу.
    pub fn shift_top(&mut self, dy: f32) {
        match self {
            FloatShape::Band { top, .. }
            | FloatShape::Circle { top, .. }
            | FloatShape::Ellipse { top, .. }
            | FloatShape::Poly { top, .. }
            | FloatShape::Profile { top, .. } => *top += dy,
        }
    }

    pub fn cut(&self, y0: f32, y1: f32) -> f32 {
        match *self {
            FloatShape::Band { top, h, w } => {
                if top + h > y0 && top < y1 {
                    w
                } else {
                    0.0
                }
            }
            FloatShape::Circle { top, cx, cy, r } => ellipse_cut(top + cy, r, r, cx, y0, y1),
            FloatShape::Ellipse {
                top,
                cx,
                cy,
                rx,
                ry,
            } => ellipse_cut(top + cy, rx, ry, cx, y0, y1),
            FloatShape::Profile { top, ref ext } => {
                let a = (y0 - top).max(0.0) as usize;
                let b = ((y1 - top).ceil()).max(0.0) as usize;
                return ext
                    .get(a..b.min(ext.len()))
                    .map(|s| s.iter().fold(0.0f32, |m, &v| m.max(v)))
                    .unwrap_or(0.0);
            }
            FloatShape::Poly { top, ref pts } => {
                // Верхняя кромка полосы ОТКРЫТА (как строки растра у
                // blink RasterShape): вершина/срез ровно на y0 принадлежит
                // предыдущей полосе — вогнутая «лесенка» иначе залипала
                // на ступени (shape-outside-polygon-007..011).
                let (y0, y1) = (y0 - top + 1e-3, y1 - top);
                let mut m = 0.0f32;
                let n = pts.len();
                for i in 0..n {
                    let (x1, py1) = pts[i];
                    let (x2, py2) = pts[(i + 1) % n];
                    // Вершина в полосе — как есть.
                    if py1 > y0 && py1 < y1 {
                        m = m.max(x1);
                    }
                    // Ребро пересекает границы полосы — X в точках среза.
                    let (lo, hi) = if py1 <= py2 { (py1, py2) } else { (py2, py1) };
                    if hi <= y0 || lo >= y1 || (hi - lo) < 1e-6 {
                        continue;
                    }
                    for yb in [y0.max(lo), y1.min(hi)] {
                        let t = (yb - py1) / (py2 - py1);
                        if (0.0..=1.0).contains(&t) {
                            m = m.max(x1 + (x2 - x1) * t);
                        }
                    }
                }
                m
            }
        }
    }

    /// Грубый ключ для кэша замера абзаца.
    pub fn hash_bits(&self) -> u64 {
        let q = |v: f32| (v * 8.0) as i64 as u64;
        match *self {
            FloatShape::Band { top, h, w } => {
                1 ^ q(top).rotate_left(8) ^ q(h).rotate_left(24) ^ q(w).rotate_left(40)
            }
            FloatShape::Circle { top, cx, cy, r } => {
                2 ^ q(top).rotate_left(6)
                    ^ q(cx).rotate_left(18)
                    ^ q(cy).rotate_left(30)
                    ^ q(r).rotate_left(44)
            }
            FloatShape::Ellipse {
                top,
                cx,
                cy,
                rx,
                ry,
            } => {
                3 ^ q(top).rotate_left(5)
                    ^ q(cx).rotate_left(15)
                    ^ q(cy).rotate_left(27)
                    ^ q(rx).rotate_left(39)
                    ^ q(ry).rotate_left(51)
            }
            FloatShape::Profile { top, ref ext } => ext
                .iter()
                .fold(5u64 ^ q(top), |acc, &v| acc.rotate_left(9) ^ q(v)),
            FloatShape::Poly { top, ref pts } => pts.iter().fold(4u64 ^ q(top), |acc, &(x, y)| {
                acc.rotate_left(7) ^ q(x) ^ q(y).rotate_left(3)
            }),
        }
    }
}

/// Экстент эллипса (центр по y — `cy_abs`, радиусы rx/ry, центр по x — cx)
/// на полосе [y0, y1): максимум `cx + rx·√(1−(dy/ry)²)` по dy в полосе.
pub(crate) fn ellipse_cut(cy_abs: f32, rx: f32, ry: f32, cx: f32, y0: f32, y1: f32) -> f32 {
    if ry <= 0.0 || rx <= 0.0 {
        return 0.0;
    }
    if y1 <= cy_abs - ry || y0 >= cy_abs + ry {
        return 0.0;
    }
    // Ближайшая к центру точка полосы даёт самый широкий срез.
    let dy = if (y0..y1).contains(&cy_abs) {
        0.0
    } else if y1 <= cy_abs {
        cy_abs - y1
    } else {
        y0 - cy_abs
    };
    let k = 1.0 - (dy / ry) * (dy / ry);
    if k <= 0.0 {
        return 0.0;
    }
    cx + rx * k.sqrt()
}

/// Ребёнок колонок: элемент, высота коробки и вертикальные поля
/// (схлопываются между соседями по правилам потока).
pub struct StackChild {
    pub el: AnyElement,
    /// Запасные копии ТОГО ЖЕ ребёнка. Разрез между колонками рисует по
    /// копии на фрагмент: элемент GPUI рисуется ровно один раз, и показать
    /// одну коробку в двух колонках иначе нечем. Копий столько же, сколько
    /// колонок, — больше ребёнок занять не может.
    pub frags: Vec<AnyElement>,
    /// Монолит — коробка, которую нельзя разрывать (css-break-3 §4.1):
    /// `break-inside: avoid`, прокручиваемая коробка, ячейка таблицы,
    /// замещаемый элемент и сплошной строчный набор. Такая уходит в
    /// следующую колонку целиком.
    pub monolith: bool,
    /// Точки ЗАКОННОГО разреза (css-break-3 §4.3, класс A) — границы
    /// вложенных блочных детей, рекурсивно: `(need, from)` — сколько
    /// ребёнка от верха должно уместиться до разреза и с какого смещения
    /// продолжать в следующей колонке (поле на границе усекается,
    /// css-break-3 §5.2). По возрастанию.
    pub cuts: Vec<(f32, f32)>,
    /// Принудительные разрывы (css-break-4 §3.1): перед коробкой, после неё
    /// и внутри — смещения от верха, где `break-before/after` вложенных
    /// блочных детей требуют новой колонки.
    pub force_before: bool,
    pub force_after: bool,
    /// Запрет разрыва на ГРАНИЦЕ с соседом (css-break-4 §4.3, правило 1):
    /// `break-before: avoid*` этой коробки и `break-after: avoid*`
    /// предыдущей запрещают разрез ровно в этой точке — но не внутри самих
    /// коробок (это `monolith`). С переносом по §break-propagation:
    /// значение снимается `render::edge_avoid`, как принудительное —
    /// `edge_break`.
    pub avoid_before: bool,
    pub avoid_after: bool,
    pub forced: Vec<f32>,
    /// Диапазоны, внутри которых разрыв запрещён (css-break-4 §4.1 —
    /// монолиты-потомки; рамка и отбивка самой коробки): `[a, b)` от верха.
    pub solid: Vec<(f32, f32)>,
    pub h: f32,
    pub mt: f32,
    pub mb: f32,
    /// `column-span: all` (css-multicol-1 §6): кладётся во всю ширину между
    /// линиями колонок; в стопке с рядами — по курсору Blink
    /// `LayoutSpanner` (не влез в остаток ряда — со следующего ряда).
    pub span: bool,
    /// Низ ПАРАЛЛЕЛЬНОГО потока (css-break-3 §3) от верха коробки:
    /// содержимое, переполняющее заданную высоту, продолжается в следующем
    /// фрагментаинере само по себе. Равен `h`, когда потока нет. Поток
    /// режется по нему, а шагом для СОСЕДА остаётся `h`.
    pub over: f32,
    /// Общий сдвиг `position: relative`, снятый с копии (`hoist_relative`).
    /// css-break-3 §5.5: «Fragmentation occurs before relative positioning …
    /// Such effects are applied per fragment» — сдвиг накладывается НА
    /// фрагмент, а значит двигает и его срез. Внутри копии он бы уехал из
    /// маски колонки и погас (`out-of-flow-in-multicolumn-042/045`), а у
    /// КОРНЯ копии `layout_as_root` его и вовсе не читает (проба `pm1`).
    pub rel: (f32, f32),
    /// `box-decoration-break: clone`: блочное украшение `(верх, низ)`.
    /// Взведённое поле значит, что КАЖДАЯ копия — уже готовый фрагмент своей
    /// высоты (`render.rs::clone_fragment`): её не поднимают на срез и не
    /// режут маской.
    pub clone_dec: Option<(f32, f32)>,
    /// Монолит-ПОТОМОК, начатый на верху колонки, переполняет её, а не режется
    /// краем (`fill_at`, `overflow_to`). Только `column-fill: auto` без рядов и
    /// только ребёнку без элементов ряда (`render::parallel_items_inside`):
    /// баланс подобрал бы высоту ниже монолита, а у ряда flex/сетки/таблицы в
    /// переполнение ушли бы соседи по ряду.
    pub overflow_top: bool,
}

/// Мера ребёнка для укладки колонок.
#[derive(Clone)]
pub struct Kid {
    pub h: f32,
    pub mt: f32,
    pub mb: f32,
    pub monolith: bool,
    pub cuts: Vec<(f32, f32)>,
    pub force_before: bool,
    pub force_after: bool,
    /// css-break-4 §4.3 правило 1 — см. `StackChild::avoid_before`. Стопка
    /// СТРАНИЦ ставит сюда `false`: правило 1 в печати пока не применяется.
    pub avoid_before: bool,
    pub avoid_after: bool,
    pub forced: Vec<f32>,
    pub solid: Vec<(f32, f32)>,
    pub span: bool,
    /// Низ параллельного потока от верха коробки (`StackChild::over`); `h`,
    /// когда потока нет.
    pub over: f32,
    /// `box-decoration-break: clone` — блочное украшение `(верх, низ)`,
    /// повторяемое в КАЖДОМ фрагменте (css-break-4 §break-decoration).
    /// `None` — `slice`, прежний путь до последней строки.
    pub clone_dec: Option<(f32, f32)>,
    /// См. `StackChild::overflow_top`; у страниц — `false`.
    pub overflow_top: bool,
}

/// Кусок ребёнка в колонке: чей он, какая по счёту копия, в какой колонке
/// стоит, на сколько отступает от её верха, какая часть содержимого видна
/// (`from` — от собственного верха ребёнка) и какой она высоты.
#[derive(Clone, Copy)]
struct Frag {
    kid: usize,
    copy: usize,
    col: usize,
    y: f32,
    from: f32,
    h: f32,
}

/// Колонки многоколоночного потока для БЛОЧНЫХ детей с известными
/// высотами (css-multicol §7.4 + css-break): жадная укладка сверху вниз,
/// балансировка «оценка + добавка на минимальный недолаз» (как в blink
/// ResolveColumnAutoBlockSize), монолиты уходят в следующую колонку
/// целиком. Разрез ДЕТЕЙ (строк/рамок) — следующая фаза.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): разрез ребёнка по краю колонки написан и
/// работает. Устройство: `StackChild` несёт по копии элемента на колонку
/// (элемент GPUI рисуется ровно один раз, и показать одну коробку в двух
/// колонках больше нечем), `fill` возвращает фрагменты
/// `{кто, копия, колонка, отступ, срез, высота}`, раскладка ставит каждую
/// копию ЦЕЛИКОМ и поднимает её на срез, а отрисовка режет маской
/// `with_content_mask` — это и есть вид `slice` из css-break-3 §4.
/// Замерено ЧЕТЫРЕ раза на срезе css-break+css-multicol (1498 пар):
///
/// * голый разрез, маска на каждом ребёнке — 333 -> 342 (+29/−20);
/// * плюс монолиты (`break-inside: avoid`, прокрутка, замещаемый, таблица,
///   ячейка, сплошной строчный набор) — 341 -> 338 (+18/−21);
/// * плюс маска ТОЛЬКО разрезанному ребёнку — 341 -> 342 (+2/−1);
/// * то же без отсечки «сплошной строчный набор» — 341 -> 341 (+5/−5).
///
/// Отсюда главный вывод, который и надо помнить: почти весь «выигрыш»
/// первого захода был ЛОЖНЫЙ — его давала маска, прятавшая переполнение у
/// НЕразрезанных детей (`overflow-clip-004`, `overflow-unsplittable-*`,
/// `overflowing-block-003` и родня возвращаются, как только маску ограничить
/// разрезанными). Сам разрез по высотам детей стоит +2 и упирается в то,
/// что укладка колонок не видит СТРОК: смелая отсечка режет посреди строки
/// и теряет ровно столько же, сколько приобретает.
///
/// Пятый замер, уже против СВЕЖЕЙ базы (342 зелёных): 342, +20/−20 — и
/// это самое важное наблюдение. Двигаются РОВНО те же двадцать пар, что и
/// от правки «у `<canvas>` есть коробка», только в обратную сторону:
/// `overflow-clip-004`, `table-cell-expansion-006`,
/// `flex-container-fragmentation-008/009`, `monolithic-with-overflow`,
/// `tall-line-in-short-fragmentainer-002` возвращаются в зелёное, а
/// `borders-001/002`, `out-of-flow-in-multicolumn-077..080`,
/// `table-*-paint-v*` уходят в красное. Значит эта двадцатка держится не на
/// разрезе, а на том, ЕСТЬ ЛИ у замещаемого коробка и считается ли он
/// монолитом, — и любой частичный шаг просто перекладывает её из кармана в
/// карман. Три разных правки (разрез, `column-fill: auto` с высотой,
/// коробка `<canvas>`) дали на этом срезе +1, +1 и +1.
///
/// Возвращать вместе с фрагментацией ПО СТРОКАМ (высота строки и её
/// границы), а не по высотам детей, и сразу с монолитами по css-break-3
/// §4.1 — одним куском. Разбор `break-inside` написан в том же патче и тоже
/// откачен: без разреза он мёртвый. Патч целиком:
/// `target/frag-slice.patch`, разборы раздела —
/// `target/scout-cssbreak-2026-09.md` и `target/scout-linefrag-2026-09.md`
/// (второй пересчитал потолок построчной фрагментации: не 200 пар, а 40-52,
/// зато «разрыв между блочными детьми РЕКУРСИВНО» — 183 пары).
/// Ширина колонки для внутренних размеров многоколоночного контейнера:
/// `Some(w)` — `column-width` в точках, `None` — `column-width: auto`.
/// Само наличие значения означает «ширину коробки решает содержимое».
#[derive(Clone, Copy)]
pub struct Intrinsic(pub Option<f32>);

/// Ряды колонок (css-multicol-2 §column-wrap, §column-height). Blink
/// (`column_layout_algorithm.cc`): ряды — сетка с шагом `h + gap` по
/// содержимому коробки; линия колонок ставится в текущий ряд, её высота —
/// остаток ряда (`RemainingRowHeightAtOffset`), лишние колонки уходят в
/// следующий ряд (`OffsetToNextRow`). Спаннеры внутри рядов — следующий шаг
/// (`target/scout-columnwrap-2026-09.md` §5).
#[derive(Clone, Copy)]
pub struct Rows {
    /// Высота ряда: `column-height`, либо высота коробки при `column-wrap:
    /// wrap` (Blink `RowHeight`); `None` — ряд не ограничен, новые ряды
    /// родятся только от принудительных разрывов
    /// (`column-wrap-no-constraints-002`).
    pub h: Option<f32>,
    /// `row-gap` между рядами (умолчание `normal` = 1em, §rg).
    pub gap: f32,
    /// `wrap` — лишние колонки в новый ряд; иначе (`nowrap` с заданным
    /// `column-height`) — вбок, за край коробки (css-multicol-1 §8.2).
    pub wrap: bool,
    /// Рядов нет — это ПОТОЛОК баланса: `h` = заданная высота коробки
    /// многоколоночника (Blink `ConstrainColumnBlockSize`: колонка не выше
    /// used block-size контейнера). Линия одна, высотой в баланс, лишние
    /// колонки — вбок.
    pub cap: bool,
}

thread_local! {
    /// Глубина построения копий детей стопки (`render.rs`, `StackChild`).
    static STACK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Сторож «строится копия ребёнка стопки». Многоколоночник со спаннером
/// внутри другой стопки остаётся на сегментном пути: единой стопке нужен
/// перенос ряда/спаннера во ВНЕШНЮЮ колонку (Blink `LayoutSpanner`: «The new
/// row doesn't fit in the outer fragmentainer»), которого нет
/// (`column-height-029`, `target/scout-columnwrap-2026-09b.md` §2.3).
pub struct StackScope;

impl StackScope {
    pub fn enter() -> Self {
        STACK_DEPTH.with(|d| d.set(d.get() + 1));
        StackScope
    }
}

impl Drop for StackScope {
    fn drop(&mut self) {
        STACK_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

pub fn in_stack() -> bool {
    STACK_DEPTH.with(|d| d.get() > 0)
}

pub struct ColumnStack {
    children: Vec<StackChild>,
    count: usize,
    gap: f32,
    /// `column-fill: auto` + заданная высота: заполнение без баланса.
    fixed_height: Option<f32>,
    /// Линейка между колонками: ширина и цвет.
    rule: Option<(f32, gpui::Hsla)>,
    /// Ряды колонок; `None` — одна линия, как в css-multicol-1.
    rows: Option<Rows>,
    /// Сколько копий у ребёнка (= сколько колонок он может занять).
    copies: usize,
    /// Внутренние размеры контейнера, если его ширину решает СОДЕРЖИМОЕ.
    /// `None` — ширину даёт родитель, и мерить детей незачем: лишний проход
    /// раскладки стоит дороже, чем всё остальное в этом элементе.
    intrinsic: Option<Intrinsic>,
    /// Буфер границ колонок и спаннеров для `GapRulePainter` (css-gaps-1
    /// §gap-multicol): колонки линии — элементы строки, спаннер — элемент во
    /// всю ширину. Заполняется в `prepaint`, художник забирает в `paint`.
    gap_items: Option<crate::interact::GapItems>,
    plan: std::cell::RefCell<Vec<Frag>>,
    col_w: std::cell::Cell<f32>,
    /// Линии колонок после укладки: `(y, высота)` каждой — линейкам и
    /// смещениям. Колонка `col` стоит в линии `col / count`. Без спаннеров
    /// линия = ряд; спаннер режет ряд на линии (Blink `LayoutLine`).
    lines_plan: std::cell::RefCell<Vec<(f32, f32)>>,
    /// Спаннеры после укладки: `(ребёнок, y)`.
    spans_plan: std::cell::RefCell<Vec<(usize, f32)>>,
}

impl ColumnStack {
    pub fn new(
        children: Vec<StackChild>,
        count: usize,
        gap: f32,
        fixed_height: Option<f32>,
        rule: Option<(f32, gpui::Hsla)>,
        rows: Option<Rows>,
        gap_items: Option<crate::interact::GapItems>,
        intrinsic: Option<Intrinsic>,
    ) -> Self {
        // Без рядов копий ровно столько, сколько колонок (как прежде); с
        // рядами — сколько построил `render.rs`: ребёнок может занять
        // больше колонок, чем `column-count`.
        let copies = match rows {
            Some(_) => children.iter().map(|c| c.frags.len() + 1).max().unwrap_or(1),
            // Переполняющие колонки (css-multicol-1 §8.2) при `column-fill: auto`
            // с заданной высотой: копий столько, сколько построил `render.rs`
            // (лишние он строит только ребёнку с абсолютным потомком). Без
            // такого ребёнка у всех детей ровно `count` копий, и значение
            // тождественно прежнему. Балансировку это не трогает: без
            // `fixed_height` ветка ниже, как прежде.
            None if fixed_height.is_some() => children
                .iter()
                .map(|c| c.frags.len() + 1)
                .max()
                .unwrap_or(1)
                .max(count.max(1)),
            None => count.max(1),
        };
        ColumnStack {
            children,
            count: count.max(1),
            gap,
            fixed_height,
            rule,
            rows,
            copies,
            gap_items,
            intrinsic,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Где стоит колонка `col`: номер в линии и смещение линии. Без рядов и
    /// при `nowrap` колонки идут вбок сплошь (переполняющие — за край).
    fn place(&self, col: usize) -> (usize, f32) {
        match self.rows {
            Some(r) if r.wrap => {
                let lines = self.lines_plan.borrow();
                (col % self.count, lines.get(col / self.count).map_or(0.0, |l| l.0))
            }
            _ => (col, 0.0),
        }
    }

    /// Жадная укладка при данной высоте колонки: сколько колонок вышло,
    /// минимальный недолаз и план кусков. Вертикальные поля соседей
    /// СХЛОПЫВАЮТСЯ (CSS 2.1 §8.3.1) и обнуляются на границе колонки
    /// (css-break §5). Не влезший ребёнок режется по ближайшей снизу
    /// ЗАКОННОЙ точке (класс A); без таких точек — по краю колонки, если
    /// он выше колонки, иначе уходит в следующую целиком; монолит режется
    /// никогда и с верха пустой колонки переполняет её (css-break-3 §4.1).
    /// `paged` — стопка СТРАНИЦ, а не колонок: монолит, переполнивший
    /// страницу, занимает место и на следующих (Blink, crbug 1402540:
    /// содержимое после него продолжается там, где кончилось переполнение,
    /// а не с верха следующей страницы — `monolithic-overflow-001`, текст
    /// «в середине второй страницы»). В колонках правило не действует.
    pub(crate) fn fill(
        kids: &[Kid],
        target: f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        Self::fill_at(kids, &|_| target, limit, paged)
    }

    /// То же с высотой ПО КОЛОНКАМ: `target_at(col)`. Нужно рядам —
    /// полные ряды стоят в `column-height`, хвост балансируется ниже
    /// (`balance_tail`).
    fn fill_at(
        kids: &[Kid],
        target_at: &dyn Fn(usize) -> f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        let mut col = 0usize;
        let mut y = 0.0f32;
        let mut prev_mb = 0.0f32;
        let mut first = true;
        let mut placed = false;
        let mut shortage = f32::MAX;
        let mut out: Vec<Frag> = Vec::with_capacity(kids.len());
        let mut force_next = false;
        for (kid, k) in kids.iter().enumerate() {
            // Принудительный разрыв перед коробкой или после предыдущей:
            // новая колонка, если текущая не пуста (css-break-4 §3.1).
            if (k.force_before || force_next) && placed {
                col += 1;
                y = 0.0;
                placed = false;
                prev_mb = 0.0;
                first = true;
            }
            force_next = k.force_after;
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): схлопывание пары полей по
            // CSS 2.1 §8.3.1 «max положительных + min отрицательных» вместо
            // голого `max`. Срез 3029 пар (css-break/multicol/поля/флоаты
            // CSS2): +0 приобретений, потеря `multi-line-column-flex-
            // fragmentation-032` (0.00 -> 99.00 — страница разъехалась).
            // Правило верное, но в стопке колонок `prev_mb`/`k.mt` уже несут
            // РЕЗУЛЬТАТ схлопывания уровнем выше, и второе применение
            // вычитает отрицательное поле дважды.
            let lead = if first { k.mt } else { prev_mb.max(k.mt) };
            let mut cur = y + lead;
            let mut from = 0.0f32;
            let mut copy = 0usize;
            // Параллельный поток (css-break-3 §3 «parallel flows»):
            // содержимое, переполняющее коробку с ЗАДАННОЙ высотой,
            // продолжается в следующем фрагментаинере само по себе, а
            // следующий СОСЕД встаёт сразу под коробкой, в той же колонке
            // (Blink `fragmentation_utils.cc`: «If the block-size is
            // constrained / fixed … we know that we're at the end»). Значит
            // режем по `flow`, а курсор соседа откатываем на конец коробки
            // (ниже, перед `prev_mb = k.mb`). Монолит не трогаем: его
            // переполнение по css-break-3 §4.1 остаётся в своей колонке
            // целиком.
            // У `clone` поток не включается: `from`/`h` его кусков — в других
            // координатах (содержимое / готовый фрагмент), и откат курсора
            // ниже их бы не понял. Неразрезанная `clone`-коробка с потоком
            // рисуется `slice` (`render.rs`, `frag_geom.len() > 1`).
            let flow = if k.over > k.h + 0.01 && !k.monolith && k.clone_dec.is_none() {
                k.over
            } else {
                k.h
            };
            loop {
                let target = target_at(col);
                let room = target - cur;
                // `box-decoration-break: clone` (css-break-4 §break-decoration):
                // блочное украшение стоит в КАЖДОМ фрагменте, из остатка
                // колонки оно вычитается каждый раз — высота коробки =
                // содержимое + N · украшение (Blink
                // `UpdateBorderPaddingForClonedBoxDecorations`). `from` здесь —
                // по СОДЕРЖИМОМУ, `Frag.h` — готовая высота фрагмента:
                // НЕпоследний тянется до низа колонки (§box-splitting: «its
                // content box extends to fill any remaining fragmentainer
                // extent (leaving room for any margins/borders/padding applied
                // by clone)»), последний — по остатку. Монолит сюда не заходит.
                // ★ Откат 07.09 (v153) был НЕ из-за этой ветки: план верен,
                // красное ушло из 14 пар; остатки и потери дала сборка копии
                // (`target/scout-bdb-2026-09-30.md` §3).
                if let Some((dt, db)) = k.clone_dec.filter(|_| !k.monolith) {
                    let dec = dt + db;
                    let content = (k.h - dec).max(0.0);
                    let croom = room - dec;
                    let rest = content - from;
                    let edge = from + croom.max(0.0);
                    // Принудительный разрыв внутри содержимого: `k.forced` — в
                    // координатах КОРОБКИ, содержимое начинается с `dt`.
                    let forced = k
                        .forced
                        .iter()
                        .map(|&f| f - dt)
                        .find(|&f| f > from + 0.01 && f < content - 0.01 && f <= edge + 0.01);
                    if forced.is_none() && rest <= croom + 0.01 {
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest + dec });
                        y = cur + rest + dec;
                        placed = true;
                        break;
                    }
                    shortage = shortage.min(rest - croom);
                    if croom <= 0.01 && placed {
                        col += 1;
                        cur = 0.0;
                        placed = false;
                        continue;
                    }
                    // Пустая колонка, где украшению не хватило места, всё равно
                    // съедает 1px содержимого — иначе коробка не продвигается
                    // (`multicol-zero-height-003`: «it should expend 1px of its
                    // content-box per fragment»).
                    // Монолит-потомок (css-break-4 §4.1; `k.solid` — в координатах
                    // КОРОБКИ): край внутри него — разрыв ПЕРЕД ним; монолит,
                    // начатый ровно с `from`, берётся целиком и переполняет
                    // фрагмент (css-break-3 §4.1; `clone-012`).
                    let before = k
                        .solid
                        .iter()
                        .map(|&(a, b)| (a - dt, b - dt))
                        .filter(|&(a, b)| a > from + 0.01 && a < edge - 0.01 && edge < b - 0.01)
                        .map(|(a, _)| a)
                        .reduce(f32::min);
                    let whole = k
                        .solid
                        .iter()
                        .map(|&(a, b)| (a - dt, b - dt))
                        .filter(|&(a, b)| (a - from).abs() <= 0.01 && b > edge + 0.01)
                        .map(|(_, b)| b.min(content))
                        .reduce(f32::max);
                    let take = match forced {
                        Some(f) => f - from,
                        None if croom <= 0.01 => rest.min(1.0),
                        None => before.or(whole).map_or(croom, |p| p - from),
                    };
                    if take >= rest - 0.01 {
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest + dec });
                        y = cur + rest + dec;
                        placed = true;
                        break;
                    }
                    let fh = if croom <= 0.01 { take + dec } else { room };
                    out.push(Frag { kid, copy, col, y: cur, from, h: fh });
                    from += take;
                    if copy + 1 >= limit {
                        y = target;
                        placed = true;
                        break;
                    }
                    copy += 1;
                    col += 1;
                    cur = 0.0;
                    placed = false;
                    continue;
                }
                let rest = flow - from;
                // Принудительный разрыв ВНУТРИ коробки раньше её конца и раньше
                // края колонки — режем ровно там.
                let forced = k
                    .forced
                    .iter()
                    .copied()
                    .find(|&f| f > from + 0.01 && f < flow - 0.01 && f - from <= room + 0.01);
                if let Some(f) = forced {
                    let nf = k
                        .cuts
                        .iter()
                        .find(|&&(need, _)| (need - f).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(f);
                    out.push(Frag { kid, copy, col, y: cur, from, h: f - from });
                    from = nf;
                    if copy + 1 >= limit {
                        y = target;
                        placed = true;
                        break;
                    }
                    copy += 1;
                    col += 1;
                    cur = 0.0;
                    placed = false;
                    continue;
                }
                if rest <= room + 0.01 {
                    out.push(Frag { kid, copy, col, y: cur, from, h: rest });
                    y = cur + rest;
                    placed = true;
                    break;
                }
                // Срез — ПО КРАЮ колонки (css-break-4 §4: slice — правило, не
                // исключение; Blink `FinishFragmentation`). Класс A нужен
                // лишь когда край попал внутрь монолита-потомка или в рамку:
                // тогда разрыв уходит к началу этого диапазона. Точка класса A
                // ровно на краю даёт усечение поля (`nf`).
                let edge = from + room;
                // `break-inside: avoid` — пожелание (css-break-4 §4.4). Коробка
                // ВЫШЕ целого фрагментаинера цельной быть не может: не с верха
                // страницы она уходит на следующую как монолит (ветка
                // `None if placed`), а с верха рвётся как обычная — по точкам
                // класса A, иначе срезом по краю. Так делает Blink: сначала
                // перенос, и только на пустой странице разрыв внутри
                // (`block-page-break-inside-avoid-7/-15-print`).
                let mono = k.monolith && !(paged && flow > target + 0.01 && cur <= 0.01);
                // Точка разреза `a` с усечением поля по классу A (`nf`).
                let at = |a: f32| -> (f32, f32) {
                    let nf = k
                        .cuts
                        .iter()
                        .find(|&&(need, _)| (need - a).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(a);
                    (a, nf)
                };
                let holds = |a: f32, b: f32| a < edge - 0.01 && edge < b - 0.01;
                // Монолит-ПОТОМОК, начатый на верху колонки, не режется краем, а
                // переполняет колонку: кусок идёт до КОНЦА монолита, продолжение —
                // со следующей колонки (css-break-4 §unforced-breaks: «the UA must
                // not break at the top of the page, i.e. it must place at least some
                // content on each fragmentainer»; Blink `fragmentation_utils.cc`
                // `FinishFragmentation`: «If intrinsic block-size is larger than
                // space left, it means that we have some tall unbreakable child
                // content … this fragment will be allowed to take up more space …
                // to encompass the unbreakable content»). Прежде ветка ниже отдавала
                // `None` при `a <= from`, и ребёнок резался по краю колонки прямо
                // сквозь монолит: `monolithic-overflow-003…005.tentative` (два
                // `contain: size` по 100 в колонках по 60), `tall-line-in-short-
                // fragmentainer-000/001` (строка `inline-block` 100 в колонке 50).
                // «Верх колонки» — пустая колонка, нулевой курсор (перед нами только
                // коробки нулевой высоты: разрыв перед монолитом прогресса не даёт,
                // `tall-line-…-000`) либо ПЕРВЫЙ кусок коробки с заданной высотой,
                // которая сама в остаток влезает: её первое содержимое остаётся в
                // колонке, даже переполняя её (Blink `BoxFragmentBuilder::
                // MustStayInCurrentFragmentainer`; `tall-content-inside-constrained-
                // block-000…002`: коробка 25 в остатке 25, внутри `contain: size` 50).
                let at_top = !placed
                    || cur <= 0.01
                    || (flow > k.h + 0.01 && from <= 0.01 && k.h <= room + 0.01);
                let overflow_to = if k.overflow_top && !mono && room > 0.01 && at_top {
                    k.solid
                        .iter()
                        .filter(|&&(a, b)| holds(a, b) && a <= from + 0.01)
                        .map(|&(_, b)| b)
                        .fold(None::<f32>, |m, b| Some(m.map_or(b, |x| x.max(b))))
                } else {
                    None
                };
                // Монолит дотянулся до конца ребёнка — ребёнок кончается в этой
                // колонке, переполнив её (как монолит-ребёнок в ветке `None =>`).
                if overflow_to.is_some_and(|b| b >= flow - 0.01) {
                    out.push(Frag { kid, copy, col, y: cur, from, h: rest });
                    y = cur + rest;
                    placed = true;
                    break;
                }
                let cut = if let Some(b) = overflow_to {
                    Some(at(b))
                } else if mono || room <= 0.01 {
                    None
                } else if paged && k.solid.iter().any(|&(a, b)| holds(a, b)) {
                    // Страницы: край внутри монолитных диапазонов, а они бывают
                    // ВЛОЖЕНЫ (`avoid` ряда/группы объемлет монолиты ячеек,
                    // `table_shape`). Беречь — самый внешний из тех, что
                    // начинаются ниже `from`. Если и внешний уже начат
                    // (`a <= from`), его не сберечь: с непустой страницы —
                    // перенос целиком (`None if placed`), с верха пустой —
                    // ближайшая внутренняя точка, а не срез по краю (Blink
                    // `FinishFragmentation`: срез = `kBreakAppealLastResort`,
                    // `HasEarlyBreak` → `kNeedsEarlierBreak`; css-break-4
                    // §unforced-breaks: «the UA may use the avoids … to weigh
                    // the appropriateness of the new breakpoints»;
                    // `row-page-break-inside-avoid-1`: «3» на третьем листе в
                    // обеих сторонах пары).
                    let outer = k
                        .solid
                        .iter()
                        .filter(|&&(a, b)| holds(a, b))
                        .map(|&(a, _)| a)
                        .fold(f32::MAX, f32::min);
                    if outer > from + 0.01 {
                        Some(at(outer))
                    } else if placed {
                        None
                    } else {
                        k.solid
                            .iter()
                            .filter(|&&(a, b)| holds(a, b) && a > from + 0.01)
                            .map(|&(a, _)| a)
                            .fold(None::<f32>, |m, a| Some(m.map_or(a, |x| x.min(a))))
                            .map(at)
                    }
                } else if let Some(&(a, _)) = k.solid.iter().find(|&&(a, b)| holds(a, b)) {
                    // Колонки — как прежде: первый содержащий диапазон.
                    if a > from + 0.01 { Some(at(a)) } else { None }
                } else {
                    Some(at(edge))
                };
                // Недолаз: на сколько не хватило колонки до ближайшего
                // разреза (или до конца ребёнка).
                let next = k
                    .cuts
                    .iter()
                    .map(|&(need, _)| need - from)
                    .find(|&d| d > room + 0.01)
                    .unwrap_or(rest);
                shortage = shortage.min(next - room);
                match cut {
                    Some((need, nf)) => {
                        out.push(Frag { kid, copy, col, y: cur, from, h: (need - from).max(0.0) });
                        from = nf;
                    }
                    None if !mono && k.cuts.is_empty() && rest > target + 0.01 && room > 0.01 => {
                        // Коробка без точек разреза выше колонки — вид
                        // `slice` по краю (css-break-3 §4).
                        out.push(Frag { kid, copy, col, y: cur, from, h: room });
                        from += room;
                    }
                    None if paged && placed && cur > target + 0.01 => {
                        // Страницы: предыдущий монолит ушёл НИЖЕ края листа.
                        // Его переполнение занимает место на следующих
                        // страницах — ребёнок продолжает с той страницы и той
                        // высоты, где переполнение кончилось (Blink,
                        // crbug 1402540; `monolithic-overflow-001`: ref режет
                        // блок 150vh на 1 + 0.5 страницы, тест с `contain:size`
                        // обязан поставить текст в ту же середину 2-й страницы).
                        let skip = (cur / target).floor();
                        col += skip as usize;
                        cur -= skip * target;
                        placed = cur > 0.01;
                        continue;
                    }
                    None if placed => {
                        // Из непустой колонки — в следующую целиком; поле на
                        // границе колонки съедается.
                        col += 1;
                        cur = 0.0;
                        placed = false;
                        continue;
                    }
                    None if !mono && rest > target + 0.01 && room > 0.01 => {
                        out.push(Frag { kid, copy, col, y: cur, from, h: room });
                        from += room;
                    }
                    None => {
                        // Монолит с верха пустой колонки: остаётся и
                        // переполняет.
                        out.push(Frag { kid, copy, col, y: cur, from, h: rest });
                        y = cur + rest;
                        placed = true;
                        break;
                    }
                }
                if copy + 1 >= limit {
                    // Копий больше нет — остаток за кадром.
                    y = target;
                    placed = true;
                    break;
                }
                copy += 1;
                col += 1;
                cur = 0.0;
                placed = false;
            }
            // Откат курсора на конец КОРОБКИ: параллельный поток уехал
            // дальше, но сосед по css-break-3 §3 продолжается там, где
            // кончилась коробка. Ищем кусок ЭТОГО ЖЕ ребёнка, внутрь
            // которого попал `k.h`; если поток оборвался раньше (кончились
            // копии), курсор остаётся где был.
            if flow > k.h + 0.01 {
                if let Some(f) = out
                    .iter()
                    .rev()
                    .take_while(|f| f.kid == kid)
                    .find(|f| f.from <= k.h + 0.01 && k.h <= f.from + f.h + 0.01)
                {
                    col = f.col;
                    y = f.y + (k.h - f.from);
                    placed = true;
                }
            }
            prev_mb = k.mb;
            first = false;
        }
        // Курсор мог быть откачен назад параллельным потоком: колонок нужно
        // столько, сколько занял самый дальний КУСОК, а не сколько прошёл
        // курсор. Без потока значение тождественно прежнему: курсор всегда
        // не меньше любого `f.col`.
        let last = out.iter().map(|f| f.col).fold(col, usize::max);
        (last + 1, shortage, out)
    }

    /// Первая граница плана, нарушающая правило 1 css-break-4 §4.3: коробка
    /// `i` начинает НЕ ту колонку, где кончилась `i-1`, хотя разрыв между
    /// ними запрещён (`break-before: avoid*` у неё либо `break-after:
    /// avoid*` у предыдущей). Возвращает `i`; `None` — нарушений нет.
    ///
    /// Принудительный разрыв сильнее запрета («at least one of them forces a
    /// break»), поэтому такие пары пропускаются — на этом стоят зелёные
    /// `break-between-avoid-005/006`, `break-after-table-cell`,
    /// `grid-item-fragmentation-032/044`, где рядом с `avoid` написан
    /// `break-*: column`. Пропускается и разрезанная предыдущая коробка:
    /// разрыв всё равно внутри неё.
    fn first_avoid_violation(kids: &[Kid], plan: &[Frag]) -> Option<usize> {
        for i in 1..kids.len() {
            if !(kids[i].avoid_before || kids[i - 1].avoid_after) {
                continue;
            }
            if kids[i].force_before || kids[i - 1].force_after {
                continue;
            }
            // Граница класса A между соседями стоит на низу КОРОБКИ, а не
            // на конце её параллельного потока (css-break-3 §3): куски за
            // `k.h` — уже отдельный поток, и колонку границы они не задают.
            // Без отсечки разрезанный ПОТОК выглядел как разрезанная
            // коробка, проверка «предыдущая сама разрезана» глушила
            // нарушение, и отступ не срабатывал вовсе — так терялись
            // `break-between-avoid-013/014` (поток 60 и 40 не влезал в
            // остаток колонки), тогда как `-011` уцелела: там поток 40 в
            // остаток 50 влезал и коробка оставалась целой.
            // Без потока условие тождественно прежнему: кусок пушится,
            // только пока `from < k.h` (`rest = flow - from > 0`), а нулевая
            // коробка проходит вторым слагаемым.
            let box_h = kids[i - 1].h;
            let prev: Vec<usize> = plan
                .iter()
                .filter(|f| f.kid == i - 1 && (f.from < box_h - 0.01 || f.from <= 0.01))
                .map(|f| f.col)
                .collect();
            let cur: Vec<usize> = plan.iter().filter(|f| f.kid == i).map(|f| f.col).collect();
            let (Some(&prev_start), Some(&prev_end), Some(&cur_start)) =
                (prev.iter().min(), prev.iter().max(), cur.iter().min())
            else {
                continue;
            };
            // Разрыва на этой границе нет — правило не нарушено.
            if cur_start == prev_end {
                continue;
            }
            // Предыдущая сама разрезана: разрыв внутри неё, отступать некуда.
            if prev_start != prev_end {
                continue;
            }
            return Some(i);
        }
        None
    }

    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (30.09): балансировка снимает нарушение
    // `break-*: avoid` РОСТОМ колонок (Blink `has_violating_break` →
    // `minimal_space_shortage`, `scout-multicol-rest-2026-09-30.md` P5).
    // Свод v215 и проба: +2 (`balance-break-avoidance-001/002`), −4 —
    // `flex-container-fragmentation-003/004` и `grid-lanes-container-
    // fragmentation-003/004` (0.27 → «красное видно»): при заданной высоте
    // 100 и `break-before: avoid` колонки вырастали за коробку. Рост должен
    // упираться в `column-height`/высоту коробки — без этого гейта не
    // возвращать.
    /// Ближайшая ВЫШЕ разрешённая граница для отступа от нарушения на `bad`:
    /// наибольшее `j` из `1..bad`, где ни `break-before` коробки `j`, ни
    /// `break-after` коробки `j-1` разрыв не запрещают. `None` — разрешённых
    /// границ нет вовсе, и по css-break-4 §4.3 правило 1 снимается («rules 1,
    /// 2 and 4 are dropped in order to find additional breakpoints»): план
    /// остаётся жадным.
    ///
    /// Ради этой проверки патч и переписан. Без неё отступ уводил в третью
    /// колонку двухколоночный `break-between-avoid-002` (ЧЕТЫРЕ коробки с
    /// `break-before: avoid; break-after: avoid` подряд — запрещены ВСЕ
    /// границы, отступать некуда), то есть ронял зелёную пару. А обход
    /// границ подряд, а не одной, берёт `break-between-avoid-014`: там
    /// граница перед третьей коробкой тоже запрещена, и разрыв обязан
    /// уехать сразу на вторую.
    fn retreat_to(kids: &[Kid], bad: usize) -> Option<usize> {
        (1..bad)
            .rev()
            .find(|&j| !kids[j].avoid_before && !kids[j - 1].avoid_after)
    }

    /// `fill_at` с соблюдением правила 1 css-break-4 §4.3: пока план рвёт
    /// запрещённую границу, разрыв ПЕРЕНОСИТСЯ на ближайшую разрешённую выше
    /// — коробке там ставится принудительный разрыв перед собой, и укладка
    /// повторяется. Это ручная запись blink-овского `early_break_`
    /// (`block_layout_algorithm.cc:1086`, `fragmentation_utils.cc:1250
    /// UpdateEarlyBreakAtBlockChild`): там алгоритм помнит лучшую точку и
    /// переукладывает поддерево, здесь — плоский повтор по списку.
    ///
    /// Метка ОДНА и только двигается назад (`next < m`), а не копится:
    /// накопленные `force_before` ставились разом на двух соседей и разводили
    /// по колонкам ровно ту пару, которую запрет велит держать вместе
    /// (`break-between-avoid-014`: выходило `A | B` `C+D`, а надо `A | B+C+D`).
    ///
    /// Быстрый выход: если запретов нет ни у кого — ровно прежний `fill_at`,
    /// без единой лишней копии `Kid`. Запреты написаны в 101 паре свода из
    /// 23108, у остальных арифметика тождественна прежней.
    fn fill_avoiding(
        kids: &[Kid],
        target_at: &dyn Fn(usize) -> f32,
        limit: usize,
        paged: bool,
    ) -> (usize, f32, Vec<Frag>) {
        if !kids.iter().any(|k| k.avoid_before || k.avoid_after) {
            return Self::fill_at(kids, target_at, limit, paged);
        }
        let mut best = Self::fill_at(kids, target_at, limit, paged);
        let mut mark: Option<usize> = None;
        for _ in 0..kids.len().min(8) {
            let Some(bad) = Self::first_avoid_violation(kids, &best.2) else {
                return best;
            };
            let Some(next) = Self::retreat_to(kids, bad) else {
                return best;
            };
            // Метка только назад — иначе цикл вечен, а план качается.
            if mark.is_some_and(|m| next >= m) {
                return best;
            }
            mark = Some(next);
            let mut work: Vec<Kid> = kids.to_vec();
            work[next].force_before = true;
            best = Self::fill_at(&work, target_at, limit, paged);
        }
        best
    }

    /// Укладка стопки: её высота, линии колонок `(y, высота)`, план кусков и
    /// спаннеры `(ребёнок, y)`. Без рядов — одна линия, как в css-multicol-1
    /// (прежний `balance`).
    /// ★ ЗАМЕРЕНО И ОТКАЧЕНО (10.09, v205/v206, `scout-fragoof-2026-09d.md`):
    /// оба патча захода по внепоточным в многоколоночнике.
    /// №2 «переполняющие колонки» (число копий не упирается в `column-count`
    /// при `column-fill: auto` с заданной высотой, css-multicol-1 §Overflow):
    /// вместе с №1 срез 826 пар дал **+18/−31**.
    /// №1 «статическая позиция прямого внепоточного ребёнка» (щуп нулевой
    /// записью стопки, коробка заместителем `spot_place` после стопки):
    /// в одиночку **+11/−17**.
    /// Потери у обоих одни и те же и лежат в БАЛАНСИРОВКЕ:
    /// `column-height-002/003/004/021/022`, `multicol-containing-003`,
    /// `multicol-fill-balance-038` уходят в «красное видно»,
    /// `multi-line-*-flex-fragmentation-*` разъезжаются. Нулевая запись стопки
    /// и лишние копии одинаково сбивают план балансировки: она считает
    /// содержимое по числу записей, а внепоточная в них не участвует.
    /// Возвращать только вместе с разделением «переполнение» и
    /// «балансировка» в самой `balance`.
    fn balance(&self, kids: &[Kid]) -> (f32, Vec<(f32, f32)>, Vec<Frag>, Vec<(usize, f32)>) {
        let count = self.count;
        // Потолок баланса без рядов (`Rows::cap`): баланс как прежде, но не
        // выше высоты коробки; копий — сколько построил `render.rs`, лишние
        // колонки переполняют вбок (`place`: `(col, 0.0)`). Линия — высотой
        // в баланс, не в потолок: по ней `growths` меряет рост.
        if let Some(Rows { h: Some(lim), cap: true, .. }) = self.rows {
            let (h, plan) = self.balance_line(kids, self.copies, Some(lim.max(1.0)));
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        let Some(rows) = self.rows else {
            // Предел копий поднимает ТОЛЬКО `column-fill: auto` с заданной
            // высотой: там `balance_line` первой строкой уходит в
            // `fill_avoiding` и высоту не подбирает. У балансировки предел
            // остаётся `count` — её условие выхода `cols <= self.count`
            // (Blink `ResolveColumnAutoBlockSize`) от числа копий зависеть не
            // должно.
            let limit = if self.fixed_height.is_some() {
                self.copies.max(count)
            } else {
                count
            };
            let (h, plan) = self.balance_line(kids, limit, None);
            return (h, vec![(0.0, h)], plan, Vec::new());
        };
        let limit = self.copies;
        let Some(h) = rows.h else {
            // Ряд без потолка: колонки балансируются одной линией, а лишние
            // (от принудительных разрывов) идут рядами ниже; высота ряда — по
            // его содержимому (Blink: `HasRowHeight()` ложь, `OffsetToNextRow`
            // = один `row_gap`; `column-wrap-no-constraints-001/002`).
            let (_, plan) = self.balance_line(kids, limit, None);
            let n = plan.iter().map(|f| f.col / count + 1).max().unwrap_or(1);
            let mut out = Vec::with_capacity(n);
            let mut y = 0.0f32;
            for r in 0..n {
                let rh = plan
                    .iter()
                    .filter(|f| f.col / count == r)
                    .map(|f| f.y + f.h)
                    .fold(0.0f32, f32::max);
                out.push((y, rh));
                y += rh + if r + 1 < n { rows.gap } else { 0.0 };
            }
            return (y, out, plan, Vec::new());
        };
        // Blink `ClampedToValidFragmentainerCapacity`: нулевая колонка всё
        // равно вмещает 1px, иначе укладка не сдвинется с места
        // (`columns: 2 / 0`, `column-height-021…023`).
        let cap = h.max(1.0);
        if !rows.wrap {
            // `nowrap` с заданным `column-height`: одна линия высотой ровно в
            // него, баланс не выше потолка (Blink `ConstrainColumnBlockSize`:
            // «Never become taller than used column-height»), лишние
            // колонки — вбок (§8.2 уровня 1; `column-height-005/030`).
            let target = match self.fixed_height {
                Some(_) => cap,
                None => self.balance_line(kids, limit, Some(cap)).0,
            };
            let (_, _, plan) = Self::fill(kids, target, limit, false);
            return (h, vec![(0.0, h)], plan, Vec::new());
        }
        // Курсор по коробке (Blink `intrinsic_block_size_`): сетка рядов с
        // шагом `h + row-gap`; фаза курсора — смещение в текущем ряду
        // (`OffsetInCurrentRow`), остаток ряда `h − фаза` (отрицателен в
        // зазоре). Дети идут ПРОБЕГАМИ: линии колонок между спаннерами и сами
        // спаннеры (`LayoutChildren` → `LayoutFragmentationContext` /
        // `LayoutSpanner`). Без спаннеров — один пробег, план шага 1.
        let stride = h + rows.gap;
        let phase = |y: f32| if stride > 0.0 { y % stride } else { 0.0 };
        // `OffsetToNextRow`: остаток ряда + зазор; ровно с начала ряда — один
        // зазор (так у Blink; на практике сюда не попадаем).
        let next_row = |y: f32| {
            let p = phase(y);
            if p > 0.01 { y - p + stride } else { y + rows.gap }
        };
        let mut y = 0.0f32;
        let mut lines: Vec<(f32, f32)> = Vec::new();
        let mut plan: Vec<Frag> = Vec::new();
        let mut spans: Vec<(usize, f32)> = Vec::new();
        let mut i = 0usize;
        while i < kids.len() {
            if kids[i].span {
                // Спаннер (§ch: выше ряда — переполняет в следующий, «crossing
                // any row-gap»). Не с начала ряда и не влезает в остаток —
                // со следующего ряда (`LayoutSpanner`: «Not enough room for
                // the spanner in the current row, and we're not at the
                // beginning of the row. Try at the next row»). Поля спаннера —
                // без схлопывания с соседями (упрощение; в тестах кластера
                // нулевые).
                let k = &kids[i];
                let mut at = y + k.mt;
                let p = phase(at);
                if p > 0.01 && h - p < k.h - 0.01 {
                    at = next_row(at);
                }
                spans.push((i, at));
                y = at + k.h + k.mb;
                i += 1;
                continue;
            }
            let j = kids[i..].iter().position(|k| k.span).map_or(kids.len(), |p| i + p);
            // Первая линия пробега — в остаток текущего ряда; остатка нет
            // (курсор в зазоре после спаннера) — со следующего ряда
            // (`LayoutFragmentationContext`: «if there's no room in the
            // current row (because of a preceding spanner, typically)»;
            // нулевой ряд — исключение, `RowHeight() > LayoutUnit()`).
            let mut p = phase(y);
            if h > 0.0 && h - p <= 0.01 {
                y = next_row(y);
                p = 0.0;
            }
            let first = (h - p).max(1.0);
            let row_start = y - p;
            // Линия перед спаннером балансируется всегда, даже при
            // `column-fill: auto` (Blink `:1070`: «We always have to balance
            // columns preceding a spanner»; css-gaps `multicol-gap-
            // decorations-011`: 120px в трёх колонках по 40, а не 60 + 60).
            let balance_last = self.fixed_height.is_none() || j < kids.len();
            let base = lines.len() * count;
            let (n, frags, tail) = self.balance_run(&kids[i..j], first, cap, balance_last);
            for f in frags {
                plan.push(Frag { kid: f.kid + i, col: f.col + base, ..f });
            }
            for l in 0..n {
                let ly = if l == 0 { y } else { row_start + l as f32 * stride };
                let lh = if l + 1 == n { tail } else if l == 0 { first } else { cap };
                lines.push((ly, lh));
            }
            // Курсор — за последней линией: сбалансированная короче ряда, и
            // следующий спаннер ложится прямо за ней (Blink `:1249`).
            y = lines.last().map_or(y, |&(ly, lh)| ly + lh);
            i = j;
        }
        // Последний ряд занимает всю `column-height`, даже если содержимое
        // короче (§ch: «empty space is left»; Blink `Layout()`: «Use all of
        // column-height on the last row as well» — прибавляется и
        // отрицательный остаток: `columns: 2 / 0` даёт `(n − 1) · gap`, как в
        // шаге 1).
        let p = phase(y);
        if p > 0.01 {
            y += h - p;
        }
        (y, lines, plan, spans)
    }

    /// Точки роста от вытолкнутых монолитов (Blink `FinishFragmentation`,
    /// `fragmentation_utils.cc:641-656`: непоследний фрагмент коробки =
    /// `space_left`; css-flexbox-1 §fragmentation: «A forced break inside a
    /// flex item effectively increases the size of its contents»). Для
    /// каждого куска плана, у которого есть продолжение и который кончается
    /// ровно в НАЧАЛЕ монолитного диапазона (`fill_at`: `holds` → `at(a)`),
    /// а не на принудительном разрыве, — `(ребёнок, смещение разреза,
    /// недобор до низа колонки)`. Только меры, без ширины: `render.rs`
    /// ставит по ним распорки в копии ДО сборки (`grow_pushed`), после чего
    /// монолит стоит ровно на краю и рост здесь выходит нулевым.
    /// Четвёртое поле — «разрыв принудительный»: там распорка обязана быть
    /// ОТДЕЛЬНОЙ КОРОБКОЙ, а не полем. Точка `forced` в мере стоит ПЕРЕД
    /// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, следом
    /// `forced.push(y)`), и рост поля её не двигает — проба
    /// `target/probe-9g/p-single-line-column-flex-fragmentation-022.html`
    /// осталась красной тем же прямоугольником, а `p2-…` с коробкой-распоркой
    /// дала 0.00.
    pub(crate) fn growths(
        kids: &[Kid],
        count: usize,
        fixed_height: Option<f32>,
        rows: Option<Rows>,
        copies: usize,
    ) -> Vec<(usize, f32, f32, bool)> {
        let probe = ColumnStack {
            children: Vec::new(),
            count: count.max(1),
            gap: 0.0,
            fixed_height,
            rule: None,
            rows,
            copies: copies.max(1),
            gap_items: None,
            intrinsic: None,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        };
        let (_, lines, plan, _) = probe.balance(kids);
        let count = probe.count;
        let mut out = Vec::new();
        for f in &plan {
            let k = &kids[f.kid];
            // У `clone` `from` — по СОДЕРЖИМОМУ, а `h` уже растянут до низа
            // колонки: `from + h` в координатах `solid`/`forced` смысла не
            // имеет, и распорка роста не нужна — фрагмент и так во всю колонку.
            if k.clone_dec.is_some() {
                continue;
            }
            let end = f.from + f.h;
            // Непоследний фрагмент — есть следующая копия. Либо кусок оборван
            // ПРИНУДИТЕЛЬНЫМ разрывом, когда копии кончились (`fill_at`: «Копий
            // больше нет — остаток за кадром»): содержимое после разрыва уходит в
            // переполняющую колонку, а коробка всё равно занимает остаток своей
            // (css-break-3 §box-splitting: «its content box extends to fill any
            // remaining fragmentainer extent»; Blink
            // `ConsumeRemainingFragmentainerSpace`, `block_layout_algorithm.cc:3129`,
            // вызов при принудительном разрыве `:3212`). Без этого фон обёртки во
            // второй колонке `multicol-fill-balance-041` обрывался на 40 из 100.
            let has_next = plan.iter().any(|g| g.kid == f.kid && g.copy == f.copy + 1);
            let cut_short =
                end < k.h - 0.01 && k.forced.iter().any(|&x| (x - end).abs() < 0.01);
            if !has_next && !cut_short {
                continue;
            }
            // Принудительный разрыв внутри коробки — такой же НЕпоследний
            // фрагмент, как выталкивание монолита: Blink
            // `fragmentation_utils.cc` `FinishFragmentation` даёт ему
            // `min(desired, space_left)`, то есть остаток фрагментаинера
            // целиком (css-flexbox-1 §pagination: «A forced break inside a
            // flex item effectively increases the size of its contents»).
            // Прежде принудительные разрывы отвергались, и фон коробки
            // обрывался на точке разрыва (`single-line-column-flex-
            // fragmentation-022`: колонка 1 красная 50..100).
            let at_solid = k.solid.iter().any(|&(a, _)| (a - end).abs() < 0.01);
            let at_forced = k.forced.iter().any(|&x| (x - end).abs() < 0.01);
            if !at_solid && !at_forced {
                continue;
            }
            let Some(&(_, line_h)) = lines.get(f.col / count) else {
                continue;
            };
            let grow = line_h - f.y - f.h;
            if grow > 0.01 {
                // Монолит растёт от НАЧАЛА своего диапазона; принудительный
                // разрыв стоит ПЕРЕД полем следующей коробки, и распорку надо
                // ставить перед самой коробкой — её верх это `nf` из `cuts`
                // (то же продолжение, что берёт `fill_at`). Парная запись
                // `cuts` у такой точки есть всегда: `shape_full` кладёт
                // `cuts.push((y, y + lead))` и `forced.push(y)` в одном
                // блоке `if !first`.
                let at = if at_solid {
                    end
                } else {
                    k.cuts
                        .iter()
                        .find(|&&(need, _)| (need - end).abs() < 0.01)
                        .map(|&(_, nf)| nf)
                        .unwrap_or(end)
                };
                out.push((f.kid, at, grow, at_forced && !at_solid));
            }
        }
        out
    }

    /// `box-decoration-break: clone`: геометрия фрагментов КАЖДОГО ребёнка —
    /// по копиям `(съеденное содержимое, высота фрагмента)`. Нужна
    /// `render.rs` ДО сборки копий: фрагмент `clone` строится отдельной
    /// коробкой своей высоты. Щуп тот же, что у `growths`.
    pub(crate) fn frags_of(
        kids: &[Kid],
        count: usize,
        fixed_height: Option<f32>,
        rows: Option<Rows>,
        copies: usize,
    ) -> Vec<Vec<(f32, f32)>> {
        let probe = ColumnStack {
            children: Vec::new(),
            count: count.max(1),
            gap: 0.0,
            fixed_height,
            rule: None,
            rows,
            copies: copies.max(1),
            gap_items: None,
            intrinsic: None,
            plan: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            lines_plan: std::cell::RefCell::new(Vec::new()),
            spans_plan: std::cell::RefCell::new(Vec::new()),
        };
        let (_, _, plan, _) = probe.balance(kids);
        let mut out: Vec<Vec<(f32, f32)>> = vec![Vec::new(); kids.len()];
        for f in &plan {
            let v = &mut out[f.kid];
            if v.len() <= f.copy {
                v.resize(f.copy + 1, (0.0, 0.0));
            }
            v[f.copy] = (f.from, f.h);
        }
        out
    }

    /// Начальная высота балансировки — по ПРОГОНАМ содержимого между
    /// принудительными разрывами (Blink `ResolveColumnAutoBlockSizeInternal`,
    /// `column_layout_algorithm.cc:1532`, `ContentRuns`: «A content run starts out
    /// as representing one single column, and we'll add as many additional
    /// implicit breaks as needed into the content runs that are the tallest
    /// ones»). css-multicol-1 §Filling Columns: «minimize variations in column
    /// height, while honoring forced breaks». Прогон рвётся на `force_before`/
    /// `force_after` детей и на внутренних `forced` (продолжение — с `nf` из
    /// `cuts`, поле на разрыве усекается). Поля между детьми не считаются — как в
    /// прежней сумме `k.h`: без принудительных разрывов прогон один, и итог
    /// тождественно прежний `total / count`.
    /// Прежняя сумма принимала план, где у единственного ребёнка кончились копии
    /// (`fill_at`: «Копий больше нет — остаток за кадром»): `multicol-fill-
    /// balance-041` (20 | 40 | 100 в двух колонках) брал 80 вместо 100,
    /// `multicol-fill-auto-004` (10|10|10|10|100 в пяти) — 28 вместо 100.
    fn runs_guess(kids: &[Kid], count: usize) -> f32 {
        let mut runs: Vec<f32> = Vec::new();
        let mut cur = 0.0f32;
        let mut started = false;
        let mut force_next = false;
        for k in kids {
            if (k.force_before || force_next) && started {
                runs.push(cur);
                cur = 0.0;
            }
            force_next = k.force_after;
            started = true;
            let mut from = 0.0f32;
            for &f in k.forced.iter().filter(|&&f| f > 0.01 && f < k.h - 0.01) {
                cur += (f - from).max(0.0);
                runs.push(cur);
                cur = 0.0;
                from = k
                    .cuts
                    .iter()
                    .find(|&&(need, _)| (need - f).abs() < 0.01)
                    .map_or(f, |&(_, nf)| nf);
            }
            cur += (k.h - from).max(0.0);
        }
        runs.push(cur);
        // `DistributeImplicitBreaks`: очередной неявный разрыв — в прогон с самой
        // высокой колонкой на данный момент.
        let mut split = vec![1usize; runs.len()];
        for _ in runs.len()..count.max(1) {
            let i = (0..runs.len())
                .max_by(|&a, &b| {
                    (runs[a] / split[a] as f32).total_cmp(&(runs[b] / split[b] as f32))
                })
                .unwrap_or(0);
            split[i] += 1;
        }
        (0..runs.len())
            .map(|i| runs[i] / split[i] as f32)
            .fold(0.0f32, f32::max)
    }

    /// Одна линия колонок: высота заданная (fill:auto) либо баланс «оценка +
    /// добавка на минимальный недолаз» (blink `ResolveColumnAutoBlockSize`);
    /// `cap` — потолок баланса (`ConstrainColumnBlockSize`).
    fn balance_line(&self, kids: &[Kid], limit: usize, cap: Option<f32>) -> (f32, Vec<Frag>) {
        if let Some(h) = self.fixed_height {
            // Правило 1 css-break-4 §4.3 применяется ТОЛЬКО здесь —
            // `column-fill: auto` с заданной высотой колонки. Балансировку
            // (`ResolveColumnAutoBlockSize` ниже) и пробег рядов
            // (`balance_run`) отступ не трогает НАМЕРЕННО: там лишняя колонка
            // от отступа растит `target` на весь недолаз и МЕНЯЕТ высоту
            // многоколоночника. Считано руками на `balance-break-avoidance-002`
            // (сегодня 0.53): баланс уходит 75 -> 125 при верных 100 — то есть
            // в балансе отступ мало поставить, надо ещё выбрать высоту, а это
            // отдельный шаг с отдельным замером.
            let (_, _, slots) = Self::fill_avoiding(kids, &|_| h, limit, false);
            return (h, slots);
        }
        // Оценка — по прогонам между принудительными разрывами (`runs_guess`); без
        // них это прежняя `сумма / count`.
        let guess = Self::runs_guess(kids, self.count);
        // Разрезаемая коробка потолка колонке не задаёт: её высоту держит
        // только сумма. Потолок нужен монолитам — они остаются целыми.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v103, `scout-break-2026-09d.md` F2):
        // баланс не короче самого длинного `solid`-диапазона внутри (Blink
        // `ConstrainColumnBlockSize`). Срез css-break+CSS2+multicol 2874: +0/−1
        // (`multicol-overflow-clip` 0.00 → 5.22).
        let tallest = kids
            .iter()
            .filter(|k| k.monolith)
            .fold(0.0f32, |m, k| m.max(k.h));
        let clamp = |t: f32| cap.map_or(t, |c| t.min(c));
        let mut target = clamp(guess.max(tallest).max(1.0));
        for _ in 0..6 {
            let (cols, shortage, slots) = Self::fill(kids, target, limit, false);
            if cols <= self.count {
                return (target, slots);
            }
            // Недолаза не было ни у одной коробки: `shortage` так и остался
            // сторожевым `f32::MAX`. Значит лишние колонки родились
            // ПРИНУДИТЕЛЬНЫМИ разрывами, и растягивать нечего —
            // css-multicol-1 §7: «minimize variations in column height, while
            // honoring forced breaks»; Blink: `if (used_column_count_ <=
            // forced_break_count + 1) break;`.
            // Прежняя строка спрашивала `shortage.is_finite()`, а `f32::MAX` —
            // КОНЕЧНОЕ число: `target += f32::MAX` уводил высоту колонки в
            // `f32::MAX`, затем в бесконечность (`multicol-fill-balance-002`,
            // «Don't overstretch»).
            // Упёрлись в потолок — выше колонкам нельзя, остаток уходит вбок.
            if shortage >= f32::MAX {
                // Лишние колонки — только от принудительных разрывов: каждая
                // колонка держит свой прогон целиком, и высота линии — самый
                // высокий из них (Blink `ContentRuns::DistributeImplicitBreaks`
                // при числе прогонов больше колонок; css-multicol-1 §7
                // «honoring forced breaks»). Стартовая оценка `total / count`
                // выше любого прогона (`grid-item-fragmentation-039`:
                // `columns: 1`, прогоны 100 + 100 — колонка 200 вместо 100).
                let used = slots.iter().map(|f| f.y + f.h).fold(0.0f32, f32::max);
                if used > 0.01 && used < target - 0.01 {
                    let (cols2, _, slots2) = Self::fill(kids, used, limit, false);
                    if cols2 == cols {
                        return (used, slots2);
                    }
                }
                return (target, slots);
            }
            if cap.is_some_and(|c| target >= c) {
                return (target, slots);
            }
            // Как blink: расти ровно на минимально необходимое.
            target = clamp(target + if shortage > 0.0 { shortage } else { 1.0 });
        }
        let (_, _, slots) = Self::fill(kids, target, limit, false);
        (target, slots)
    }

    /// Пробег линий колонок между спаннерами: первая линия высотой `first`
    /// (остаток текущего ряда), остальные — `cap`; последняя при
    /// `balance_last` балансируется (Blink балансирует КАЖДУЮ линию и режет
    /// её остатком ряда, так что полные ряды выходят ровно в `column-height`,
    /// а последняя — по содержимому: `column-height-003` — 80px остатка в
    /// двух колонках по 40, а не 50 + 30). Возвращает число линий, план
    /// (колонки от нуля, дети от нуля) и высоту последней линии.
    fn balance_run(
        &self,
        kids: &[Kid],
        first: f32,
        cap: f32,
        balance_last: bool,
    ) -> (usize, Vec<Frag>, f32) {
        let count = self.count;
        let limit = self.copies;
        let line_h = |l: usize| if l == 0 { first } else { cap };
        let (cols, _, plan) = Self::fill_at(kids, &|c| line_h(c / count), limit, false);
        let n = cols.div_ceil(count).max(1);
        let last = line_h(n - 1);
        if !balance_last {
            return (n, plan, last);
        }
        let full = (n - 1) * count;
        let rem: f32 = plan.iter().filter(|f| f.col >= full).map(|f| f.h).sum();
        let tallest = plan
            .iter()
            .filter(|f| f.col >= full && kids[f.kid].monolith)
            .map(|f| f.h)
            .fold(0.0f32, f32::max);
        let mut t = (rem / count as f32).ceil().max(tallest).max(1.0);
        if t >= last {
            return (n, plan, last);
        }
        for _ in 0..6 {
            let at = |c: usize| if c < full { line_h(c / count) } else { t };
            let (cols, shortage, slots) = Self::fill_at(kids, &at, limit, false);
            if cols <= full + count {
                return (n, slots, t);
            }
            if shortage >= f32::MAX {
                break;
            }
            t += if shortage > 0.0 { shortage } else { 1.0 };
            if t >= last {
                break;
            }
        }
        (n, plan, last)
    }
}

impl Element for ColumnStack {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let heights: Vec<Kid> = self
            .children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
            })
            .collect();
        let count = self.count;
        let fixed = self.fixed_height;
        let gap = self.gap;
        let rows = self.rows;
        let copies = self.copies;
        // Внутренние размеры многоколоночного контейнера. Спека их не
        // определяет (css-multicol-1 §3.4: «This specification does not
        // define how U is calculated»), единственное письменное определение —
        // css-sizing-4 `intrinsic-sizing-notes.bs` §multicol-intrinsic; его же
        // держит Blink (`column_layout_algorithm.cc:433`
        // `ComputeMinMaxSizes`). Прежде замер отдавал НОЛЬ, и всякий
        // многоколоночник, чью ширину решает содержимое (плавающий, строчная
        // коробка, элемент гибкого контейнера или сетки), схлопывался в
        // отбивку: `intrinsic-size-001` — зелёная коробка 60×100 вместо
        // 100×100, эталоны `column-grid-lanes-container-baseline-*` — полоса
        // в 25 px вместо 320.
        //
        // Детей меряем ТОЛЬКО когда ширину решает содержимое: обычному
        // блочному контейнеру её даёт родитель, и второй проход раскладки там
        // ничего не даст, кроме времени.
        let intrinsic = self.intrinsic.map(|Intrinsic(col_w)| {
            let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
            let (mut span_min, mut span_max) = (0.0f32, 0.0f32);
            for c in self.children.iter_mut() {
                let mut measure = |el: &mut AnyElement, w: gpui::AvailableSpace| {
                    f32::from(
                        el.layout_as_root(
                            size(w, gpui::AvailableSpace::MaxContent),
                            window,
                            cx,
                        )
                        .width,
                    )
                };
                let mn = measure(&mut c.el, gpui::AvailableSpace::MinContent);
                let mx = measure(&mut c.el, gpui::AvailableSpace::MaxContent);
                // Спаннер идёт во всю ширину коробки: на число колонок он не
                // умножается, а лишь ПОДНИМАЕТ итог — Blink
                // `ComputeSpannersMinMaxSizes` (:523) через
                // `MinMaxSizes::Encompass` (`min_max_sizes.h:26`, это `max`
                // по обеим границам).
                if c.span {
                    span_min = span_min.max(mn);
                    span_max = span_max.max(mx);
                } else {
                    kid_min = kid_min.max(mn);
                    kid_max = kid_max.max(mx);
                }
            }
            let n = count.max(1) as f32;
            let gap_extra = gap * (n - 1.0);
            let (mut mn, mut mx) = (kid_min, kid_max);
            match col_w.filter(|w| *w > 0.0) {
                // «The min-content inline size of a multi-column container
                // with a computed column-width not auto is the smaller of its
                // column-width and the largest min-content inline-size
                // contribution of its contents.»
                Some(w) => {
                    mn = mn.min(w);
                    mx = mx.max(w).max(mn);
                }
                // «…with a computed column-width of auto is the largest
                // min-content inline-size contribution of its contents
                // multiplied by its column-count …, plus its column-gap
                // multiplied by column-count minus 1.» При ЗАДАННОЙ ширине
                // колонки минимум на число колонок не умножается (Blink
                // :482 — «column-count … is ignored in intrinsic min
                // inline-size calculation, if column-width is specified»).
                None => mn = mn * n + gap_extra,
            }
            mx = mx * n + gap_extra;
            (mn.max(span_min), mx.max(span_max))
        });
        let id = window.request_measured_layout(
            gpui::Style::default(),
            move |known, available, _window, _cx| {
                let w = known
                    .width
                    .map(f32::from)
                    .or(match available.width {
                        gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                        gpui::AvailableSpace::MinContent => {
                            intrinsic.map(|(mn, _)| mn)
                        }
                        gpui::AvailableSpace::MaxContent => {
                            intrinsic.map(|(_, mx)| mx)
                        }
                    })
                    .unwrap_or(0.0);
                let probe = ColumnStack {
                    children: Vec::new(),
                    count,
                    gap,
                    fixed_height: fixed,
                    rule: None,
                    rows,
                    copies,
                    gap_items: None,
                    intrinsic: None,
                    plan: std::cell::RefCell::new(Vec::new()),
                    col_w: std::cell::Cell::new(0.0),
                    lines_plan: std::cell::RefCell::new(Vec::new()),
                    spans_plan: std::cell::RefCell::new(Vec::new()),
                };
                let (height, _, _, _) = probe.balance(&heights);
                size(px(w), px(height))
            },
        );
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let w = f32::from(bounds.size.width);
        let col_w = ((w - self.gap * (self.count as f32 - 1.0)) / self.count as f32).max(1.0);
        let heights: Vec<Kid> = self
            .children
            .iter()
            .map(|c| Kid {
                h: c.h,
                mt: c.mt,
                mb: c.mb,
                monolith: c.monolith,
                cuts: c.cuts.clone(),
                force_before: c.force_before,
                force_after: c.force_after,
                avoid_before: c.avoid_before,
                avoid_after: c.avoid_after,
                forced: c.forced.clone(),
                solid: c.solid.clone(),
                span: c.span,
                over: c.over,
                clone_dec: c.clone_dec,
                overflow_top: c.overflow_top,
            })
            .collect();
        let (_, lines, plan, spans) = self.balance(&heights);
        self.col_w.set(col_w);
        *self.lines_plan.borrow_mut() = lines;
        let step = col_w + self.gap;
        for f in &plan {
            // У `clone` копия — САМ фрагмент своей высоты со своими рамками,
            // отбивкой и фоном (`render.rs::clone_fragment`): раскладывать её
            // на полную высоту коробки и поднимать на срез нельзя.
            let clone = self.children[f.kid].clone_dec.is_some();
            let full_h = if clone { f.h } else { self.children[f.kid].h };
            // Колонка в своём ряду: `x` по номеру в ряду, `y` от верха ряда.
            let (c, ry) = self.place(f.col);
            // Сдвиг фрагмента (css-break-3 §5.5) — здесь, а не внутри копии:
            // `layout_as_root` края её КОРНЯ не читает (проба `pm1`).
            let rel = self.children[f.kid].rel;
            let x = bounds.origin.x + px(c as f32 * step + rel.0);
            let y = bounds.origin.y + px(ry + f.y + rel.1);
            let kid = &mut self.children[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её
            // часть делает маска в отрисовке. Иначе половина коробки просто
            // сжалась бы, а не продолжилась в следующей колонке.
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(col_w)),
                    gpui::AvailableSpace::Definite(px(full_h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(point(x, if clone { y } else { y - px(f.from) }), window, cx);
        }
        // Спаннер — во всю ширину коробки, первой копией (запасных у него
        // нет: между колонками он не режется).
        for &(kid, sy) in &spans {
            let kid = &mut self.children[kid];
            let h = kid.h;
            kid.el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(bounds.size.width),
                    gpui::AvailableSpace::Definite(px(h)),
                ),
                window,
                cx,
            );
            kid.el.prepaint_at(point(bounds.origin.x, bounds.origin.y + px(sy)), window, cx);
        }
        // Границы для линеек промежутков (css-gaps-1 §gap-multicol): коробки
        // ЗАНЯТЫХ колонок каждой линии (как Blink `AddCrossGap` на колонку
        // ряда; в третьем ряду `column-height-009` линейка одна) и спаннеры
        // во всю ширину — они обрывают линейки колонок.
        if let Some(items) = &self.gap_items {
            let lines = self.lines_plan.borrow();
            let mut used = vec![0usize; lines.len()];
            for f in &plan {
                if let Some(u) = used.get_mut(f.col / self.count) {
                    *u = (*u).max(f.col % self.count + 1);
                }
            }
            let mut items = items.borrow_mut();
            for (l, &(ly, lh)) in lines.iter().enumerate() {
                for c in 0..used[l].max(1) {
                    items.push(Bounds {
                        origin: point(bounds.origin.x + px(c as f32 * step), bounds.origin.y + px(ly)),
                        size: size(px(col_w), px(lh)),
                    });
                }
            }
            for &(kid, sy) in &spans {
                items.push(Bounds {
                    origin: point(bounds.origin.x, bounds.origin.y + px(sy)),
                    size: size(bounds.size.width, px(self.children[kid].h)),
                });
            }
        }
        *self.plan.borrow_mut() = plan;
        *self.spans_plan.borrow_mut() = spans;
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Линейки — по центрам промежутков, высотой в колонку, в каждой линии.
        // Это простая `column-rule` без рядов; при рядах `render.rs` отдаёт
        // линейки (в том числе `row-rule`, css-multicol-2 §rg) художнику
        // `GapRulePainter` по буферу `gap_items`, и `rule` здесь `None`.
        if let Some((rw, color)) = self.rule {
            let col_w = self.col_w.get();
            let rows = self.lines_plan.borrow().clone();
            // css-multicol-1 §4 (`column-rule`): «Column rules are only drawn
            // between two columns that both have content». Занятость — по
            // плану укладки: наибольшая колонка линии с НЕнулевым куском
            // (щуп статической позиции абсолюта — кусок нулевой высоты).
            // `grid-container-fragmentation-007/008`: 3 колонки из 5, лишние
            // линейки в 3-м и 4-м промежутках — 0.83 %. Без рядов (и при
            // `nowrap`) все колонки — одна линия, переполняющие тоже в ней.
            let wrap = matches!(self.rows, Some(r) if r.wrap);
            let used: Vec<usize> = {
                let plan = self.plan.borrow();
                (0..rows.len())
                    .map(|l| {
                        plan.iter()
                            .filter(|f| {
                                f.h > 0.01 && (if wrap { f.col / self.count } else { 0 }) == l
                            })
                            .map(|f| if wrap { f.col % self.count } else { f.col } + 1)
                            .max()
                            .unwrap_or(0)
                    })
                    .collect()
            };
            for (l, &(ry, rh)) in rows.iter().enumerate() {
                for i in 1..used.get(l).copied().unwrap_or(0).min(self.count) {
                    let cx_ = i as f32 * (col_w + self.gap) - self.gap * 0.5;
                    window.paint_quad(gpui::fill(
                        Bounds {
                            origin: point(
                                bounds.origin.x + px(cx_ - rw * 0.5),
                                bounds.origin.y + px(ry),
                            ),
                            size: size(px(rw), px(rh)),
                        },
                        color,
                    ));
                }
            }
        }
        let plan = self.plan.borrow().clone();
        let col_w = self.col_w.get();
        let step = col_w + self.gap;
        // Маска нужна ТОЛЬКО разрезанному ребёнку. У целого она обрезала бы
        // его собственное переполнение, которого коробка не прячет: отсюда
        // уходили в красное `overflow-clip-004`, `overflow-unsplittable-*`,
        // `overflowing-block-003` и родня.
        let mut parts = vec![0usize; self.children.len()];
        for f in &plan {
            parts[f.kid] += 1;
        }
        for f in plan {
            let (c, ry) = self.place(f.col);
            // Срез едет вместе со сдвинутым фрагментом (css-break-3 §5.5):
            // маска, оставленная на месте колонки, съедала его целиком
            // (`out-of-flow-in-multicolumn-042/045`, проба `pm3`).
            let rel = self.children[f.kid].rel;
            let x = bounds.origin.x + px(c as f32 * step + rel.0);
            let y = bounds.origin.y + px(ry + f.y + rel.1);
            // Маска — устройство `slice`. Фрагмент `clone` самодостаточен:
            // содержимое режет его внутренняя обёртка (`clone_fragment`), а
            // тень/контур обязаны выходить за колонку (`clone-009`).
            let split = parts[f.kid] > 1 && self.children[f.kid].clone_dec.is_none();
            // Срез `slice` (css-break-3 §4) — поперёк БЛОЧНОЙ оси. Вбок колонка
            // переполнение не режет: css-multicol-1 §8.1 «content that extends
            // outside column boxes visibly overflows and is not clipped to the column
            // box» (Blink режет только `overflow` самой коробки). Маска шириной в
            // колонку прятала жёлтую полосу 180px поверх линеек (`column-rule-002`),
            // правую четверть ребёнка 100px в колонке 75
            // (`relative-child-overflowing-column-gap`) и всё содержимое при стопке
            // шириной 0 (`relative-child-overflowing-container`, колонка 1px). Вылет —
            // на ширину окна (у стопки нулевой ширины своей ширины нет); дальше режет
            // маска предка. Заменяет P10 `scout-grid-frag-2026-09-30.md` §5.10.
            let win_w = window.viewport_size().width;
            let spill = if win_w > bounds.size.width { win_w } else { bounds.size.width };
            let mask = gpui::ContentMask {
                bounds: Bounds {
                    origin: point(x - spill, y),
                    size: size(px(col_w) + spill + spill, px(f.h)),
                },
            };
            let kid = &mut self.children[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            // Маска и режет: копия нарисована во всю свою высоту, видна
            // только полоса своей колонки (css-break-3 §4, вид `slice`).
            if split {
                window.with_content_mask(Some(mask), |window| el.paint(window, cx));
            } else {
                el.paint(window, cx);
            }
        }
        let spans = self.spans_plan.borrow().clone();
        for (kid, _) in spans {
            self.children[kid].el.paint(window, cx);
        }
    }
}

impl IntoElement for ColumnStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Лист страницы (css-page-3 §page-model): полный размер, поля, рамка и
/// отступы (верх/право/низ/лево), фон листа, канвас документа и page area —
/// контентная область листа, она же фрагментаинер.
#[derive(Clone, Copy, Debug)]
pub struct PageGeom {
    pub size: (f32, f32),
    pub margin: [f32; 4],
    pub border: (f32, gpui::Hsla),
    pub padding: [f32; 4],
    /// Фон листа — кроет ВЕСЬ лист вместе с полями (§painting, слой 1).
    pub bg: gpui::Hsla,
    /// Канвас документа — фон `html`/`body`; кроет border box листа (слой 2).
    pub canvas: Option<gpui::Hsla>,
    pub area: (f32, f32),
}

impl PageGeom {
    /// Левый верх page area внутри листа.
    fn area_origin(&self) -> (f32, f32) {
        (
            self.margin[3] + self.border.0 + self.padding[3],
            self.margin[0] + self.border.0 + self.padding[0],
        )
    }
}

/// Блок верхнего уровня документа в стопке страниц. Высота заранее НЕ
/// известна и меряется раскладкой (`layout_as_root`) в `prepaint` — так в
/// стопку попадает и голый текст, которого укладка колонок не видит.
pub struct PageKid {
    pub el: AnyElement,
    /// Копии на случай разреза между страницами (см. `StackChild::frags`).
    pub frags: Vec<AnyElement>,
    /// Монолит (css-break-4 §4.1 плюс `contain: size`, как Blink `IsMonolithic`).
    pub monolith: bool,
    /// Принудительный разрыв страницы перед/после (css-break-4 §3.1), включая
    /// смену имени страницы (css-page-3 §"Using named pages", п. 4).
    pub force_before: bool,
    pub force_after: bool,
    /// Мера поддерева от `render::shape_full` (css-break-4 §possible-breaks):
    /// высота в точках, точки законного разреза `(need, from)`, смещения
    /// принудительных разрывов и монолитные диапазоны — как у `StackChild`.
    /// `None` — высота не известна заранее, берётся измеренная, разрезов
    /// внутри нет.
    pub shape: Option<(f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>,
}

/// Стопка страниц: page area каждой — фрагментаинер (css-break-4 §2). Листы
/// раскладываются сеткой и МАСШТАБИРУЮТСЯ до вмещения в свою коробку: стенд
/// сравнивает кадр целиком, а печатный эталон WPT тоже многостраничен, и
/// сравнивать надо все страницы обеих сторон.
pub struct PageStack {
    kids: Vec<PageKid>,
    geom: PageGeom,
    /// Слой начального содержащего блока (внепоточные без позиционированного
    /// предка) — по КОПИИ на страницу: `icb[p]` рисуется на листе `p` со
    /// сдвигом на `p` page area вверх, то есть абсолют раскладывается «как
    /// непрерывный поток» и режется страницами (css-position-3
    /// §abspos-breaking; `monolithic-overflow-013`: текст после монолита
    /// 350vh — в середине четвёртой страницы). Повтор `position: fixed` на
    /// каждой странице без сдвига — шаг 3.
    icb: Vec<Vec<AnyElement>>,
    /// Досягаемость абсолютов корня — низ самого дальнего (с переполнением
    /// монолитов, как Blink `ReserveSpaceForMonolithicOverflow`): листов не
    /// меньше, чем нужно, чтобы её показать.
    icb_reach: f32,
    /// Слой `position: fixed` — по копии на страницу БЕЗ сдвига: содержащий
    /// блок фиксированного — page area каждого листа (Blink `IsMonolithic`:
    /// «IsFixedPositioned() && GetDocument().Printing()» — монолит, повторяемый
    /// на каждой странице; `fixedpos-007..009`).
    fixed: Vec<Vec<AnyElement>>,
    plan: std::cell::RefCell<Vec<Frag>>,
    pages: std::cell::Cell<usize>,
    /// Листов в ряду и масштаб стопки.
    grid: std::cell::Cell<(usize, f32)>,
}

impl PageStack {
    pub fn new(
        kids: Vec<PageKid>,
        geom: PageGeom,
        icb: Vec<Vec<AnyElement>>,
        icb_reach: f32,
        fixed: Vec<Vec<AnyElement>>,
    ) -> Self {
        PageStack {
            kids,
            geom,
            icb,
            icb_reach,
            fixed,
            plan: std::cell::RefCell::new(Vec::new()),
            pages: std::cell::Cell::new(1),
            grid: std::cell::Cell::new((1, 1.0)),
        }
    }

    /// Левый верх листа `i` в НЕмасштабированных точках стопки.
    fn sheet_origin(&self, i: usize) -> (f32, f32) {
        let per_row = self.grid.get().0.max(1);
        (
            (i % per_row) as f32 * self.geom.size.0,
            (i / per_row) as f32 * self.geom.size.1,
        )
    }

    /// Прямоугольник page area листа `i` в ИТОГОВЫХ координатах окна (с
    /// масштабом): шейдер режет по маске после преобразования
    /// (`shaders.hlsl` `distance_from_clip_rect_transformed`).
    fn area_mask(&self, bounds: Bounds<Pixels>, i: usize) -> gpui::ContentMask<Pixels> {
        let s = self.grid.get().1;
        let (sx, sy) = self.sheet_origin(i);
        let (ax, ay) = self.geom.area_origin();
        gpui::ContentMask {
            bounds: Bounds {
                origin: point(
                    bounds.origin.x + px((sx + ax) * s),
                    bounds.origin.y + px((sy + ay) * s),
                ),
                size: size(px(self.geom.area.0 * s), px(self.geom.area.1 * s)),
            },
        }
    }
}

impl Element for PageStack {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // Стопка занимает РОДИТЕЛЯ целиком (окно стенда): число страниц
        // становится известно только после меры детей в `prepaint`, а размер
        // листов подгоняется масштабом, не размером стопки. Измеряемая
        // раскладка здесь не годится: блочный родитель не отдаёт ей
        // определённой высоты, и стопка получала 800x0 — масштаб схлопывался
        // в точку, кадр выходил пустым (первый заход, 06.09).
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let g = self.geom;
        let (aw, ah) = (g.area.0.max(1.0), g.area.1.max(1.0));
        // 1. Мера: ширина — page area, высота — по содержимому. Поля детей
        //    уже внутри их коробок (обёртка `render_paged`), поэтому mt/mb = 0.
        let kids: Vec<Kid> = self
            .kids
            .iter_mut()
            .map(|k| {
                let sz = k.el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                // Мера поддерева даёт точки разреза внутри ребёнка; без неё
                // ребёнок — цельный кусок измеренной высоты.
                let (h, cuts, forced, solid) = match &k.shape {
                    Some((h, cuts, forced, solid)) => {
                        (*h, cuts.clone(), forced.clone(), solid.clone())
                    }
                    None => (f32::from(sz.height), Vec::new(), Vec::new(), Vec::new()),
                };
                Kid {
                    h,
                    mt: 0.0,
                    mb: 0.0,
                    monolith: k.monolith,
                    cuts,
                    force_before: k.force_before,
                    force_after: k.force_after,
                    // Правило 1 §4.3 в ПЕЧАТИ пока не применяется (§2 отчёта):
                    // `PageKid` запретов не носит, а `false` в обоих полях
                    // включает быстрый выход `fill_avoiding` — путь страниц
                    // остаётся байт-в-байт прежним.
                    avoid_before: false,
                    avoid_after: false,
                    forced,
                    solid,
                    span: false,
                    // Страницы параллельный поток пока не берут: у них своё
                    // правило переполнения монолита (`fill_at`, ветка
                    // `paged && placed && cur > target`, crbug 1402540), и
                    // мешать их без отдельного замера печатного среза нельзя.
                    // `over == h` — поток выключен.
                    over: h,
                    // Страницы: `box-decoration-break` пока `slice` — весь
                    // кластер `clone` в колонках.
                    clone_dec: None,
                    // У страниц своё правило переполнения монолита (`fill_at`,
                    // ветка `paged && placed && cur > target`, crbug 1402540).
                    overflow_top: false,
                }
            })
            .collect();
        let limit = self
            .kids
            .iter()
            .map(|k| k.frags.len() + 1)
            .min()
            .unwrap_or(1);
        let (pages, _, plan) = ColumnStack::fill(&kids, ah, limit, true);
        // Абсолюты корня добавляют листы, пока не кончится их досягаемость
        // (Blink: «If overflowed by monolithic overflow, we need more pages»,
        // box_fragment_builder.h). Потолок — число копий слоя.
        let icb_pages = if self.icb.is_empty() {
            0
        } else {
            ((self.icb_reach / ah).ceil().max(0.0) as usize).min(self.icb.len())
        };
        let pages = pages.max(icb_pages).max(1);
        // 2. Сетка листов и масштаб до вмещения в коробку стопки.
        let (ww, wh) = (
            f32::from(bounds.size.width).max(1.0),
            f32::from(bounds.size.height).max(1.0),
        );
        let (pw, ph) = (g.size.0.max(1.0), g.size.1.max(1.0));
        let mut best = (1usize, 0.0f32);
        for per_row in 1..=pages {
            let rows = pages.div_ceil(per_row);
            let s = (ww / (per_row as f32 * pw))
                .min(wh / (rows as f32 * ph))
                .min(1.0);
            if s > best.1 {
                best = (per_row, s);
            }
        }
        self.grid.set(best);
        self.pages.set(pages);
        if std::env::var("HTML_VIEWPORT").is_ok() {
            eprintln!(
                "PAGESTACK bounds={:?} kids={} heights={:?} cuts={:?} shape/mono={:?} forced={:?} area={:?} size={:?} pages={} grid={:?} plan={}",
                bounds,
                kids.len(),
                kids.iter().map(|k| k.h).collect::<Vec<_>>(),
                kids.iter().map(|k| k.cuts.len()).collect::<Vec<_>>(),
                self.kids.iter().map(|k| (k.shape.is_some(), k.monolith)).collect::<Vec<_>>(),
                kids.iter().map(|k| k.forced.len()).collect::<Vec<_>>(),
                g.area,
                g.size,
                pages,
                best,
                plan.len()
            );
        }
        // 3. Копии раскладываются ЦЕЛИКОМ и поднимаются на срез — ровно как
        //    в `ColumnStack::prepaint`; видимую часть делает маска.
        let (ax, ay) = g.area_origin();
        for f in &plan {
            let (sx, sy) = self.sheet_origin(f.col);
            let full_h = kids[f.kid].h;
            let kid = &mut self.kids[f.kid];
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(aw)),
                    gpui::AvailableSpace::Definite(px(full_h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(
                point(
                    bounds.origin.x + px(sx + ax),
                    bounds.origin.y + px(sy + ay + f.y - f.from),
                ),
                window,
                cx,
            );
        }
        // Слой ICB — копия `p` на листе `p`, в его page area, поднятая на `p`
        // высот area: непрерывный поток абсолютов, разрезанный страницами.
        for p in 0..pages.min(self.icb.len()) {
            let (sx, sy) = self.sheet_origin(p);
            let lift = p as f32 * ah;
            for el in &mut self.icb[p] {
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
                el.prepaint_at(
                    point(
                        bounds.origin.x + px(sx + ax),
                        bounds.origin.y + px(sy + ay - lift),
                    ),
                    window,
                    cx,
                );
            }
        }
        // Слой `fixed` — копия `p` на листе `p` без сдвига: page area каждого
        // листа — его содержащий блок.
        for p in 0..pages.min(self.fixed.len()) {
            let (sx, sy) = self.sheet_origin(p);
            for el in &mut self.fixed[p] {
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
                el.prepaint_at(
                    point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
                    window,
                    cx,
                );
            }
        }
        *self.plan.borrow_mut() = plan;
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let g = self.geom;
        let s = self.grid.get().1;
        let pages = self.pages.get();
        // Масштаб вокруг левого верха стопки; матрица — в точках устройства,
        // как у `interact::Transformed` (interact.rs:1152-1188).
        let k = window.scale_factor();
        let dev = |v: f32| px(v).scale(k);
        let origin = point(dev(f32::from(bounds.origin.x)), dev(f32::from(bounds.origin.y)));
        let back = point(dev(-f32::from(bounds.origin.x)), dev(-f32::from(bounds.origin.y)));
        let matrix = gpui::TransformationMatrix::unit()
            .translate(origin)
            .compose(gpui::TransformationMatrix {
                rotation_scale: [[s, 0.0], [0.0, s]],
                translation: [0.0, 0.0],
            })
            .translate(back);
        let plan = self.plan.borrow().clone();
        let masks: Vec<gpui::ContentMask<Pixels>> =
            (0..pages).map(|i| self.area_mask(bounds, i)).collect();
        let rect = |x: f32, y: f32, w: f32, h: f32| Bounds {
            origin: point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
            size: size(px(w), px(h)),
        };
        window.with_transformation(matrix, |window| {
            // Порядок краски css-page-3 §painting: фон листа → канвас
            // документа (border box листа) → рамки → содержимое.
            for i in 0..pages {
                let (sx, sy) = self.sheet_origin(i);
                window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
                let bx = sx + g.margin[3];
                let by = sy + g.margin[0];
                let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
                let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
                if let Some(c) = g.canvas {
                    window.paint_quad(gpui::fill(rect(bx, by, bw, bh), c));
                }
                let (t, c) = g.border;
                if t > 0.0 {
                    for r in [
                        (bx, by, bw, t),
                        (bx, by + bh - t, bw, t),
                        (bx, by, t, bh),
                        (bx + bw - t, by, t, bh),
                    ] {
                        window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), c));
                    }
                }
            }
            // Содержимое — под маской СВОЕГО ФРАГМЕНТА: копия нарисована во
            // всю высоту, видна только полоса `[y, y + h)` этой страницы (вид
            // `slice`, css-break-4 §4; ровно как у `ColumnStack`). Маска по
            // целой page area оставляла на странице хвост следующего
            // фрагмента до края листа (`block-page-break-inside-avoid-7`).
            for f in plan {
                let Some(page) = masks.get(f.col).cloned() else { continue };
                let (sx, sy) = self.sheet_origin(f.col);
                let (ax, ay) = g.area_origin();
                let mask = gpui::ContentMask {
                    bounds: Bounds {
                        origin: point(
                            bounds.origin.x + px((sx + ax) * s),
                            bounds.origin.y + px((sy + ay + f.y) * s),
                        ),
                        size: size(px(g.area.0 * s), px(f.h * s)),
                    }
                    .intersect(&page.bounds),
                };
                let kid = &mut self.kids[f.kid];
                let el = if f.copy == 0 {
                    &mut kid.el
                } else {
                    match kid.frags.get_mut(f.copy - 1) {
                        Some(e) => e,
                        None => continue,
                    }
                };
                // Маски детей (overflow ячеек, полосы таблиц, срезы) заданы в
                // немасштабированных точках — под матрицей стопки они обязаны
                // пройти то же подобие (`Window::with_mask_scale`).
                window.with_content_mask(Some(mask), |window| {
                    window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                });
            }
            for p in 0..pages.min(self.icb.len()) {
                let Some(mask) = masks.get(p).cloned() else { continue };
                for el in &mut self.icb[p] {
                    window.with_content_mask(Some(mask.clone()), |window| {
                        window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                    });
                }
            }
            for p in 0..pages.min(self.fixed.len()) {
                let Some(mask) = masks.get(p).cloned() else { continue };
                for el in &mut self.fixed[p] {
                    window.with_content_mask(Some(mask.clone()), |window| {
                        window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                    });
                }
            }
        });
    }
}

impl IntoElement for PageStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
