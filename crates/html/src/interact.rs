//! Изменение размера мышью — `resize`.
//!
//! Как и переход, требует памяти между кадрами: размер, заданный пользователем,
//! должен пережить перерисовку. Память элемента — единственное такое место в
//! GPUI, поэтому это свой `Element`, а не стиль.
//!
//! Ручка рисуется в углу самим элементом; тянуть её можно по той оси, которую
//! разрешил CSS.

use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Styled, Window, px,
};
use std::rc::Rc;

/// По каким осям разрешено тянуть.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResizeAxis {
    Both,
    Horizontal,
    Vertical,
}

/// Память между кадрами: заданный пользователем размер и состояние перетаскивания.
#[derive(Default, Clone, Copy)]
struct State {
    width: Option<f32>,
    height: Option<f32>,
    dragging: bool,
    /// Размер на момент нажатия — от него считается сдвиг.
    from: (f32, f32),
    at: (f32, f32),
}

/// Сторона квадратной ручки в углу.
const GRIP: f32 = 12.0;

pub struct Resizable {
    id: ElementId,
    axis: ResizeAxis,
    build: Rc<dyn Fn(Option<f32>, Option<f32>) -> AnyElement>,
}

impl Resizable {
    pub fn new(
        id: ElementId,
        axis: ResizeAxis,
        build: Rc<dyn Fn(Option<f32>, Option<f32>) -> AnyElement>,
    ) -> Self {
        Resizable { id, axis, build }
    }
}

impl Element for Resizable {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let build = self.build.clone();
        let Some(global_id) = id else {
            let mut el = build(None, None);
            let layout_id = el.request_layout(window, cx);
            return (layout_id, el);
        };
        window.with_element_state::<State, _>(global_id, |state, window| {
            let st = state.unwrap_or_default();
            let mut el = build(st.width, st.height);
            let layout_id = el.request_layout(window, cx);
            ((layout_id, el), st)
        })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        child.prepaint(window, cx);
        // Область попадания — только уголок: остальная площадь элемента
        // обязана оставаться кликабельной как обычно.
        let grip = Bounds {
            origin: gpui::point(
                bounds.origin.x + bounds.size.width - px(GRIP),
                bounds.origin.y + bounds.size.height - px(GRIP),
            ),
            size: gpui::size(px(GRIP), px(GRIP)),
        };
        window.insert_hitbox(grip, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        grip: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
        let Some(global_id) = id else { return };
        let axis = self.axis;
        let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));

        // Курсор над уголком показывает, что его можно тянуть.
        if grip.is_hovered(window) {
            window.set_cursor_style(
                match axis {
                    ResizeAxis::Horizontal => gpui::CursorStyle::ResizeLeftRight,
                    ResizeAxis::Vertical => gpui::CursorStyle::ResizeUpDown,
                    ResizeAxis::Both => gpui::CursorStyle::ResizeUpLeftDownRight,
                },
                grip,
            );
        }

        let hovered = grip.is_hovered(window);
        window.with_element_state::<State, _>(global_id, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));

            let down = st.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, _window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !hovered {
                    return;
                }
                let mut s = down.get();
                s.dragging = true;
                s.from = (s.width.unwrap_or(size.0), s.height.unwrap_or(size.1));
                s.at = (f32::from(e.position.x), f32::from(e.position.y));
                down.set(s);
            });

            let mv = st.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = mv.get();
                if !s.dragging {
                    return;
                }
                let dx = f32::from(e.position.x) - s.at.0;
                let dy = f32::from(e.position.y) - s.at.1;
                if matches!(axis, ResizeAxis::Both | ResizeAxis::Horizontal) {
                    s.width = Some((s.from.0 + dx).max(GRIP));
                }
                if matches!(axis, ResizeAxis::Both | ResizeAxis::Vertical) {
                    s.height = Some((s.from.1 + dy).max(GRIP));
                }
                mv.set(s);
                window.refresh();
            });

            let up = st.clone();
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = up.get();
                if s.dragging {
                    s.dragging = false;
                    up.set(s);
                }
            });

            ((), st.get())
        });
    }
}

impl IntoElement for Resizable {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// `transform` — поворот, масштаб и сдвиг при отрисовке.
///
/// Раскладка преобразование не видит: элемент занимает своё место, а рисуется
/// изменённым — так же, как в CSS. Матрица считается по границам элемента,
/// потому что точка отсчёта (`transform-origin`) задаётся долями от них.
///
/// Оговорка: области попадания курсора остаются на исходном месте — они
/// расставляются до отрисовки, когда преобразования ещё нет.
/// Что липкому элементу известно о родителе и о видимой части ленты.
///
/// Оба прямоугольника снимает распорка, которую сборщик дерева кладёт первым
/// ребёнком родителя: сам элемент к моменту своей отрисовки уже вынесен из
/// потока и обрезки ленты не видит.
#[derive(Clone, Copy, Default)]
pub struct StickyFrame {
    pub container: Option<Bounds<Pixels>>,
    pub viewport: Option<Bounds<Pixels>>,
}

pub type StickyCell = std::rc::Rc<std::cell::Cell<StickyFrame>>;

/// `position: sticky`: элемент едет с потоком, пока не упрётся в край видимой
/// части, и дальше стоит у края — но не выходит за пределы родителя.
///
/// Раньше липкий вёл себя как обычный: смещение ленты элементу было неоткуда
/// узнать. Теперь видимую часть даёт распорка родителя, а порядок отрисовки —
/// отложенный проход: иначе содержимое, идущее ниже по разметке, закрашивало
/// бы прилипший заголовок.
pub struct Sticky {
    child: Option<AnyElement>,
    /// Пороги прилипания в точках; `None` — сторона не задана.
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub frame: StickyCell,
}

impl Sticky {
    pub fn new(child: AnyElement, frame: StickyCell) -> Self {
        Sticky {
            child: Some(child),
            top: None,
            bottom: None,
            left: None,
            right: None,
            frame,
        }
    }

    /// Насколько сдвинуть элемент, чтобы он остался у края видимой части.
    fn shift(&self, bounds: Bounds<Pixels>) -> gpui::Point<Pixels> {
        let frame = self.frame.get();
        let Some(view) = frame.viewport else {
            return gpui::point(px(0.0), px(0.0));
        };
        let mut dx = px(0.0);
        let mut dy = px(0.0);
        if let Some(t) = self.top {
            let want = view.origin.y + px(t);
            if bounds.origin.y < want {
                dy = want - bounds.origin.y;
            }
        }
        if let Some(b) = self.bottom {
            let want = view.origin.y + view.size.height - px(b) - bounds.size.height;
            if bounds.origin.y > want {
                dy = want - bounds.origin.y;
            }
        }
        if let Some(l) = self.left {
            let want = view.origin.x + px(l);
            if bounds.origin.x < want {
                dx = want - bounds.origin.x;
            }
        }
        if let Some(r) = self.right {
            let want = view.origin.x + view.size.width - px(r) - bounds.size.width;
            if bounds.origin.x > want {
                dx = want - bounds.origin.x;
            }
        }
        // За пределы родителя липкий не выходит: у края родителя он снова
        // уезжает вместе с потоком.
        if let Some(c) = frame.container {
            let max_y = c.origin.y + c.size.height - bounds.size.height - bounds.origin.y;
            let min_y = c.origin.y - bounds.origin.y;
            dy = dy.clamp(min_y.min(px(0.0)), max_y.max(px(0.0)));
            let max_x = c.origin.x + c.size.width - bounds.size.width - bounds.origin.x;
            let min_x = c.origin.x - bounds.origin.x;
            dx = dx.clamp(min_x.min(px(0.0)), max_x.max(px(0.0)));
        }
        gpui::point(dx, dy)
    }
}

impl Element for Sticky {
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
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
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
        let shift = self.shift(bounds);
        let child = self.child.as_mut().unwrap();
        window.with_element_offset(shift, |window| child.prepaint(window, cx));
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
        // Отложенный проход рисует вне обрезки ленты — возвращаем её сами,
        // иначе прилипший элемент вылезал бы за края прокручиваемой области.
        let child = self.child.as_mut().unwrap();
        match self.frame.get().viewport {
            Some(view) => window
                .with_content_mask(Some(gpui::ContentMask { bounds: view }), |window| {
                    child.paint(window, cx)
                }),
            None => child.paint(window, cx),
        }
    }
}

impl IntoElement for Sticky {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Сетка таблицы, у которой ширины колонок считаются по содержимому.
///
/// Поддерево, нарисованное в отдельный буфер (`filter: blur(N)`).
///
/// Размытию нужна сложенная картинка поддерева целиком: размывать каждый
/// примитив по отдельности — не то же самое, края внутри группы обязаны
/// смешаться до размытия. Патч gpui рисует детей в свой буфер и кладёт его в
/// кадр уже размытым.
pub struct Grouped {
    child: Option<AnyElement>,
    /// Радиус размытия в точках; 0 — только сборка в буфер.
    pub blur: f32,
    /// Прозрачность группы целиком.
    pub opacity: f32,
    /// Режим смешивания с кадром (`mix-blend-mode`), 0 — обычный.
    pub blend: u32,
    /// Обрезка многоугольником: вершины в долях коробки (`clip-path`).
    pub polygon: Vec<(f32, f32)>,
}

impl Grouped {
    pub fn new(child: AnyElement) -> Self {
        Grouped {
            child: Some(child),
            blur: 0.0,
            opacity: 1.0,
            blend: 0,
            polygon: Vec::new(),
        }
    }
}

impl Element for Grouped {
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
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().prepaint(window, cx);
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
        // Размытая картинка выходит за края элемента — в браузере тоже.
        // Вчетверо шире радиуса: маска композита обязана лежать там, где
        // размытая картинка уже сошла на нет, иначе край режется прямоугольником.
        let margin = px(self.blur * 4.0);
        let area = Bounds {
            origin: gpui::point(bounds.origin.x - margin, bounds.origin.y - margin),
            size: gpui::size(
                bounds.size.width + margin * 2.0,
                bounds.size.height + margin * 2.0,
            ),
        };
        // Вершины заданы долями коробки — точки известны только здесь.
        let polygon: Vec<gpui::Point<Pixels>> = self
            .polygon
            .iter()
            .map(|(fx, fy)| {
                gpui::point(
                    bounds.origin.x + bounds.size.width * *fx,
                    bounds.origin.y + bounds.size.height * *fy,
                )
            })
            .collect();
        let child = self.child.as_mut().unwrap();
        window.paint_group(
            area,
            gpui::Corners::default(),
            self.blur,
            self.opacity,
            self.blend,
            &polygon,
            |window| child.paint(window, cx),
        );
    }
}

impl IntoElement for Grouped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub struct Transformed {
    child: Option<AnyElement>,
    /// Поворот в радианах, масштаб по осям, сдвиг в точках.
    pub rotate: f32,
    /// Скос по осям в радианах (`transform: skew`).
    pub skew: (f32, f32),
    pub scale: (f32, f32),
    pub translate: (f32, f32),
    /// Сдвиг долями СОБСТВЕННОГО размера: `translate(-50%, -50%)`.
    pub translate_pct: (f32, f32),
    /// Доли от размера элемента: 0.5, 0.5 — центр.
    pub origin: (f32, f32),
    /// Точка отсчёта В ТОЧКАХ по осям — сильнее доли, когда задана.
    pub origin_px: (Option<f32>, Option<f32>),
}

impl Transformed {
    pub fn new(child: AnyElement) -> Self {
        Transformed {
            child: Some(child),
            rotate: 0.0,
            skew: (0.0, 0.0),
            scale: (1.0, 1.0),
            translate: (0.0, 0.0),
            translate_pct: (0.0, 0.0),
            origin: (0.5, 0.5),
            origin_px: (None, None),
        }
    }
}

impl Element for Transformed {
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
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().prepaint(window, cx);
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
        let scale_factor = window.scale_factor();
        // Матрица живёт в физических точках устройства.
        let dev = |v: f32| px(v).scale(scale_factor);
        // Точка отсчёта — в устройстве, от неё и разворачиваем. Записанная
        // длиной, она сильнее доли: `transform-origin: 0 0` — левый верх, а
        // не центр (доля из длины считается только здесь, где размер известен).
        let ox = self
            .origin_px
            .0
            .unwrap_or(f32::from(bounds.size.width) * self.origin.0);
        let oy = self
            .origin_px
            .1
            .unwrap_or(f32::from(bounds.size.height) * self.origin.1);
        let origin = gpui::point(
            dev(f32::from(bounds.origin.x) + ox),
            dev(f32::from(bounds.origin.y) + oy),
        );
        let back = gpui::point(
            dev(-(f32::from(bounds.origin.x) + ox)),
            dev(-(f32::from(bounds.origin.y) + oy)),
        );
        // Порядок как в CSS: сдвиг, затем поворот, затем масштаб — всё вокруг
        // точки отсчёта, поэтому она сначала уводится в ноль и возвращается.
        let matrix = gpui::TransformationMatrix::unit()
            .translate(origin)
            .rotate(gpui::Radians(self.rotate))
            .skew(gpui::Radians(self.skew.0), gpui::Radians(self.skew.1))
            .scale(gpui::size(self.scale.0, self.scale.1))
            .translate(back)
            // Проценты сдвига считаются от собственного размера — он известен
            // только здесь, на отрисовке.
            .translate(gpui::point(
                dev(self.translate.0 + f32::from(bounds.size.width) * self.translate_pct.0),
                dev(self.translate.1 + f32::from(bounds.size.height) * self.translate_pct.1),
            ));
        let child = self.child.as_mut().unwrap();
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}

impl IntoElement for Transformed {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Подложка: поддерево рисуется ПОД всем содержимым кадра.
///
/// Отрицательный `z-index` (CSS 2.1 §9.9, шаг 3): элемент остаётся на своём
/// месте в потоке — его слот и есть его координата, — но краска ложится ниже
/// содержимого, нарисованного до него. Порядок отрисовки у нас — порядок
/// детей, и позднему ребёнку иначе никак не лечь под раннего.
pub struct Underlay {
    child: Option<AnyElement>,
}

impl Underlay {
    pub fn new(child: AnyElement) -> Self {
        Underlay { child: Some(child) }
    }
}

impl Element for Underlay {
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
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().prepaint(window, cx);
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
        let child = self.child.as_mut().unwrap();
        window.paint_bottom_layer(|window| child.paint(window, cx));
    }
}

impl IntoElement for Underlay {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Отрисовка ребёнка только в ПРЯМОУГОЛЬНИКАХ, снятых пробами прошлого кадра.
///
/// Фон ряда таблицы (css-tables-3 §drawing-backgrounds): картинка ряда
/// рисуется В ЯЧЕЙКАХ, непрерывно от начала ряда, а зазоры остаются чистыми.
/// Геометрию ячеек знает только раскладка — её снимают пробы в ячейках, а
/// ряд рисует своего ребёнка по разу на прямоугольник, обрезая маской.
/// Первый кадр пуст (пробы ещё не писали) — стенд и так ждёт устоявшийся.
/// Прямоугольник ячейки + флаг «точная»: точная лежит целиком в своём
/// ряду/колонке (span = 1), объединённая (rowspan/colspan) выходит за них.
/// Область фона считается ТОЛЬКО по точным — объединённая растягивала бы
/// градиент колонки на чужие дорожки; маски краски — по всем.
pub type RowRects = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, bool)>>>;

thread_local! {
    /// Буферы прямоугольников ПО РЯДАМ, переживающие перестройку дерева:
    /// каждый кадр стенд строит элементы заново, и Rc из прошлого кадра
    /// иначе терялся вместе с записями проб.
    static ROW_RECTS: std::cell::RefCell<std::collections::HashMap<u64, RowRects>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Буфер прямоугольников ряда по устойчивому номеру узла.
pub fn row_rects_for(node_id: u64) -> RowRects {
    ROW_RECTS.with(|m| m.borrow_mut().entry(node_id).or_default().clone())
}

/// Сброс буферов проб при смене документа.
///
/// Номера узлов считаются с нуля в каждом документе: без сброса полоса
/// нового документа забирала прямоугольники ячеек ПРЕЖНЕГО с тем же
/// номером, и первый кадр красил фон по чужим местам — а если ничего не
/// инвалидировало окно, грязный кадр оставался последним.
pub fn forget_row_rects() {
    ROW_RECTS.with(|m| m.borrow_mut().clear());
    CELL_EDGES.with(|m| m.borrow_mut().clear());
    forget_clamp_buffers();
}

pub struct CellsClipped {
    style: crate::computed::Computed,
    rects: RowRects,
}

impl CellsClipped {
    pub fn new(rects: RowRects, style: crate::computed::Computed) -> Self {
        CellsClipped { style, rects }
    }
}

impl Element for CellsClipped {
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
        let mut style = gpui::Style::default();
        // Оверлей вне потока: раскладку таблицы полоса не трогает.
        style.position = gpui::Position::Absolute;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Прошлый кадр забирается целиком: пробы ячеек напишут свежие
        // прямоугольники после нас, в этом же кадре.
        let rects = std::mem::take(&mut *self.rects.borrow_mut());
        if std::env::var("HTML_ROWBG").is_ok() {
            eprintln!("ROWBG paint: rects={} {:?}", rects.len(), rects);
        }
        if rects.is_empty() {
            // Пробы ячеек ещё не писали (первый кадр) — без нового кадра
            // окно не перерисуется, и фон не появится никогда.
            window.request_animation_frame();
            return;
        }
        // Область ряда/колонки — охват ТОЧНЫХ ячеек (span = 1): от неё
        // считается и размер плитки, и `background-position`. Объединённые
        // лежат и на чужих дорожках — они только маски.
        let exact: Vec<Bounds<Pixels>> = rects
            .iter()
            .filter(|(_, e)| *e)
            .map(|(b, _)| *b)
            .collect();
        let all: Vec<Bounds<Pixels>> = rects.iter().map(|(b, _)| *b).collect();
        let base = if exact.is_empty() { &all } else { &exact };
        let mut area = base[0];
        for r in &base[1..] {
            let right = area.origin.x + area.size.width;
            let bottom = area.origin.y + area.size.height;
            let x0 = area.origin.x.min(r.origin.x);
            let y0 = area.origin.y.min(r.origin.y);
            let x1 = right.max(r.origin.x + r.size.width);
            let y1 = bottom.max(r.origin.y + r.size.height);
            area = Bounds {
                origin: gpui::point(x0, y0),
                size: gpui::size(x1 - x0, y1 - y0),
            };
        }
        // Тень РЯДА — вокруг охвата всех его ячеек, без маски: она лежит
        // снаружи. Резкая (без размытия) рисуется кольцевым квадом — тот же
        // обход вырождения шейдера, что у обычных коробок.
        for sh in &self.style.shadows {
            let colour = if sh.color.a < 0.0 {
                self.style.color.unwrap_or(crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
            } else {
                sh.color
            };
            let shifted = Bounds {
                origin: gpui::point(area.origin.x + gpui::px(sh.x), area.origin.y + gpui::px(sh.y)),
                size: area.size,
            };
            if sh.blur > 0.0 {
                window.paint_shadows(
                    shifted,
                    gpui::Corners::default(),
                    &[gpui::BoxShadow {
                        color: colour.to_hsla(),
                        offset: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                        blur_radius: gpui::px(sh.blur),
                        spread_radius: gpui::px(sh.spread),
                    }],
                );
            } else {
                let grown = Bounds {
                    origin: gpui::point(
                        shifted.origin.x - gpui::px(sh.spread),
                        shifted.origin.y - gpui::px(sh.spread),
                    ),
                    size: gpui::size(
                        shifted.size.width + gpui::px(sh.spread * 2.0),
                        shifted.size.height + gpui::px(sh.spread * 2.0),
                    ),
                };
                let mut quad = gpui::fill(grown, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px((sh.spread - sh.y).max(0.0)),
                    right: gpui::px((sh.spread + sh.x).max(0.0)),
                    bottom: gpui::px((sh.spread + sh.y).max(0.0)),
                    left: gpui::px((sh.spread - sh.x).max(0.0)),
                };
                window.paint_quad(quad);
            }
        }
        for rect in all {
            window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |window| {
                // Цвет ряда — под картинкой, в тех же прямоугольниках: на
                // ячейки его в этом случае не переносят (иначе он закрашивал
                // бы картинку, рисуясь позже полосы).
                if let Some(bg) = self.style.background {
                    window.paint_quad(gpui::fill(rect, bg.to_hsla()));
                }
                crate::background::paint_area(&self.style, area, window);
            });
        }
    }
}

impl IntoElement for CellsClipped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Сросшиеся кромки таблицы: ячейка без собственных рамок отдаёт их
/// отдельному слою — кромки рисуются НА ЛИНИЯХ сетки поверх фонов
/// (css-tables-3 §drawing-borders), а конфликт «шире побеждает»
/// (CSS 2.1 §17.6.2.1) решается порядком: узкие раньше, широкие поверх.
pub struct EdgeCell {
    pub bounds: Bounds<Pixels>,
    /// Ширины кромок [верх, право, низ, лево] в точках.
    pub widths: [f32; 4],
    pub colors: [crate::value::Color; 4],
    /// Ранги стилей сторон (см. `Computed::border_side_styles`); 9 = solid.
    pub styles: [u8; 4],
    /// Ранг источника: ячейка 2, таблица 0 (CSS 2.1 §17.6.2.1 п.4).
    pub source: u8,
    /// Порядок в документе: раньше = выше/левее, при равенстве побеждает.
    pub doc_ix: u32,
}

pub type CellEdges = std::rc::Rc<std::cell::RefCell<Vec<EdgeCell>>>;

thread_local! {
    static CELL_EDGES: std::cell::RefCell<std::collections::HashMap<u64, CellEdges>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn cell_edges_for(key: u64) -> CellEdges {
    CELL_EDGES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

/// Проба кромок: как проба фона, пишет в PREPAINT границы и рамки ячейки.
pub fn edge_probe(
    edges: CellEdges,
    widths: [f32; 4],
    colors: [crate::value::Color; 4],
    styles: [u8; 4],
    source: u8,
    doc_ix: u32,
    inset: [f32; 4],
) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            // Вжим границ внутрь: линии рамки самой таблицы лежат на
            // ВНУТРЕННИХ краях её рамочного места.
            let bounds = Bounds {
                origin: gpui::point(
                    bounds.origin.x + gpui::px(inset[3]),
                    bounds.origin.y + gpui::px(inset[0]),
                ),
                size: gpui::size(
                    bounds.size.width - gpui::px(inset[1] + inset[3]),
                    bounds.size.height - gpui::px(inset[0] + inset[2]),
                ),
            };
            edges
                .borrow_mut()
                .push(EdgeCell { bounds, widths, colors, styles, source, doc_ix });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Слой сросшихся кромок: рисует рамки всех ячеек таблицы, центрируя
/// каждую на границе ячейки. Совпадающие кромки соседей ложатся друг на
/// друга; побеждает нарисованная позже — порядок по ширине даёт правило
/// «шире побеждает».
pub struct EdgePainter {
    edges: CellEdges,
}

impl EdgePainter {
    pub fn new(edges: CellEdges) -> Self {
        EdgePainter { edges }
    }
}

impl Element for EdgePainter {
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
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let cells = std::mem::take(&mut *self.edges.borrow_mut());
        // Кандидат кромки на ЛИНИИ сетки: совпадающие отрезки соседей — ОДНА
        // кромка, победитель по CSS 2.1 §17.6.2.1 (hidden гасит всех, затем
        // шире, ранг стиля, источник ячейка>таблица, порядок в документе).
        struct Cand {
            line: f32,
            a: f32,
            b: f32,
            w: f32,
            style: u8,
            source: u8,
            doc_ix: u32,
            colour: crate::value::Color,
            /// Наружная сторона крайней линии таблицы (-1/1); 0 — центр.
            outward: i8,
        }
        let mut vert: Vec<Cand> = vec![];
        let mut horiz: Vec<Cand> = vec![];
        for c in &cells {
            let bnd = c.bounds;
            let (x0, y0) = (f32::from(bnd.origin.x), f32::from(bnd.origin.y));
            let (x1, y1) = (x0 + f32::from(bnd.size.width), y0 + f32::from(bnd.size.height));
            let is_table = c.source == 0;
            let mut side =
                |list: &mut Vec<Cand>, line: f32, a: f32, b: f32, i: usize, out: i8| {
                    if c.widths[i] > 0.0 || c.styles[i] == 1 {
                        list.push(Cand {
                            line,
                            a,
                            b,
                            w: c.widths[i],
                            style: c.styles[i],
                            source: c.source,
                            doc_ix: c.doc_ix,
                            colour: c.colors[i],
                            outward: if is_table { out } else { 0 },
                        });
                    }
                };
            side(&mut horiz, y0, x0, x1, 0, -1);
            side(&mut vert, x1, y0, y1, 1, 1);
            side(&mut horiz, y1, x0, x1, 2, 1);
            side(&mut vert, x0, y0, y1, 3, -1);
        }
        let mut draw = |cands: &mut Vec<Cand>, vertical: bool| {
            cands.sort_by(|p, q| p.line.partial_cmp(&q.line).unwrap_or(std::cmp::Ordering::Equal));
            // Крайние линии таблицы: кромка не центрируется, а рисуется
            // внутрь бокса (наружная половина у браузеров уходит в поля,
            // эталоны считают рамку частью коробки).
            let lo_line = cands.first().map(|c| c.line).unwrap_or(0.0);
            let hi_line = cands.last().map(|c| c.line).unwrap_or(0.0);
            let mut i = 0;
            while i < cands.len() {
                let mut j = i + 1;
                while j < cands.len() && (cands[j].line - cands[i].line).abs() < 0.75 {
                    j += 1;
                }
                let group = &cands[i..j];
                let outward = group.iter().find_map(|c| (c.outward != 0).then_some(c.outward));
                let mut cuts: Vec<f32> = group.iter().flat_map(|c| [c.a, c.b]).collect();
                cuts.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
                cuts.dedup_by(|p, q| (*p - *q).abs() < 0.5);
                for seg in cuts.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    if b - a < 0.5 {
                        continue;
                    }
                    let mid = (a + b) / 2.0;
                    let covering: Vec<&Cand> = group
                        .iter()
                        .filter(|c| c.a - 0.25 <= mid && mid <= c.b + 0.25)
                        .collect();
                    if covering.is_empty() || covering.iter().any(|c| c.style == 1) {
                        continue;
                    }
                    let win = covering
                        .iter()
                        .max_by(|p, q| {
                            let kp = (p.w, p.style, p.source, u32::MAX - p.doc_ix);
                            let kq = (q.w, q.style, q.source, u32::MAX - q.doc_ix);
                            kp.partial_cmp(&kq).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap();
                    if win.w <= 0.0 || win.colour.a == 0.0 {
                        continue;
                    }
                    let line = win.line;
                    let edge_dir = match outward {
                        Some(d) => Some(d),
                        None if (line - lo_line).abs() < 0.75 => Some(1),
                        None if (line - hi_line).abs() < 0.75 => Some(-1),
                        None => None,
                    };
                    let (lo, hi) = match edge_dir {
                        Some(-1) => (line - win.w, line),
                        Some(_) => (line, line + win.w),
                        None => (line - win.w / 2.0, line + win.w / 2.0),
                    };
                    // Продление В УГЛЫ только у горизонталей: пересечение
                    // иначе оставалось пустым квадратом, а продление обеих
                    // осей рисовало лишние усы на пунктирных рамках.
                    let (a, b) = if !vertical && win.style >= 9 {
                        (a - win.w / 2.0, b + win.w / 2.0)
                    } else {
                        (a, b)
                    };
                    let rect = if vertical {
                        Bounds {
                            origin: gpui::point(gpui::px(lo), gpui::px(a)),
                            size: gpui::size(gpui::px(hi - lo), gpui::px(b - a)),
                        }
                    } else {
                        Bounds {
                            origin: gpui::point(gpui::px(a), gpui::px(lo)),
                            size: gpui::size(gpui::px(b - a), gpui::px(hi - lo)),
                        }
                    };
                    window.paint_quad(gpui::fill(rect, win.colour.to_hsla()));
                }
                i = j;
            }
        };
        draw(&mut vert, true);
        draw(&mut horiz, false);
    }
}

impl IntoElement for EdgePainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Бюджет строк обрезки (`line-clamp`, css-overflow-3/4): точка среза —
/// низ N-й СЧИТАЕМОЙ строки. Строки потомков в собственном контексте
/// форматирования (BFC: overflow, флоат, корень потока) видимы, но НЕ
/// считаются; блок, пересекающий точку среза, прячется целиком — срез
/// поднимается к его верху. Точка меряется пробами построенного кадра и
/// применяется потолком высоты на СЛЕДУЮЩЕМ (перестройка каждый кадр).
pub struct ClampEntry {
    pub bounds: Bounds<Pixels>,
    /// Высота строки в точках; 0 — блок без собственного текста.
    pub line: f32,
    /// Строки не считаются (элемент внутри вложенного BFC).
    pub skip_count: bool,
}

pub type ClampLines = std::rc::Rc<std::cell::RefCell<Vec<ClampEntry>>>;

thread_local! {
    static CLAMP_LINES: std::cell::RefCell<std::collections::HashMap<u64, ClampLines>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Вычисленные точки среза (высота от верха контейнера) прошлого кадра.
    static CLAMP_CUTS: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Стек активных clamp-контейнеров при ПОСТРОЕНИИ дерева:
    /// (ключ, граница BFC уже пройдена).
    static CLAMP_STACK: std::cell::RefCell<Vec<(u64, bool)>> =
        std::cell::RefCell::new(Vec::new());
}

pub fn clamp_lines_for(key: u64) -> ClampLines {
    CLAMP_LINES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

pub fn clamp_cut(key: u64) -> Option<f32> {
    CLAMP_CUTS.with(|m| m.borrow().get(&key).copied())
}

pub fn forget_clamp_buffers() {
    CLAMP_LINES.with(|m| m.borrow_mut().clear());
    CLAMP_CUTS.with(|m| m.borrow_mut().clear());
    CLAMP_STACK.with(|st| st.borrow_mut().clear());
}

/// Сторож стека clamp-контекста на время построения поддерева.
pub struct ClampGuard(bool);

impl ClampGuard {
    /// Вход в сам clamp-контейнер.
    pub fn enter(key: u64) -> Self {
        CLAMP_STACK.with(|st| st.borrow_mut().push((key, false)));
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

/// Проба строк: канвас в элементе с текстом (или блоке), пишет границы и
/// высоту строки в prepaint своего кадра.
pub fn clamp_probe(lines: ClampLines, line: f32, skip_count: bool) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry { bounds, line, skip_count });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Вычислитель точки среза: абсолютный элемент В КОНЦЕ clamp-контейнера,
/// его границы — весь контейнер. Считает низ N-й считаемой строки,
/// поднимает срез к верху пересечённого блока и просит новый кадр, когда
/// точка изменилась.
pub struct ClampCut {
    key: u64,
    lines: ClampLines,
    /// Число считаемых строк; None — `line-clamp: auto` (срез только по
    /// потолку высоты, но пересечённый блок всё равно прячется целиком).
    limit: Option<u32>,
    /// Потолок высоты контейнера в точках (max-height), если задан.
    max_h: Option<f32>,
}

impl ClampCut {
    pub fn new(key: u64, lines: ClampLines, limit: Option<u32>, max_h: Option<f32>) -> Self {
        ClampCut { key, lines, limit, max_h }
    }
}

impl Element for ClampCut {
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
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let entries = std::mem::take(&mut *self.lines.borrow_mut());
        let top = f32::from(bounds.origin.y);
        // Строки: у текстового вклада их bounds.height / line штук.
        let mut rows: Vec<(f32, f32, bool)> = vec![]; // (верх, низ, считается)
        let mut blocks: Vec<(f32, f32)> = vec![];
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                let n = (h / e.line).round().max(1.0) as usize;
                let step = h / n as f32;
                for i in 0..n {
                    rows.push((y0 + i as f32 * step, y0 + (i + 1) as f32 * step, !e.skip_count));
                }
            } else if h > 0.0 {
                blocks.push((y0, y0 + h));
            }
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cut: Option<f32> = self.max_h.map(|m| top + m);
        if let Some(limit) = self.limit {
            let mut seen = 0u32;
            for (_, bottom, countable) in &rows {
                if *countable {
                    seen += 1;
                    if seen == limit {
                        let candidate = *bottom;
                        cut = Some(cut.map_or(candidate, |c| c.min(candidate)));
                        break;
                    }
                }
            }
            if seen < limit && self.max_h.is_none() {
                // Строк меньше предела — среза нет.
                cut = None;
            }
        }
        // Блок, пересекающий срез, прячется целиком: срез к его верху.
        if let Some(c) = cut {
            let mut c2 = c;
            for (y0, y1) in &blocks {
                if *y0 < c2 && c2 < *y1 {
                    c2 = *y0;
                }
            }
            // И строка, пересечённая потолком, тоже не показывается
            // половинкой: срез к её верху.
            for (y0, y1, _) in &rows {
                if *y0 < c2 && c2 < *y1 - 0.5 {
                    c2 = *y0;
                }
            }
            cut = Some(c2);
        }
        let rel = cut.map(|c| (c - top).max(0.0));
        let prev = clamp_cut(self.key);
        let changed = match (prev, rel) {
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            (None, None) => false,
            _ => true,
        };
        if changed {
            CLAMP_CUTS.with(|m| {
                let mut m = m.borrow_mut();
                match rel {
                    Some(v) => {
                        m.insert(self.key, v);
                    }
                    None => {
                        m.remove(&self.key);
                    }
                }
            });
            window.request_animation_frame();
        }
    }
}

impl IntoElement for ClampCut {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Проба ячейки: канвас, записывающий свои границы для фона ряда.
pub fn cell_rect_probe(rects: RowRects, exact: bool, shift: (f32, f32)) -> AnyElement {
    gpui::canvas(
        // Запись В PREPAINT: подготовка ВСЕХ элементов идёт до отрисовки,
        // и полоса фона читает прямоугольники СВОЕГО кадра — с записью в
        // paint она рисовала прошлый кадр и мигала на каждой смене раскладки.
        move |bounds: Bounds<Pixels>, _, _| {
            // Сдвиг краски относительно коробки ячейки: в сросшейся модели
            // фоновая сетка начинается от середины рамки таблицы.
            let bounds = Bounds {
                origin: gpui::point(
                    bounds.origin.x + gpui::px(shift.0),
                    bounds.origin.y + gpui::px(shift.1),
                ),
                size: bounds.size,
            };
            rects.borrow_mut().push((bounds, exact));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Строка вертикального письма: `writing-mode: vertical-rl` и `vertical-lr`.
///
/// Поворота мало: у повёрнутого текста меняются местами ширина и высота, и
/// раскладка обязана считать их поменянными — иначе строка занимает место как
/// горизонтальная и наезжает на соседей. Поэтому это свой элемент: он
/// измеряет текст сам, отдаёт раскладке перевёрнутый размер, а на отрисовке
/// разворачивает содержимое на четверть оборота по часовой стрелке.
pub struct VerticalText {
    child: Option<AnyElement>,
    /// Естественный размер содержимого до поворота.
    natural: gpui::Size<Pixels>,
}

impl VerticalText {
    pub fn new(child: AnyElement) -> Self {
        VerticalText {
            child: Some(child),
            natural: gpui::Size::default(),
        }
    }
}

impl Element for VerticalText {
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
        // Собственный корень раскладки: размер текста нужен ЗДЕСЬ, чтобы
        // отдать его родителю перевёрнутым.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: замер по запросу родителя
        // (`request_measured_layout`), чтобы ограничение доходило до
        // содержимого и вертикальный текст переносился. Не работает: на этом
        // шаге раскладочный движок недоступен — повёрнутый блок сам живёт
        // внутри чужого замера, и вызов падает на `layout_engine.unwrap()`
        // (vendor/gpui/src/window.rs). Ограничение придётся доводить другим
        // путём — например, осью потока в самом стиле.
        let space = gpui::size(
            gpui::AvailableSpace::MaxContent,
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = self
            .child
            .as_mut()
            .unwrap()
            .layout_as_root(space, window, cx);
        let mut style = gpui::Style::default();
        // Ширина заявляется, высота — НЕТ. Ширина повёрнутого блока это число
        // строк, его меньше не сделать. А высота — длина строки, и её решает
        // родитель: заявленная здесь, она делала коробку сколь угодно длинной,
        // ограничение до текста не доходило, и он не переносился никогда.
        style.size.width = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(self.natural.height),
        ));
        (window.request_layout(style, [], cx), ())
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
        // Ограничение доводится ЗДЕСЬ: на замере размеры коробки ещё
        // неизвестны, а на подготовке они уже решены раскладкой. Оси при этом
        // переставлены: то, что для родителя высота, для повёрнутого
        // содержимого — длина строки. Без этого шага вертикальный текст
        // мерился «по максимуму содержимого» и не переносился никогда.
        // ПРОБОВАЛИ И ОТКАТИЛИ: ограничивать длину строки высотой области
        // просмотра, когда родитель своей не задал (так велит CSS для
        // ортогональных потоков). Замерено: writing-modes 195 → 194,
        // `available-size-020/021` не чинятся, а `slr-alongside-vlr-floats`
        // ломается. Значит предел приходит откуда-то ещё.
        let along = bounds.size.height;
        let child = self.child.as_mut().unwrap();
        if along > gpui::px(0.) {
            let space = gpui::size(
                gpui::AvailableSpace::Definite(along),
                gpui::AvailableSpace::Definite(bounds.size.width),
            );
            child.layout_as_root(space, window, cx);
        }
        child.prepaint_at(bounds.origin, window, cx);
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
        let scale_factor = window.scale_factor();
        let dev = |v: Pixels| v.scale(scale_factor);
        // Поворот на четверть по часовой стрелке вокруг левого верхнего угла
        // уводит содержимое влево от коробки; сдвиг на её ширину возвращает
        // его на место.
        let matrix = gpui::TransformationMatrix::unit()
            .translate(gpui::point(
                dev(bounds.origin.x + bounds.size.width),
                dev(bounds.origin.y),
            ))
            .rotate(gpui::Radians(std::f32::consts::FRAC_PI_2))
            .translate(gpui::point(dev(-bounds.origin.x), dev(-bounds.origin.y)));
        let child = self.child.as_mut().unwrap();
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}

impl IntoElement for VerticalText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Прокрутка содержимого — `overflow: auto` и `scroll`.
///
/// Раньше оба значения давали только обрезку: часть содержимого пропадала без
/// возможности до неё добраться. Прокрутка в GPUI требует РУЧКИ, живущей между
/// кадрами, — а память есть только у своего элемента. Сама лента и колесо мыши
/// уже реализованы в `div`, поэтому здесь только ручка и её хранение.
pub struct ScrollArea {
    id: ElementId,
    horizontal: bool,
    vertical: bool,
    build: Rc<dyn Fn(&gpui::ScrollHandle, bool, bool) -> AnyElement>,
}

impl ScrollArea {
    pub fn new(
        id: ElementId,
        horizontal: bool,
        vertical: bool,
        build: Rc<dyn Fn(&gpui::ScrollHandle, bool, bool) -> AnyElement>,
    ) -> Self {
        ScrollArea {
            id,
            horizontal,
            vertical,
            build,
        }
    }
}

impl Element for ScrollArea {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let build = self.build.clone();
        let (h, v) = (self.horizontal, self.vertical);
        let Some(global_id) = id else {
            let mut el = build(&gpui::ScrollHandle::default(), h, v);
            let layout_id = el.request_layout(window, cx);
            return (layout_id, el);
        };
        window.with_element_state::<gpui::ScrollHandle, _>(global_id, |handle, window| {
            let handle = handle.unwrap_or_default();
            let mut el = build(&handle, h, v);
            let layout_id = el.request_layout(window, cx);
            ((layout_id, el), handle)
        })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl IntoElement for ScrollArea {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Где на экране оказалась распорка — и где оказался её заместитель.
///
/// Позиционированный элемент по CSS рисуется ПОВЕРХ обычного содержимого, а
/// порядок отрисовки у нас — порядок детей. Отложенная отрисовка для этого не
/// годится (вложенная в GPUI запрещена, а без вложенности рушится раскладка),
/// поэтому элемент уходит последним ребёнком и возвращается на место сдвигом:
/// щуп запоминает, где стояла распорка, заместитель — где встал сам, разница
/// и есть нужный сдвиг.
#[derive(Clone, Copy, Default)]
pub struct Spot {
    pub hole: Option<Bounds<Pixels>>,
    /// Высота строки, если элемент блочный: его статическая позиция — начало
    /// СЛЕДУЮЩЕЙ строки, а не точка в текущей (CSS 2.1 §10.6.4).
    pub next_line: Option<f32>,
    /// Письмо справа налево: начало строки у такого блока — правый край.
    pub rtl: bool,
    /// Вертикальное письмо: строки идут поперёк, поэтому «следующая строка» —
    /// сдвиг по горизонтали, а не по вертикали.
    pub vertical: bool,
    /// Вертикальное письмо справа налево: следующая строка левее текущей.
    pub vertical_rl: bool,
}

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

thread_local! {
    /// Слои верхней отрисовки: по одному на каждый блок-контейнер в работе.
    /// Позиционированный элемент кладёт себя в верхний слой, а контейнер
    /// забирает слой целиком и дописывает его последними детьми.
    static LATE: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Открыть слой на время сборки детей контейнера.
pub fn late_open() {
    LATE.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем и закрыть его.
pub fn late_close() -> Vec<AnyElement> {
    LATE.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| spot_place(spot, el))
        .collect()
}

/// Отдать содержимое верхнему слою. Если слоя нет (элемент собирают вне
/// блока-контейнера), содержимое возвращается — рисовать его на месте.
pub fn late_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    LATE.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

/// Нулевая распорка на месте элемента: занимает его место в потоке и
/// запоминает, где это место оказалось. `full` — распорка в потоке блоков (во
/// всю ширину), иначе — внутри строки.
pub fn spot_probe(spot: SpotCell, full: bool) -> AnyElement {
    let mut probe = gpui::div().h_0().flex_shrink_0();
    if full {
        probe = probe.w_full();
    } else {
        // Внутри строки распорка прижимается к её ВЕРХУ: на базовой линии
        // содержимое уезжало бы под строку.
        probe = probe.w_0();
        probe.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    probe
        .child(gpui::canvas(
            move |bounds, _, _| {
                let mut now = spot.get();
                now.hole = Some(bounds);
                spot.set(now);
            },
            |_, _, _, _| {},
        ))
        .into_any_element()
}

/// Заместитель в конце списка: рисуется последним, но встаёт туда, где стояла
/// распорка. Сдвиг считается прямо в подготовке кадра — распорка идёт раньше
/// по списку детей, поэтому её место к этому моменту уже известно.
pub fn spot_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    // Ряд, а не столбец: у абсолютного элемента ширина «по содержимому»
    // (CSS 2.1 §10.3.7), а ребёнок столбца растягивался бы во всю ширину
    // родителя — при письме справа налево содержимое такой коробки уезжало за
    // её край на всю эту ширину.
    let now = spot.get();
    // При вертикальном письме поток строк идёт поперёк: распорка тянется по
    // высоте, а не по ширине, иначе она уводила бы содержимое вниз.
    let mut wrap = if now.vertical {
        gpui::div().h_full().w_0().flex_shrink_0().flex().flex_col()
    } else {
        gpui::div().w_full().h_0().flex_shrink_0().flex().flex_row()
    };
    wrap = wrap.items_start();
    wrap = if now.rtl {
        wrap.justify_end()
    } else {
        wrap.justify_start()
    };
    wrap.child(LatePlace {
            child: Some(child),
            spot,
        })
        .into_any_element()
}

pub struct LatePlace {
    child: Option<AnyElement>,
    spot: SpotCell,
}

impl Element for LatePlace {
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
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
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
        let now = self.spot.get();
        let shift = match (now.hole, now.next_line) {
            // Блочный: слева — край содержимого родителя (там же, где стоит сам
            // заместитель), сверху — низ строки, в которой он оказался.
            // Поперёк потока строк заместитель уже стоит у нужного края (его
            // ставит распорка ряда), поэтому правится только та ось, вдоль
            // которой идут строки.
            (Some(hole), Some(line)) if now.vertical && now.vertical_rl => {
                gpui::point(
                    hole.origin.x - px(line) - bounds.origin.x,
                    px(0.0),
                )
            }
            (Some(hole), Some(line)) if now.vertical => {
                gpui::point(hole.origin.x + px(line) - bounds.origin.x, px(0.0))
            }
            (Some(hole), Some(line)) => {
                gpui::point(px(0.0), hole.origin.y + px(line) - bounds.origin.y)
            }
            (Some(hole), None) => hole.origin - bounds.origin,
            (None, _) => gpui::point(px(0.0), px(0.0)),
        };
        let child = self.child.as_mut().unwrap();
        window.with_element_offset(shift, |window| child.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for LatePlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}


