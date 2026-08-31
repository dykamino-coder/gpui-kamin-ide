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
fn ellipse_cut(cy_abs: f32, rx: f32, ry: f32, cx: f32, y0: f32, y1: f32) -> f32 {
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
    pub h: f32,
    pub mt: f32,
    pub mb: f32,
}

/// Колонки многоколоночного потока для БЛОЧНЫХ детей с известными
/// высотами (css-multicol §7.4 + css-break): жадная укладка сверху вниз,
/// балансировка «оценка + добавка на минимальный недолаз» (как в blink
/// ResolveColumnAutoBlockSize), монолиты уходят в следующую колонку
/// целиком. Разрез ДЕТЕЙ (строк/рамок) — следующая фаза.
pub struct ColumnStack {
    children: Vec<StackChild>,
    count: usize,
    gap: f32,
    /// `column-fill: auto` + заданная высота: заполнение без баланса.
    fixed_height: Option<f32>,
    /// Линейка между колонками: ширина и цвет.
    rule: Option<(f32, gpui::Hsla)>,
    slots: std::cell::RefCell<Vec<(f32, f32)>>,
    col_w: std::cell::Cell<f32>,
    target: std::cell::Cell<f32>,
}

impl ColumnStack {
    pub fn new(
        children: Vec<StackChild>,
        count: usize,
        gap: f32,
        fixed_height: Option<f32>,
        rule: Option<(f32, gpui::Hsla)>,
    ) -> Self {
        ColumnStack {
            children,
            count: count.max(1),
            gap,
            fixed_height,
            rule,
            slots: std::cell::RefCell::new(Vec::new()),
            col_w: std::cell::Cell::new(0.0),
            target: std::cell::Cell::new(0.0),
        }
    }

    /// Жадная укладка при данной высоте колонки: сколько колонок вышло и
    /// минимальный недолаз. Вертикальные поля соседей СХЛОПЫВАЮТСЯ
    /// (CSS 2.1 §8.3.1) и обнуляются на границе колонки (css-break §5).
    fn fill(kids: &[(f32, f32, f32)], target: f32) -> (usize, f32, Vec<(usize, f32)>) {
        let mut col = 0usize;
        let mut y = 0.0f32;
        let mut prev_mb = 0.0f32;
        let mut first = true;
        let mut shortage = f32::MAX;
        let mut slots = Vec::with_capacity(kids.len());
        for &(h, mt, mb) in kids {
            let lead = if first { mt } else { prev_mb.max(mt) };
            if !first && y + lead + h > target + 0.01 {
                shortage = shortage.min(y + lead + h - target);
                col += 1;
                y = 0.0;
                // Поле на границе колонки съедается.
                slots.push((col, y));
                y += h + 0.0;
                prev_mb = mb;
                first = false;
                continue;
            }
            slots.push((col, y + lead));
            y += lead + h;
            prev_mb = mb;
            first = false;
        }
        (col + 1, shortage, slots)
    }

    /// Высота колонок: заданная (fill:auto) либо баланс.
    fn balance(&self, kids: &[(f32, f32, f32)]) -> (f32, Vec<(usize, f32)>) {
        if let Some(h) = self.fixed_height {
            let (_, _, slots) = Self::fill(kids, h);
            return (h, slots);
        }
        let total: f32 = kids.iter().map(|&(h, ..)| h).sum();
        let tallest = kids.iter().fold(0.0f32, |m, &(h, ..)| m.max(h));
        let mut target = (total / self.count as f32).max(tallest).max(1.0);
        for _ in 0..6 {
            let (cols, shortage, slots) = Self::fill(kids, target);
            if cols <= self.count {
                return (target, slots);
            }
            // Как blink: расти ровно на минимально необходимое.
            target += if shortage.is_finite() { shortage } else { 1.0 };
        }
        let (_, _, slots) = Self::fill(kids, target);
        (target, slots)
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
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let heights: Vec<(f32, f32, f32)> =
            self.children.iter().map(|c| (c.h, c.mt, c.mb)).collect();
        let count = self.count;
        let fixed = self.fixed_height;
        let gap = self.gap;
        let id = window.request_measured_layout(
            gpui::Style::default(),
            move |known, available, _window, _cx| {
                let w = known
                    .width
                    .map(f32::from)
                    .or(match available.width {
                        gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                let probe = ColumnStack {
                    children: Vec::new(),
                    count,
                    gap,
                    fixed_height: fixed,
                    rule: None,
                    slots: std::cell::RefCell::new(Vec::new()),
                    col_w: std::cell::Cell::new(0.0),
                    target: std::cell::Cell::new(0.0),
                };
                let (target, _) = probe.balance(&heights);
                size(px(w), px(target))
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
        let heights: Vec<(f32, f32, f32)> =
            self.children.iter().map(|c| (c.h, c.mt, c.mb)).collect();
        let (target, cols) = self.balance(&heights);
        self.col_w.set(col_w);
        self.target.set(target);
        let slots: Vec<(f32, f32)> = cols
            .iter()
            .map(|&(col, y)| (col as f32 * (col_w + self.gap), y))
            .collect();
        for (c, (sx, sy)) in self.children.iter_mut().zip(slots.iter()) {
            let origin = point(bounds.origin.x + px(*sx), bounds.origin.y + px(*sy));
            c.el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(col_w)),
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
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Линейки — по центрам промежутков, высотой в колонку.
        if let Some((rw, color)) = self.rule {
            let col_w = self.col_w.get();
            let target = self.target.get();
            for i in 1..self.count {
                let cx_ = i as f32 * (col_w + self.gap) - self.gap * 0.5;
                window.paint_quad(gpui::fill(
                    Bounds {
                        origin: point(bounds.origin.x + px(cx_ - rw * 0.5), bounds.origin.y),
                        size: size(px(rw), px(target)),
                    },
                    color,
                ));
            }
        }
        for c in self.children.iter_mut() {
            c.el.paint(window, cx);
        }
    }
}

impl IntoElement for ColumnStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
