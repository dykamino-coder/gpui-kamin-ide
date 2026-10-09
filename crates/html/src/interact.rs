//! Изменение размера мышью — `resize`.
//!
//! Как и переход, требует памяти между кадрами: размер, заданный пользователем,
//! должен пережить перерисовку. Память элемента — единственное такое место в
//! GPUI, поэтому это свой `Element`, а не стиль.
//!
//! Ручка рисуется в углу самим элементом; тянуть её можно по той оси, которую
//! разрешил CSS.

pub(crate) mod physical_atomic_frame;
mod vertical_line_baseline;
use gpui::{
    AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Styled, Window, px,
};
use std::rc::Rc;

mod spot_geometry;
mod rectangular_clip;
mod mask_geometry;
mod legacy_clip;
mod mask_size;
mod polygon_clip;
mod orthogonal_measure;
mod vertical_style;
mod combined_geometry;
mod gap_segments;
mod gap_fragment_tail;
mod transform_geometry;
use gap_segments::segments;
use transform_geometry::quarter_turn;

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
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
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
    /// Чистая изоляция (контекст наложения со смешиванием внутри,
    /// `isolation: isolate`): буфер кладётся в кадр ЦЕЛИКОМ, коробка его не
    /// режет — вылезшие за неё потомки остаются видимыми.
    pub spill: bool,
    /// Обрезка многоугольником: вершины в долях коробки (`clip-path`).
    pub polygon: Vec<(crate::value::Len, crate::value::Len)>,
    /// Правило намотки полигона: `evenodd` шейдер не умеет.
    pub polygon_evenodd: bool,
    /// Сдвиг опорной коробки формы от bounds наружу: верх/право/низ/лево
    /// (margin-box положительные, content-box отрицательные).
    pub poly_expand: [f32; 4],
    /// Маска-изображение (`mask-image`): источник строкой — путь растра или
    /// запись градиента. Резолвится при отрисовке: рисунку и градиенту нужен
    /// размер коробки (mask-size auto без своего размера = область,
    /// css-masking §7.4), а он известен только здесь.
    pub mask: Option<String>,
    /// `mask-size`: размер плитки; None — auto (интринзик картинки).
    pub mask_size: Option<(crate::value::Len, crate::value::Len)>,
    /// `mask-size: contain|cover` (1|2) — вписывание по интринзику.
    pub mask_fit: u8,
    /// `mask-repeat`: пооосный запрет мощения (no-x, no-y) — первого слоя.
    pub mask_no_repeat: (bool, bool),
    /// `mask-repeat` ПО СЛОЯМ (css-masking-1 §7.6); пусто — берётся скаляр.
    pub mask_repeat_list: Vec<(bool, bool)>,
    /// Per-layer `space`/`round` axes (2/3); empty — none.
    pub mask_repeat_modes: Vec<(u8, u8)>,
    /// `mask-mode: luminance` — гасит светимостью, а не альфой.
    pub mask_luminance: bool,
    /// `mask-mode: alpha`: ссылка на `<mask>` маскирует альфой.
    pub mask_alpha_mode: bool,
    /// `mask-position`: смещение плитки; доля — от свободного места.
    pub mask_pos: Option<(crate::value::Len, crate::value::Len)>,
    /// Смещение от правого/нижнего края (`right 30px bottom 25px`).
    pub mask_pos_far: (bool, bool),
    /// `mask-position` ПО СЛОЯМ (css-masking-1 §7.7): `(x, y, справа, снизу)`;
    /// пусто — берётся скаляр.
    pub mask_pos_list: Vec<(crate::value::Len, crate::value::Len, bool, bool)>,
    /// Края коробки укладки (`mask-origin`) от border-box внутрь: t/r/b/l.
    pub mask_origin_off: [f32; 4],
    /// Края коробки окраски (`mask-clip`); None — border-box/no-clip.
    pub mask_clip_off: Option<[f32; 4]>,
    /// `clip: rect(t r b l)`: координаты видимой области от углов коробки.
    pub clip_rect: Option<[Option<f32>; 4]>,
    /// Сдвиг коробки клипа трансформом элемента: px и доли своего размера.
    pub clip_shift: (f32, f32, f32, f32),
    /// `clip-path: inset(t r b l)`: срезы краёв; доли — от своих сторон.
    pub clip_inset: Option<[crate::value::Len; 4]>,
    pub clip_edges: Option<[Option<crate::value::Len>; 4]>,
    pub clip_xywh: Option<[crate::value::Len; 4]>,
    /// `round <radius>` of `inset()`/`rect()`/`xywh()` in points: the group
    /// composites through a rounded rectangle equal to the clip rectangle.
    pub clip_round: Option<crate::value::Len>,
    /// `mask-composite` по слоям: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub mask_composite: Vec<u8>,
    /// Подложка ПОД буфером группы, вне его маски: наружные тени
    /// `box-shadow` коробки с `border-shape` — они лежат снаружи фигуры, а
    /// маска группы (`bordershape:`) режет всё содержимое буфера фигурой.
    /// Колбэк строит SVG-разметку по размеру коробки (bw, bh), выносу
    /// (l, t) и холсту (aw, ah) — размеры известны только на отрисовке.
    pub under: Option<Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<String>>>,
    /// Накладка НАД буфером группы, вне его маски: кольцо рамки
    /// `border-shape` у коробки с обрезкой переполнения — содержимое режется
    /// ВНУТРЕННИМ контуром (css-borders-4 §border-shape-overflow-interaction),
    /// а рамка лежит снаружи него и поверх обрезанных детей. Колбэк — как у
    /// `under`.
    pub over: Vec<Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<String>>>,
    /// Множитель интринзика плитки маски: у SVG-ребёнка маска живёт в ЕГО
    /// пользовательских единицах (css-masking-1 §7.4 `auto` — размер
    /// картинки в системе координат элемента), и при `viewBox` 50×50
    /// рисунок-маска кроет 100×100 CSS-точек (mask-origin-3, mask-clip-2).
    /// Задаёт `svg::masked_layers` через `Computed::mask_user_scale`.
    pub mask_scale: f32,
}

impl Grouped {
    pub fn new(child: AnyElement) -> Self {
        Grouped {
            under: None,
            over: Vec::new(),
            mask_scale: 1.0,
            child: Some(child),
            blur: 0.0,
            opacity: 1.0,
            blend: 0,
            spill: false,
            polygon: Vec::new(),
            polygon_evenodd: false,
            poly_expand: [0.0; 4],
            mask: None,
            mask_size: None,
            mask_fit: 0,
            mask_no_repeat: (false, false),
            mask_repeat_list: Vec::new(),
            mask_repeat_modes: Vec::new(),
            clip_round: None,
            mask_luminance: false,
            mask_alpha_mode: false,
            mask_pos_far: (false, false),
            mask_pos_list: Vec::new(),
            mask_origin_off: [0.0; 4],
            mask_clip_off: None,
            clip_rect: None,
            clip_shift: (0.0, 0.0, 0.0, 0.0),
            clip_inset: None,
            clip_edges: None,
            clip_xywh: None,
            mask_composite: Vec::new(),
            mask_pos: None,
        }
    }
}

/// Голый источник слоя маски: содержимое `url(...)` либо запись градиента.
fn mask_layer_source(layer: &str) -> Option<String> {
    let t = layer.trim();
    if t.contains("-gradient(") {
        return Some(t.to_string());
    }
    if t.starts_with("clipsnap:") || t.starts_with("pathdef:") || t.starts_with("shapedef:") {
        return Some(t.to_string());
    }
    let at = t.find("url(")?;
    let rest = &t[at + 4..];
    let end = rest.find(')')?;
    Some(
        rest[..end]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_string(),
    )
}

/// Содержимое `<mask id>`/`<clipPath id>` из ВНЕШНЕГО файла рисунка
/// (`mask-image: url(file.svg#id)`): грубый текстовый вырез — дерево
/// документа рисунка нам нигде больше не нужно.
fn svg_fragment(path: &str, id: &str) -> Option<String> {
    let markup = std::fs::read_to_string(path).ok()?;
    for tag in ["mask", "clipPath"] {
        let mut rest = markup.as_str();
        while let Some(at) = rest.find(&format!("<{tag}")) {
            let head_end = rest[at..].find('>')? + at;
            let head = &rest[at..head_end];
            let close = format!("</{tag}>");
            let body_end = rest[head_end..].find(&close)? + head_end;
            if head.contains(&format!("id=\"{id}\"")) || head.contains(&format!("id='{id}'")) {
                return Some(rest[head_end + 1..body_end].to_string());
            }
            rest = &rest[body_end + close.len()..];
        }
    }
    None
}

/// Растр определения `<mask>`/`<clipPath>` из документа под коробку.
///
/// Содержимое сериализовано при сборе (`render::mask_def`); маска берёт
/// светимость своих красок, обрезка — покрытие (заливка принудительно
/// белая), поэтому обе идут люминанс-растром.
fn rasterize_mask_def(
    key: &str,
    w: f32,
    h: f32,
    force_white: bool,
) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let markup = crate::render::mask_snapshot(key)?;
    // Внутри <clipPath> правило намотки несёт `clip-rule`; растеризатор
    // рисует контур как обычный и читает только `fill-rule`
    // (clip-path-shape-002: у эталона пропадала дырка evenodd).
    let markup = markup.replace("clip-rule", "fill-rule");
    let body = if force_white {
        format!(r##"<g fill="#ffffff">{markup}</g>"##)
    } else {
        markup
    };
    crate::svg::rasterize(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">{body}</svg>"#
        ),
        w,
        h,
    )
}

impl Element for Grouped {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (Bounds<Pixels>, Bounds<Pixels>);

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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // The clip reference box is read BEFORE the child's prepaint: a pure
        // translation of the element is placed into its layout origin there
        // (`Transformed::prepaint`), while `clip_shift` already moves the clip
        // with the transform (CSS Masking §5: clip lives in the element's
        // pre-transform space). Read afterwards, the shift applied twice
        // (clip-transform-order: the clip landed 110px right of the box).
        let clip_bounds = rectangular_clip::reference_box(self, bounds, *_state, window);
        self.child.as_mut().unwrap().prepaint(window, cx);
        // Mask positioning box before device snapping (css-masking-1 §7.7).
        let mask_box = mask_geometry::positioning_box(self.mask.as_deref(), bounds, *_state, window);
        (clip_bounds, mask_box)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Слой маски-картинки, которая НЕ загрузилась (файла нет, формат не
        // читается), — «image layer of transparent black» (css-masking-1
        // §7.1). Все слои такие — элемент скрыт целиком (`mask-image-4a`:
        // `url(non-existent.png)` рисовался без маски). Ссылки на
        // определения, градиенты и формы сюда не входят.
        if let Some(src) = self.mask.as_deref()
            && src.contains("url(")
        {
            let layers: Vec<String> = crate::css::split_args(src)
                .iter()
                .filter_map(|l| mask_layer_source(l))
                .collect();
            let plain = |l: &str| {
                !l.contains("-gradient(")
                    && !l.contains("snap:")
                    && !l.contains("def:")
                    && !l.contains('#')
                    && !l.starts_with("data:")
                    // Только ЛОКАЛЬНЫЙ файл без схемы и запроса: `invalid://`
                    // у SVG-элемента одиночным слоем игнорируется
                    // (`bad-mask-image-svg-2/3`), а серверный путь с
                    // `?pipe=` стенду недоступен вовсе
                    // (`mask-image-svg-loading-error`) — их держит прежняя
                    // ветка «маски нет».
                    && !l.contains("://")
                    && !l.contains('?')
            };
            if !layers.is_empty()
                && layers.iter().all(|l| plain(l) && crate::background::source(l).is_none())
            {
                return;
            }
        }
        // Размытая картинка выходит за края элемента — в браузере тоже.
        // Вчетверо шире радиуса: маска композита обязана лежать там, где
        // размытая картинка уже сошла на нет, иначе край режется прямоугольником.
        let margin = px(self.blur * 4.0);
        // Форма клипа НЕ ограничена коробкой (css-masking §1.2: обрезается
        // только краска ФОРМОЙ): `circle(closest-corner at ...)` выходит за
        // края, и содержимое в её пределах обязано остаться видимым
        // (clip-path-circle-closest-corner). Область композита расширяется
        // до объединения коробки с рамкой формы.
        let shape_ext = self.mask.as_deref().and_then(|src| {
            // `border-shape`: вынос области записан в спеке маски
            // (`render::grouped` ← `Computed::border_shape_ext`): половина
            // обводки наружу, margin-box, запас под митры. Порядок в записи
            // t r b l, здесь — l t r b.
            if let Some(spec) = src.strip_prefix("bordershape:") {
                let head = spec.split_once(':')?.0;
                let v: Vec<f32> = head
                    .split_whitespace()
                    .filter_map(|t| t.parse::<f32>().ok())
                    .collect();
                if v.len() != 9 {
                    return None;
                }
                return Some((v[8], v[5], v[6], v[7]));
            }
            let raw = src.strip_prefix("shape:")?;
            if raw.starts_with("rrect(") {
                return None;
            }
            let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let (cx, cy, rx, ry) = crate::background::shape_params(raw, bw, bh, 1.0)?;
            let l = (rx - cx).max(0.0);
            let t = (ry - cy).max(0.0);
            let r = (cx + rx - bw).max(0.0);
            let b = (cy + ry - bh).max(0.0);
            (l + t + r + b > 0.0).then_some((l, t, r, b))
        });
        let (sl, st, sr, sb) = shape_ext.unwrap_or((0.0, 0.0, 0.0, 0.0));
        // Опорная коробка шире `bounds` (`clip-path: margin-box`, срезы с
        // отрицательными краями): буфер группы кроет и поля, иначе краска
        // там (outline) терялась бы вместе с буфером.
        let (sl, st, sr, sb) = match self.clip_inset {
            Some([t, r, b, l]) => {
                let neg = |v: crate::value::Len| match v {
                    crate::value::Len::Px(p) if p < 0.0 => -p,
                    _ => 0.0,
                };
                (sl.max(neg(l)), st.max(neg(t)), sr.max(neg(r)), sb.max(neg(b)))
            }
            None => (sl, st, sr, sb),
        };
        let area = Bounds {
            origin: gpui::point(
                bounds.origin.x - margin - px(sl),
                bounds.origin.y - margin - px(st),
            ),
            size: gpui::size(
                bounds.size.width + margin * 2.0 + px(sl + sr),
                bounds.size.height + margin * 2.0 + px(st + sb),
            ),
        };
        // Изоляция не обрезает (css-compositing-1 §isolation: группа меняет
        // только порядок сложения): смешиваемый ребёнок на 50 точек за краем
        // контейнера (`mix-blend-mode-overflowing-child`) и кольцо 10 точек
        // (`-blended-element-with-transparent-pixels`) обязаны попасть в кадр.
        // Буфер группы — во всё окно, поэтому и область композита — окно;
        // обрезку предков несёт маска содержимого самого композита.
        let area = if self.spill {
            Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: window.viewport_size(),
            }
        } else {
            area
        };
        // Вершины считаются от ОПОРНОЙ коробки формы (bounds ± края:
        // margin-box шире, content-box уже); проценты — доли её сторон,
        // точки — как есть (clip-path-polygon-008).
        let (polygon, polygon_clip) = polygon_clip::geometry(
            self,
            bounds,
            _prepaint.0,
            window.scale_factor(),
            window.current_transformation() == gpui::TransformationMatrix::unit(),
        );
        // Плитка маски: у растра — его точки как CSS-точки (density 1), у
        // рисунка без размера и градиента — сама коробка (mask-size auto,
        // css-masking §7.4); `mask-size` подменяет размер, `mask-position`
        // смещает (доля — от свободного места, как у background-position).
        // Битый источник — маски нет, элемент виден целиком
        // (bad-mask-image-svg-*).
        // Полигон сверх восьми вершин (предел шейдера) или с `evenodd` —
        // растровой маской-путём в системе коробки (clip-path-polygon-004/005).
        let poly_mask = if self.mask.is_none() && (polygon.len() > 8 || self.polygon_evenodd) {
            let d: Vec<String> = polygon
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "{}{} {}",
                        if i == 0 { "M" } else { "L" },
                        f32::from(p.x - bounds.origin.x),
                        f32::from(p.y - bounds.origin.y)
                    )
                })
                .collect();
            let rule = if self.polygon_evenodd { "evenodd" } else { "nonzero" };
            Some(format!("pathdef:{rule}:{} Z", d.join(" ")))
        } else {
            None
        };
        let polygon = if poly_mask.is_some() { Vec::new() } else { polygon };
        let mask = self.mask.as_deref().or(poly_mask.as_deref()).and_then(|src| {
            // `border-shape` (css-borders-4): маска — внешний контур рамки,
            // SVG-растр на РАСШИРЕННУЮ область (обводка выходит за
            // border-box), одной плиткой без мощения; альфа = покрытие.
            if let Some(spec) = src.strip_prefix("bordershape:") {
                let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let (aw, ah) = (bw + sl + sr, bh + st + sb);
                let markup =
                    crate::background::border_shape_mask_svg(spec, bw, bh, sl, st, aw, ah)?;
                let img = crate::svg::rasterize(&markup, aw, ah)?;
                return Some((
                    img,
                    Bounds {
                        origin: gpui::point(bounds.origin.x - px(sl), bounds.origin.y - px(st)),
                        size: gpui::size(px(aw), px(ah)),
                    },
                    // Одна плитка: за пределами области пусто.
                    3,
                ));
            }
            // Слои: `url(a), url(b)` — полотно, собранное по mask-composite;
            // одиночный слой идёт плиткой прямо в композит.
            let layers: Vec<String> = if src.starts_with("shape:") {
                vec![src.to_string()]
            } else {
                crate::css::split_args(src)
                    .iter()
                    .filter_map(|l| mask_layer_source(l))
                    .collect()
            };
            let src: &str = layers.first().map(String::as_str)?;
            let bounds = _prepaint.1;
            // Коробка укладки (`mask-origin`): плитка и её свободное место
            // считаются от неё, а не от border-box.
            let [ot, or_, ob, ol] = self.mask_origin_off;
            let (bw, bh) = (
                f32::from(bounds.size.width) - ol - or_,
                f32::from(bounds.size.height) - ot - ob,
            );
            let len = |l: crate::value::Len, side: f32, auto: f32| match l {
                crate::value::Len::Px(v) => v,
                crate::value::Len::Pct(p) => p * side,
                _ => auto,
            };
            // Несколько слоёв или снимок определения из документа:
            // полотно, собранное по `mask-composite` (css-masking §7.12).
            // Плитка слоя — его интринзик (auto), укладка от угла коробки;
            // уложенное полотно уходит одной плиткой без мощения.
            let referenced = layers.iter().any(|l| {
                l.starts_with("svgsnap:")
                    || l.starts_with("clipsnap:")
                    || l.starts_with("pathdef:")
                    || l.starts_with("shapedef:")
                    || (l.contains('#') && l.contains(".svg"))
            });
            if layers.len() > 1 || referenced || !self.mask_repeat_modes.is_empty() {
                let sf = window.scale_factor();
                let (cw, ch) = (
                    (bw * sf).round().max(1.0) as u32,
                    (bh * sf).round().max(1.0) as u32,
                );
                // Укладка СВОЕГО слоя (css-masking-1 §7.6-7.7). Список короче
                // набора слоёв повторяется (css-backgrounds-3 §2.2); пустой —
                // старое поведение, одно значение на все слои.
                let repeat_of = |i: usize| -> (bool, bool) {
                    let v = &self.mask_repeat_list;
                    if v.is_empty() {
                        self.mask_no_repeat
                    } else {
                        v[i % v.len()]
                    }
                };
                // Точка укладки слоя: доля — от СВОБОДНОГО места (коробка
                // минус плитка), `right`/`bottom` зеркалят отсчёт — та же
                // арифметика, что на однослойном пути ниже.
                let pos_of = |i: usize, tw: f32, th: f32| -> (f32, f32) {
                    let v = &self.mask_pos_list;
                    let pick = if v.is_empty() {
                        self.mask_pos
                            .map(|(x, y)| (x, y, self.mask_pos_far.0, self.mask_pos_far.1))
                    } else {
                        Some(v[i % v.len()])
                    };
                    let Some((x, y, fx, fy)) = pick else {
                        return (0.0, 0.0);
                    };
                    let one = |l: crate::value::Len, free: f32, far: bool| {
                        let val = match l {
                            crate::value::Len::Pct(p) => p * free,
                            l => len(l, free, 0.0),
                        };
                        if far { free - val } else { val }
                    };
                    (one(x, bw - tw, fx), one(y, bh - th, fy))
                };
                let built: Vec<crate::background::MaskLayer> = layers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| {
                        let mut space_gap = (0.0, 0.0);
                        let mut space_once = (false, false);
                        // Ссылка на определение в документе: растр под
                        // коробку, светимость вместо альфы.
                        let (image, tile, lum) = if let Some(id) = l.strip_prefix("svgsnap:") {
                            // css-masking-1 §7.2: `match-source` у ссылки на
                            // `<mask>` — его `mask-type` (светимость по
                            // умолчанию); явный `mask-mode` главнее.
                            let alpha = !self.mask_luminance
                                && (self.mask_alpha_mode || id.ends_with('A'));
                            (
                                rasterize_mask_def(id, bw, bh, false)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                !alpha,
                            )
                        } else if let Some(id) = l.strip_prefix("clipsnap:") {
                            (
                                rasterize_mask_def(id, bw, bh, true)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else if let Some(markup) = l
                            .rsplit_once('#')
                            .filter(|(f, _)| f.ends_with(".svg"))
                            .and_then(|(file, frag)| svg_fragment(file, frag))
                        {
                            // CSS Masking §7.1: a <mask> reference is distinct
                            // from an SVG image URL with an ordinary fragment.
                            let markup = markup.replace("clip-rule", "fill-rule");
                            let markup = format!(
                                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}">{markup}</svg>"#
                            );
                            (
                                crate::svg::rasterize(&markup, bw, bh)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else if let Some(rest) = l.strip_prefix("shapedef:") {
                            // Команды `shape()` переводятся в контур `d`
                            // с резолвом долей по ОПОРНОЙ коробке формы
                            // (`shape(...) content-box`, css-masking
                            // §1.3.1.1): её края несёт poly_expand, контур
                            // сдвигается на них внутрь.
                            let (rule, body) = rest.split_once(':')?;
                            let [et, _er, _eb, el] = self.poly_expand;
                            let (fw2, fh2) = (
                                (bw + el + self.poly_expand[1]).max(1.0),
                                (bh + et + self.poly_expand[2]).max(1.0),
                            );
                            let d = crate::background::shape_to_path(body, fw2, fh2)?;
                            let (dx, dy) = (-el, -et);
                            let markup = format!(
                                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}"><g transform="translate({dx} {dy})"><path fill="#ffffff" fill-rule="{rule}" d="{d}"/></g></svg>"##
                            );
                            (
                                crate::svg::rasterize(&markup, bw, bh)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else if let Some(rest) = l.strip_prefix("pathdef:") {
                            // Контур `path()`: белая заливка с правилом
                            // намотки, светимость = покрытие.
                            let (rule, d) = rest.split_once(':')?;
                            let markup = format!(
                                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}"><path fill="#ffffff" fill-rule="{rule}" d="{d}"/></svg>"##
                            );
                            (
                                crate::svg::rasterize(&markup, bw, bh)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else {
                            let source = crate::background::source(l)?;
                            let (tw, th) = mask_size::tile(
                                source.intrinsic(), self.mask_scale, (bw, bh), self.mask_size, self.mask_fit,
                            );
                            let modes = self.mask_repeat_modes.as_slice();
                            let (mx, my) = if modes.is_empty() { (0, 0) } else { modes[i % modes.len()] };
                            let (tw, th) = mask_size::round_tile(
                                (tw, th),
                                (bw, bh),
                                (mx == 3, my == 3),
                                self.mask_size,
                            );
                            // Плитка кладётся не в угол, а в точку СВОЕГО
                            // слоя: без этого `mask-position: top, bottom`
                            // сваливал оба слоя в (0,0) (mask-position-5).
                            let (ox, oy) = pos_of(i, tw, th);
                            // `space` (css-backgrounds-3 §3.4): whole tiles
                            // with equal gaps, the outer ones touching the
                            // edges; position only acts when fewer than two fit.
                            let (ox, gx, nx) = mask_size::space_axis(mx == 2, ox, tw, bw);
                            let (oy, gy, ny) = mask_size::space_axis(my == 2, oy, th, bh);
                            space_gap = (gx * sf, gy * sf);
                            space_once = (nx, ny);
                            (
                                source.mask_raster((tw, th), sf)?,
                                [ox * sf, oy * sf, tw * sf, th * sf],
                                false,
                            )
                        };
                        let no_repeat = repeat_of(i);
                        Some(crate::background::MaskLayer {
                            image,
                            tile,
                            no_repeat: (no_repeat.0 || space_once.0, no_repeat.1 || space_once.1),
                            gap: space_gap,
                            snap: !self.mask_repeat_modes.is_empty(),
                            // Список операторов КОРОЧЕ набора слоёв
                            // повторяется (css-masking-1 §7.12 ->
                            // css-backgrounds-3 §2.2): прежде слоям сверх
                            // длины доставался `add`, и `mask-composite:
                            // subtract` из трёх слоёв считался только на
                            // верхнем (mask-composite-1d 3.17). При ДВУХ
                            // слоях итог не меняется: оператор нижнего слоя
                            // не читается вовсе (ветка `first` в
                            // `compose_mask_layers`).
                            op: if self.mask_composite.is_empty() {
                                0
                            } else {
                                self.mask_composite[i % self.mask_composite.len()]
                            },
                            luminance: lum || self.mask_luminance,
                        })
                    })
                    .collect();
                // Snapped tiles need a canvas on the device grid: its origin
                // rounds, and the tiles keep their exact device positions.
                let (mut built, mut origin, mut size) = (
                    built,
                    gpui::point(bounds.origin.x + px(ol), bounds.origin.y + px(ot)),
                    gpui::size(px(bw), px(bh)),
                );
                if !self.mask_repeat_modes.is_empty() {
                    let (ex, ey) = (f32::from(origin.x) * sf, f32::from(origin.y) * sf);
                    let (rx, ry) = (ex.round(), ey.round());
                    for layer in &mut built {
                        layer.tile[0] += ex - rx;
                        layer.tile[1] += ey - ry;
                    }
                    origin = gpui::point(px(rx / sf), px(ry / sf));
                    size = gpui::size(px(cw as f32 / sf), px(ch as f32 / sf));
                }
                let img = crate::background::compose_mask_layers(&built, cw, ch)?;
                return Some((
                    img,
                    Bounds { origin, size },
                    // Светимость уже учтена при сборке полотна.
                    3,
                ));
            }
            let source = crate::background::source(src)?;
            let (img, tw, th) = match &source {
                crate::background::Source::Raster(img) => {
                    let (tw, th) = mask_size::tile(
                        source.intrinsic(), self.mask_scale, (bw, bh), self.mask_size, self.mask_fit,
                    );
                    (img.clone(), tw, th)
                }
                _ => {
                    let (tw, th) = mask_size::tile(
                        source.intrinsic(), self.mask_scale, (bw, bh), self.mask_size, self.mask_fit,
                    );
                    // Растр — в физических точках окна: маска в CSS-точках
                    // растягивалась при композите и мылила край формы
                    // (clip-path-circle-010 и родня: 0.71 вместо нуля).
                    let sf = window.scale_factor();
                    let img = match &source {
                        crate::background::Source::Shape { raw }
                            if !raw.trim_start().starts_with("rrect(") =>
                        {
                            // Форма может выйти за коробку — растр кроет
                            // расширенную область, центр смещён на вынос.
                            //
                            // Радиусы и центр считаются от ОПОРНОЙ КОРОБКИ
                            // формы (css-masking-1 §1.3.1.1): её края несёт
                            // `poly_expand`, как и у полигона. Прежде круг
                            // всегда мерился border-box, и
                            // `circle(farthest-side) content-box` выходил
                            // радиусом во всю коробку
                            // (`clip-path-contentBox-1a/1d/1e`,
                            // `-fillBox-*`, `-viewBox-*`).
                            let [pt, pr, pb, pl] = self.poly_expand;
                            let (rw, rh) = ((bw + pl + pr).max(1.0), (bh + pt + pb).max(1.0));
                            let (cx0, cy0, rx, ry) =
                                crate::background::shape_params(raw, rw, rh, 1.0)?;
                            let (cx, cy) = (cx0 - pl, cy0 - pt);
                            let (aw, ah) = (bw + sl + sr, bh + st + sb);
                            let img = crate::background::rasterize_ellipse_px(
                                (cx + sl) * sf,
                                (cy + st) * sf,
                                rx * sf,
                                ry * sf,
                                (aw * sf).round().max(1.0) as u32,
                                (ah * sf).round().max(1.0) as u32,
                            )?;
                            return Some((
                                img,
                                Bounds {
                                    origin: gpui::point(
                                        bounds.origin.x - px(sl),
                                        bounds.origin.y - px(st),
                                    ),
                                    size: gpui::size(px(aw), px(ah)),
                                },
                                // Одна плитка: за пределами области пусто.
                                3,
                            ));
                        }
                        crate::background::Source::Shape { raw } => {
                            // Точечные величины формы записаны в CSS-точках —
                            // растеризатору нужен их масштаб.
                            crate::background::rasterize_shape(
                                raw,
                                (tw * sf).round().max(1.0) as u32,
                                (th * sf).round().max(1.0) as u32,
                                sf,
                            )?
                        }
                        // Рисунок со СВОИМ размером растрируется в нём:
                        // без viewBox при большем вьюпорте содержимое НЕ
                        // растёт, и плитка выходила с прозрачными полосами
                        // (mask-repeat-1: щели; mask-size-cover: четверть).
                        // На плитку его натянет сэмплер композита. Без
                        // своего размера — точно в плитку (CSS-точки,
                        // чёткость даёт плотность растеризатора).
                        // Плитка в CSS-точках; рисунок масштабирует
                        // with_viewport (viewBox из своих размеров).
                        _ => source.mask_raster((tw, th), sf)?,
                    };
                    (img, tw, th)
                }
            };
            if std::env::var("MASK_DBG").is_ok() {
                eprintln!(
                    "MASK bounds=({:?},{:?} {:?}x{:?}) tile={tw}x{th} sf={}",
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                    window.scale_factor()
                );
            }
            let (ox, oy) = match self.mask_pos {
                Some((x, y)) => {
                    // Доля — от свободного места; `right/bottom` зеркалит
                    // отсчёт (css-backgrounds §3.6, mask-position-1a).
                    let one = |l: crate::value::Len, free: f32, far: bool| {
                        let v = match l {
                            crate::value::Len::Pct(p) => p * free,
                            l => len(l, free, 0.0),
                        };
                        if far { free - v } else { v }
                    };
                    (
                        one(x, bw - tw, self.mask_pos_far.0),
                        one(y, bh - th, self.mask_pos_far.1),
                    )
                }
                None => (0.0, 0.0),
            };
            Some((
                img,
                mask_size::snap_tile(
                    gpui::point(bounds.origin.x + px(ol + ox), bounds.origin.y + px(ot + oy)),
                    (tw, th),
                    window.scale_factor(),
                ),
                ((self.mask_no_repeat.0 as u32)
                    | ((self.mask_no_repeat.1 as u32) << 1)
                    | ((self.mask_luminance as u32) << 2)),
            ))
        });
        // Коробка окраски (`mask-clip`): вне её маска не красится — элемент
        // там скрыт (mask-size-contain-clip-padding).
        let mask_clip = polygon_clip::intersect(
            rectangular_clip::resolve(self, _prepaint.0, window.scale_factor()),
            polygon_clip,
        );
        // Подложка (наружные тени `border-shape`) — в текущий контекст ДО
        // композита группы: под буфером и вне его маски, на области выноса.
        let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let (aw, ah) = (bw + sl + sr, bh + st + sb);
        let layer_at = Bounds {
            origin: gpui::point(bounds.origin.x - px(sl), bounds.origin.y - px(st)),
            size: gpui::size(px(aw), px(ah)),
        };
        if let Some(under) = self.under.as_ref()
            && let Some(markup) = under(bw, bh, sl, st, aw, ah)
            && let Some(img) = crate::svg::rasterize(&markup, aw, ah)
        {
            let _ = window.paint_image_with_sampling(layer_at, gpui::Corners::default(), img, 0, false, gpui::ImageSampling::Linear);
        }
        // css-shapes-1 §basic-shape-rect: `round` rounds the corners of the
        // clip rectangle itself; the composite quad carries those radii.
        // Percentages: Blink BasicShapeInset resolves radii against the
        // reference box; one scalar radius per corner takes the smaller axis.
        let round = match self.clip_round {
            Some(crate::value::Len::Px(v)) => v,
            Some(crate::value::Len::Pct(p)) => {
                p * f32::from(_prepaint.0.size.width).min(f32::from(_prepaint.0.size.height))
            }
            _ => 0.0,
        };
        let (area, corners) = match mask_clip {
            Some([x, y, w, h])
                if round > 0.0
                    && mask.is_none()
                    && polygon.is_empty()
                    && w > 0.0
                    && h > 0.0 =>
            {
                let sf = window.scale_factor();
                let (w, h) = (w / sf, h / sf);
                (
                    Bounds {
                        origin: gpui::point(px(x / sf), px(y / sf)),
                        size: gpui::size(px(w), px(h)),
                    },
                    gpui::Corners::all(px(round.min(w / 2.0).min(h / 2.0))),
                )
            }
            _ => (area, gpui::Corners::default()),
        };
        let child = self.child.as_mut().unwrap();
        window.paint_group(
            area,
            corners,
            self.blur,
            self.opacity,
            self.blend,
            &polygon,
            mask,
            mask_clip,
            |window| child.paint(window, cx),
        );
        // Накладка (кольцо `border-shape` над обрезанным содержимым) — после
        // композита, в тот же контекст.
        for over in &self.over {
            if let Some(markup) = over(bw, bh, sl, st, aw, ah)
                && let Some(img) = crate::svg::rasterize(&markup, aw, ah)
            {
                let _ = window.paint_image_with_sampling(layer_at, gpui::Corners::default(), img, 0, false, gpui::ImageSampling::Linear);
            }
        }
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
    /// Матрица функций в порядке записи (см. `computed::Transform::lin`).
    pub lin: [[f32; 2]; 2],
    /// Сдвиг: пиксели, доля ширины, доля высоты.
    pub tr: [[f32; 3]; 2],
    /// Полная 4×4 элемента и доли размера в её столбце сдвига
    /// (`computed::Transform::m4`/`m4_pct`); `has_3d` — идти по ней.
    pub m4: [[f32; 4]; 4],
    pub m4_pct: [[f32; 2]; 4],
    pub has_3d: bool,
    /// `backface-visibility: hidden` — решается по `m4[2][2]` на отрисовке,
    /// коробка держит место.
    pub backface_hidden: bool,
    /// Третья координата `transform-origin` в css-точках.
    pub origin_z: Option<f32>,
    /// Своя `perspective` (css-точки, ≥ 1px), её точка отсчёта долями и
    /// точками по осям, и ячейка, куда `paint` кладёт T(po)·P(d)·T(−po) в
    /// точках устройства — для объёмных детей.
    pub perspective: Option<f32>,
    pub perspective_origin: (f32, f32),
    pub perspective_origin_px: (Option<f32>, Option<f32>),
    pub perspective_frame: Option<crate::computed::PerspectiveFrame>,
    /// Ячейка ПРЯМОГО родителя: объёмный путь домножает на неё слева
    /// (css-transforms-2 §3d-transform-rendering, п.3).
    pub under_perspective: Option<crate::computed::PerspectiveFrame>,
    /// Своя ячейка объёмного контекста (`transform-style: preserve-3d`):
    /// `paint` кладёт в неё накопленную 4×4 и свою аффинную долю ДО детей.
    pub frame_3d: Option<crate::computed::Frame3d>,
    /// Ячейка объёмного контекста ПРЯМОГО родителя: своя матрица копится
    /// поверх неё, изнанка решается по накопленной, доля родителя снимается.
    pub under_3d: Option<crate::computed::Frame3d>,
    /// Чистый плоский сдвиг уже перенесён в место раскладки на подготовке
    /// (`prepaint`, `Window::set_layout_placed_origin`): `paint` рисует без
    /// матрицы.
    placed: bool,
    /// Неокруглённое место коробки из подготовки: осевой поворот на
    /// отрисовке округляет края от него (`layout_origin_unrounded` доступен
    /// только до отрисовки).
    exact_origin: Option<gpui::Point<Pixels>>,
    /// Reference box shared by the cells of a transformed table row or row
    /// group (css-transforms-1 §transformable-element: the row has no box
    /// of its own in our grid): every cell unions its unrounded box into it
    /// on prepaint, and paint resolves origin/percentages against it.
    pub ref_box: Option<RefBox>,
}

/// Union of unrounded boxes, filled on prepaint (see `Transformed::ref_box`).
pub type RefBox = std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>;

/// Сплющивание плоскости z=0 в аффинную матрицу экрана
/// (css-transforms-2 §3d-transform-rendering).
///
/// Точка плоскости (x, y, 0, 1) уходит в (F0·p, F1·p, ·, F3·p); экран —
/// деление на w = F3·p. Если w не зависит от x и y (строка 3 без x/y —
/// весь класс `translateZ` + `perspective()`), результат ТОЧНО аффинный.
/// Иначе (rotateX/Y под `perspective()` — трапеция, которую квад gpui не
/// рисует) берём касательную аффинную карту в центре коробки:
/// детерминированно и одинаково для теста и эталона с той же гомографией
/// (transform3d-matrix3d-003/-004). `None` — плоскость за глазом или ребром.

fn flatten_plane(f: &[[f32; 4]; 4], center: (f32, f32)) -> Option<gpui::TransformationMatrix> {
    const EPS: f32 = 1e-5;
    if crate::computed::det3_plane(f).abs() < EPS {
        return None;
    }
    let (cx, cy) = center;
    if f[3][0].abs() < 1e-9 && f[3][1].abs() < 1e-9 {
        let w = f[3][3];
        if w <= 1e-9 {
            return None;
        }
        return Some(gpui::TransformationMatrix {
            rotation_scale: [[f[0][0] / w, f[0][1] / w], [f[1][0] / w, f[1][1] / w]],
            translation: [f[0][3] / w, f[1][3] / w],
        });
    }
    let wc = f[3][0] * cx + f[3][1] * cy + f[3][3];
    if wc <= 1e-6 {
        return None;
    }
    let x = (f[0][0] * cx + f[0][1] * cy + f[0][3]) / wc;
    let y = (f[1][0] * cx + f[1][1] * cy + f[1][3]) / wc;
    let j = |i: usize, k: usize, v: f32| (f[i][k] - v * f[3][k]) / wc;
    let rs = [[j(0, 0, x), j(0, 1, x)], [j(1, 0, y), j(1, 1, y)]];
    Some(gpui::TransformationMatrix {
        rotation_scale: rs,
        translation: [x - rs[0][0] * cx - rs[0][1] * cy, y - rs[1][0] * cx - rs[1][1] * cy],
    })
}

/// Обратная аффинная `[[a, b, tx], [c, d, ty]]`.
///
/// Нужна ровно затем, чтобы снять долю родителя: gpui складывает вложенные
/// `with_transformation` как `inner∘outer` (`window.rs:2789`; обратный
/// порядок замерен и откачен), поэтому ребёнок объёмного контекста, желая
/// оказаться на абсолютной `G`, обязан втолкнуть `G ∘ F_родителя⁻¹`.
fn invert_affine(m: [[f32; 3]; 2]) -> Option<gpui::TransformationMatrix> {
    let (a, b, tx) = (m[0][0], m[0][1], m[0][2]);
    let (c, d, ty) = (m[1][0], m[1][1], m[1][2]);
    let det = a * d - b * c;
    if det.abs() < 1e-9 {
        return None;
    }
    Some(gpui::TransformationMatrix {
        rotation_scale: [[d / det, -b / det], [-c / det, a / det]],
        translation: [(b * ty - d * tx) / det, (c * tx - a * ty) / det],
    })
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
            lin: [[1.0, 0.0], [0.0, 1.0]],
            tr: [[0.0; 3]; 2],
            m4: crate::computed::IDENTITY4,
            m4_pct: [[0.0; 2]; 4],
            has_3d: false,
            backface_hidden: false,
            origin_z: None,
            perspective: None,
            perspective_origin: (0.5, 0.5),
            perspective_origin_px: (None, None),
            perspective_frame: None,
            under_perspective: None,
            frame_3d: None,
            under_3d: None,
            placed: false,
            exact_origin: None,
            ref_box: None,
        }
    }

    /// Плоская матрица — чистый сдвиг (линейная часть единичная с точностью
    /// до ошибки `f32`: `rotate(360deg)` даёт sin ≈ 1e-7): сдвиг в css-точках
    /// для коробки `w × h`.
    fn pure_shift(&self, w: f32, h: f32) -> Option<(f32, f32)> {
        let [[a, b], [c, d]] = self.lin;
        let eps = 1e-5;
        let id = (a - 1.0).abs() < eps && b.abs() < eps && c.abs() < eps && (d - 1.0).abs() < eps;
        let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
        let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
        (id && sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }
}

impl Element for Transformed {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Плоская матрица — на стек якорей (`anchor::tf_push`): рамку якоря
        // проба снимает на подготовке, а трансформ применяется только в
        // `paint`, и без стека якорь виделся до трансформа (css-anchor-
        // position-1 §2 «includes … transforms»; `transform-001/002/009`).
        // Формула та же, что у плоского пути `paint`, но в css-точках, без
        // `scale_factor`: x' = o + lin·(x − o) + сдвиг. Объёмный путь в стек
        // не идёт — его матрица решается на отрисовке по накопленной ячейке.
        let flat = !self.has_3d && self.frame_3d.is_none() && self.under_3d.is_none();
        // Чистый плоский сдвиг — смена начала координат (css-transforms-1
        // §transform-rendering): коробка обязана рисоваться байт в байт как
        // разложенная на сдвинутом месте. Матрицей дробный сдвиг устройства
        // (10px × 1.25) ложился ПОСЛЕ округления краёв раскладки и выбора
        // подпикселя глифов — края и текст расходились на точку с эталоном
        // на `top/left` (Blink так же проносит дробное смещение сквозь
        // 2D-сдвиг: `PaintPropertyTreeBuilder`, subpixel accumulation).
        // Поддерево переносится до округления (`set_layout_placed_origin`,
        // тот же механизм у `LatePlace`), края округляются на конечном месте.
        self.placed = false;
        self.exact_origin = Some(window.layout_origin_unrounded(*layout_id));
        if let Some(r) = self.ref_box.as_ref() {
            let own = gpui::Bounds {
                origin: window.layout_origin_unrounded(*layout_id),
                size: window.layout_size_unrounded(*layout_id),
            };
            r.set(Some(r.get().map_or(own, |u| u.union(&own))));
        }
        if flat && self.perspective.is_none() {
            let size = window.layout_size_unrounded(*layout_id);
            if let Some((sx, sy)) =
                self.pure_shift(f32::from(size.width), f32::from(size.height))
            {
                let origin = window.layout_origin_unrounded(*layout_id);
                window.set_layout_placed_origin(*layout_id, origin + gpui::point(px(sx), px(sy)));
                self.placed = true;
                self.child
                    .as_mut()
                    .unwrap()
                    .prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
                return;
            }
        }
        if flat {
            let (w, h) = {
                let exact = window.layout_size_unrounded(*layout_id);
                (f32::from(exact.width), f32::from(exact.height))
            };
            let origin = self.scaled_origin(bounds.origin);
            let ox = f32::from(origin.x) + w * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
            let oy = f32::from(origin.y) + h * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
            let sx = self.tr[0][0] + w * self.tr[0][1] + h * self.tr[0][2];
            let sy = self.tr[1][0] + w * self.tr[1][1] + h * self.tr[1][2];
            let [[a, b], [c, d]] = self.lin;
            crate::anchor::tf_push([
                [a, b, ox - a * ox - b * oy + sx],
                [c, d, oy - c * ox - d * oy + sy],
            ]);
        }
        self.child.as_mut().unwrap().prepaint(window, cx);
        if flat {
            crate::anchor::tf_pop();
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placed {
            self.child.as_mut().unwrap().paint(window, cx);
            return;
        }
        let scale_factor = window.scale_factor();
        // CSS transform origins precede device-pixel snapping (Transforms 1 §3).
        let raw_origin = self.exact_origin.unwrap_or(bounds.origin);
        let scaled_origin = self.scaled_origin(bounds.origin);
        let dev = |v: f32| px(v).scale(scale_factor);
        // Точка отсчёта — в устройстве, от неё и разворачиваем. Записанная
        // длиной, она сильнее доли: `transform-origin: 0 0` — левый верх, а
        // не центр (доля из длины считается только здесь, где размер известен).
        // Доля × размер ПЛЮС точки: `calc(50% + 10px)` — смесь, и доля у
        // чистых точек равна нулю (css-transforms-1 §5.2).
        // Доли (`transform-origin: 50%`, `translate(100%)`) — от размера
        // раскладки, а не от округлённых к точке устройства краёв: 50px ×
        // 1.25 = 62.5 округлялось до 63, и `translateY(100%)` уезжал на
        // 0.4px от `translateY(50px)` (`transform-percent-*`).
        let exact = window.layout_size_unrounded(*layout_id);
        let (w, h) = (f32::from(exact.width), f32::from(exact.height));
        // Reference box of a table row/row group spread over its cells
        // (`ref_box`): origin and percentages resolve against it, offset
        // from this cell's own box.
        let (rw, rh, rdx, rdy) = match self.ref_box.as_ref().and_then(|r| r.get()) {
            Some(r) => (
                f32::from(r.size.width),
                f32::from(r.size.height),
                f32::from(r.origin.x - raw_origin.x),
                f32::from(r.origin.y - raw_origin.y),
            ),
            None => (w, h, 0.0, 0.0),
        };
        let ox = rdx + rw * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
        let oy = rdy + rh * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
        let origin = gpui::point(
            dev(f32::from(scaled_origin.x) + ox),
            dev(f32::from(scaled_origin.y) + oy),
        );
        let back = gpui::point(
            dev(-(f32::from(scaled_origin.x) + ox)),
            dev(-(f32::from(scaled_origin.y) + oy)),
        );
        // Матрица функций в порядке записи (css-transforms-1
        // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
        // возвращается. Проценты сдвига считаются от собственного размера —
        // он известен только здесь, на отрисовке.
        let shift = |row: [f32; 3]| (row[0] + rw * row[1] + rh * row[2]) * scale_factor;
        // Изнанка (css-transforms-2 §backface-visibility): элемент разложен и
        // держит место, но не рисуется. m33 — из полной 4×4 самого элемента;
        // у плоских функций он равен 1, так что 2D-путь сюда не попадает.
        // …и по НАКОПЛЕННОЙ, когда элемент внутри объёмного контекста
        // (css-transforms-2 §backface-visibility, «m33 < 0 → not rendered»,
        // где m33 — от накопленной: transform3d-backface-visibility-004/006,
        // backface-visibility-hidden-004, backface-visibility-with-sibling-001).
        // `m33` не зависит ни от сдвигов, ни от свёртки `S·M·S⁻¹`, поэтому
        // считается прямо здесь, до перевода в точки устройства.
        let accum33 = match self.under_3d.as_ref().and_then(|f| f.get()) {
            Some((a, _)) => crate::computed::mul4(a, self.m4)[2][2],
            None => self.m4[2][2],
        };
        // Владелец `preserve-3d` изнанкой уносит только СЕБЯ: его дети —
        // отдельные плоскости того же контекста и решают свою видимость сами
        // (composited-under-rotateY-180deg-preserve-3d: зелёный ребёнок под
        // `backface-visibility: hidden; rotateY(180deg); preserve-3d`).
        if self.backface_hidden && accum33 < 0.0 && self.frame_3d.is_none() {
            return;
        }
        // Своя `perspective` (css-transforms-2 §perspective-matrix-computation):
        // T(po)·P(d)·T(−po) в точках устройства — в ячейку для детей ДО их
        // отрисовки, на обоих путях. Свёртка та же, что у объёмного пути ниже:
        // сдвиги ×sf, m34 = −1/(d·sf). Сам элемент перспективой не трогается
        // (она действует только на детей) и идёт своим путём как прежде.
        if let (Some(d), Some(frame)) = (self.perspective, self.perspective_frame.as_ref()) {
            use crate::computed::{mul4, Transform};
            let px = self
                .perspective_origin_px
                .0
                .unwrap_or(w * self.perspective_origin.0);
            let py = self
                .perspective_origin_px
                .1
                .unwrap_or(h * self.perspective_origin.1);
            let (px_d, py_d) = (
                (f32::from(raw_origin.x) + px) * scale_factor,
                (f32::from(raw_origin.y) + py) * scale_factor,
            );
            let p = mul4(
                mul4(
                    Transform::translate4(px_d, py_d, 0.0),
                    Transform::perspective4(d * scale_factor),
                ),
                Transform::translate4(-px_d, -py_d, 0.0),
            );
            frame.set(Some(p));
        }
        // Плоский путь годится, только когда объёмного контекста рядом нет:
        // и владелец `preserve-3d`, и его ребёнок идут по 4×4, даже когда
        // своих объёмных функций у них нет — `m4` держит и плоские функции
        // (`computed::Transform::m4`, «плоские вкладываются как есть»).
        let in_3d =
            self.frame_3d.is_some() || self.under_3d.as_ref().and_then(|f| f.get()).is_some();
        if !self.has_3d && !in_3d {
            // Плоский путь — прежний по матрице. Маски детей (`overflow`,
            // плитки фона, полосы рамки) едут вместе с содержимым
            // (`Window::with_transformation_masked`): прежде обрезка стояла
            // на месте коробки до `transform` (`transform-clip-001`,
            // `transform-background-001/002`, `transform-fixed-bg-001/003`).
            let quarter = quarter_turn(self.lin)
                .filter(|_| window.current_transformation() == gpui::TransformationMatrix::unit());
            let mut matrix = gpui::TransformationMatrix::unit()
                .translate(origin)
                .compose(gpui::TransformationMatrix {
                    rotation_scale: quarter.unwrap_or(self.lin),
                    translation: [shift(self.tr[0]), shift(self.tr[1])],
                })
                .translate(back);
            let fill_matrix = quarter.map(|_| {
                self.exact_fill_matrix(matrix, bounds.origin, raw_origin, scale_factor)
            });
            if quarter.is_some() {
                // Поворот на кратное четверти (и отражение) оставляет коробку
                // осевой: её края обязаны округляться к точке устройства так
                // же, как у той же коробки, разложенной на месте (чистый сдвиг
                // выше идёт через раскладку — round half up). Растеризатор
                // по правилу «верх-лево» относит ровную половину вниз, а
                // ошибка `f32` у `rotate(-90deg)` (cos ≈ −4e-8) решает
                // ничью случайно — `offset-path-ray-011/013/014` против
                // эталона `translate(...)`. Skia так же кладёт осевой
                // прямоугольник по round(x) (`SkScan::FillRect`), а
                // `gfx::SinCosDegrees` даёт точные 0/±1 у кратных 90°.
                let sf = scale_factor;
                let corner_min = |o: gpui::Point<Pixels>, w: f32, h: f32| {
                    let mut m = (f32::INFINITY, f32::INFINITY);
                    for (dx, dy) in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
                        let p = matrix.apply(gpui::point(
                            px((f32::from(o.x) + dx) * sf),
                            px((f32::from(o.y) + dy) * sf),
                        ));
                        m = (m.0.min(f32::from(p.x)), m.1.min(f32::from(p.y)));
                    }
                    m
                };
                let exact_origin = self.exact_origin.unwrap_or(bounds.origin);
                let exact = corner_min(exact_origin, w, h);
                let cur = corner_min(
                    bounds.origin,
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                );
                let snap = |v: f32| ((v * 64.0).round() / 64.0 + 0.5).floor();
                matrix.translation[0] += snap(exact.0) - cur.0;
                matrix.translation[1] += snap(exact.1) - cur.1;
            }
            let child = self.child.as_mut().unwrap();
            window.with_transformation_masked(matrix, |window| {
                if let Some(exact) = fill_matrix {
                    window.with_css_fill_transform(exact, |window| child.paint(window, cx));
                } else {
                    child.paint(window, cx);
                }
            });
            return;
        }
        // --- Объёмный путь: одна 4×4 ОДНОГО элемента, сплющенная на экран ---
        use crate::computed::{det4, mul4, Transform};
        let sf = scale_factor;
        // Из css-точек в точки устройства — подобие S·M·S⁻¹, S = diag(sf, sf,
        // sf, 1): столбец сдвига строк 0..2 умножается на sf, строка w
        // столбцов 0..2 делится на sf, m44 НЕ трогается. (В шаге 1 цикл `0..4`
        // домножал и m44 — `flatten_plane` делила на него всю матрицу, и
        // каждый объёмный элемент сжимался в 1/sf; scout-3d-2026-09b.md §1.)
        let mut own = self.m4;
        for i in 0..3 {
            own[i][3] = (own[i][3] + rw * self.m4_pct[i][0] + rh * self.m4_pct[i][1]) * sf;
        }
        for j in 0..3 {
            own[3][j] /= sf;
        }
        // Точка отсчёта по трём осям, в точках устройства: T(o)·M·T(−o).
        // `ox`/`oy` посчитаны выше в css-точках; `ScaledPixels.0` — pub(crate)
        // в gpui, поэтому `origin.x.0` отсюда не читается.
        let oz = self.origin_z.unwrap_or(0.0) * sf;
        let (ox_d, oy_d) = (
            (f32::from(raw_origin.x) + ox) * sf,
            (f32::from(raw_origin.y) + oy) * sf,
        );
        let own = mul4(
            mul4(Transform::translate4(ox_d, oy_d, oz), own),
            Transform::translate4(-ox_d, -oy_d, -oz),
        );
        // Перспектива ПРЯМОГО родителя (§3d-transform-rendering, п.3:
        // «pre-multiply the parent element's perspective matrix»); стека
        // preserve-3d здесь ещё нет — внукам не достаётся
        // (perspective-children-only-*). Ячейку наполнил `paint` родителя в
        // этом же кадре (или прошлом — она переживает кадр), поэтому её видит
        // и отложенный слой абсолюта. Плоский ребёнок (z = 0) под
        // перспективой не меняется — потому только объёмный путь.
        let own = match self.under_perspective.as_ref().and_then(|f| f.get()) {
            Some(p) => mul4(p, own),
            None => own,
        };
        // Вырожденная 4×4 (`scale3d(2, 2, 0)`, transform3d-scale-004:
        // «singular, causes the contents not to display»).
        if det4(&own).abs() < 1e-9 {
            return;
        }
        // Накопленная матрица объёмного контекста (css-transforms-2
        // §accumulated-3d-transformation-matrix): A(родителя) · P(его
        // перспектива — домножена выше) · C(своя). Обе уже в точках
        // устройства, второй свёртки `S·M·S⁻¹` не возникает — ровно из-за
        // неё «3D full» терял −60 (computed.rs:691).
        let under = self.under_3d.as_ref().and_then(|f| f.get());
        let full = match under {
            Some((a, _)) => mul4(a, own),
            None => own,
        };
        let center = (
            (f32::from(raw_origin.x) + w * 0.5) * sf,
            (f32::from(raw_origin.y) + h * 0.5) * sf,
        );
        let flat = flatten_plane(&full, center);
        // Ячейка для СВОИХ детей — накопленная и своя аффинная доля;
        // наполняется ДО отрисовки детей, как у перспективы (и ради
        // отложенных слоёв абсолютов).
        if let Some(frame) = self.frame_3d.as_ref() {
            let share = flat.map_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], |m| {
                [
                    [
                        m.rotation_scale[0][0],
                        m.rotation_scale[0][1],
                        m.translation[0],
                    ],
                    [
                        m.rotation_scale[1][0],
                        m.rotation_scale[1][1],
                        m.translation[1],
                    ],
                ]
            });
            frame.set(Some((full, share)));
        }
        // Plane depth in the 3D rendering context (css-transforms-2
        // §3d-transform-rendering: planes of one context render by z, not
        // document order) — z/w of the box centre under the accumulated
        // matrix; the context root opens the sorting scope.
        let depth = {
            let v = [center.0, center.1, 0.0, 1.0];
            let row = |i: usize| (0..4).map(|k| full[i][k] * v[k]).sum::<f32>();
            let (z, wv) = (row(2), row(3));
            if wv.abs() > 1e-6 { z / wv } else { z }
        };
        let root_3d = self.frame_3d.is_some() && under.is_none();
        let in_context = root_3d || under.is_some();
        let paint_child = |child: &mut AnyElement, m: Option<gpui::TransformationMatrix>, masked: bool, window: &mut Window, cx: &mut App| {
            if in_context {
                let mut body = |window: &mut Window| {
                    window.paint_depth_plane(depth, |window| match m {
                        Some(m) if masked => window.with_transformation_masked(m, |window| child.paint(window, cx)),
                        Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                        None => child.paint(window, cx),
                    })
                };
                if root_3d {
                    window.paint_depth_context(body)
                } else {
                    body(window)
                }
            } else {
                match m {
                    Some(m) if masked => window.with_transformation_masked(m, |window| child.paint(window, cx)),
                    Some(m) => window.with_transformation(m, |window| child.paint(window, cx)),
                    None => child.paint(window, cx),
                }
            }
        };
        // Ребро (`rotateX(90deg)`) — не рисуется, как и прежняя нулевая
        // высота. Но в объёмном контексте ПОТОМКИ ребром не становятся
        // (transform3d-preserve3d-011: `rotateX(90)` над `rotateX(90)` =
        // 180°): краска идёт под единичной долей, а место каждый потомок
        // назначает себе сам по накопленной.
        let Some(flat) = flat else {
            if self.frame_3d.is_some() {
                let child = self.child.as_mut().unwrap();
                paint_child(child, Some(gpui::TransformationMatrix::unit()), false, window, cx);
            }
            return;
        };
        // Своя доля для gpui: родитель УЖЕ втолкнул `F_P`, а вложения
        // складываются как `inner∘outer` (`window.rs:2789`, порядок замерен и
        // оставлен) — значит втолкнуть надо `G ∘ F_P⁻¹`.
        let flat = match under.and_then(|(_, fp)| invert_affine(fp)) {
            Some(inv) => flat.compose(inv),
            None => flat,
        };
        let child = self.child.as_mut().unwrap();
        // Маски детей едут за сплющенной матрицей (как на плоском пути,
        // `Window::with_transformation_masked`); косая — прежнее поведение.
        paint_child(child, Some(flat), true, window, cx);
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
/// градиент колонки на чужие дорожки; маски краски — по всем. Третье поле —
/// тот же прямоугольник без округления к точке устройства: от него
/// считается область позиционирования (CSS 2.1 §17.5.1, Blink — по
/// неокруглённой геометрии ячеек), маски остаются округлёнными.
pub type RowRects = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, bool, Bounds<Pixels>)>>>;

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
    BAND_RETRIES.with(|m| m.borrow_mut().clear());
    forget_clamp_buffers();
}

thread_local! {
    /// Сколько кадров полоса фона прождала своих проб. Ключ — адрес буфера
    /// проб.
    ///
    /// Полоса без единой ячейки (`<col>` без рядов, `<col>` за краем сетки)
    /// не дождётся их никогда, а запрос кадра без счётчика вертел бы окно
    /// вечно: документ не успокаивается, и стенд снимает его на таймауте.
    static BAND_RETRIES: std::cell::RefCell<std::collections::HashMap<usize, u8>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько кадров ждать пробы, прежде чем счесть полосу пустой.
const BAND_WAIT_FRAMES: u8 = 2;

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
        // Пробы ячеек пишут в PREPAINT, а вся подготовка кадра идёт до
        // отрисовки — здесь забираются прямоугольники ЭТОГО ЖЕ кадра.
        // Пустота возможна только на самом первом кадре документа.
        let rects = std::mem::take(&mut *self.rects.borrow_mut());
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("HTML_ROWBG").is_ok());
            *ON
        } {
            eprintln!("ROWBG paint: rects={} {:?}", rects.len(), rects);
        }
        if rects.is_empty() {
            // Пробы ячеек ещё не писали (первый кадр) — без нового кадра
            // окно не перерисуется, и фон не появится никогда. Но ждать
            // бесконечно нельзя: у полосы может не быть ячеек вовсе.
            let key = std::rc::Rc::as_ptr(&self.rects) as usize;
            let waited = BAND_RETRIES.with(|m| {
                let mut m = m.borrow_mut();
                let n = m.entry(key).or_insert(0);
                *n = n.saturating_add(1);
                *n
            });
            if waited <= BAND_WAIT_FRAMES {
                window.request_animation_frame();
            }
            return;
        }
        BAND_RETRIES.with(|m| {
            m.borrow_mut()
                .remove(&(std::rc::Rc::as_ptr(&self.rects) as usize));
        });
        // Область ряда/колонки — охват ТОЧНЫХ ячеек (span = 1): от неё
        // считается и размер плитки, и `background-position`. Объединённые
        // лежат и на чужих дорожках — они только маски.
        let union = |pick: &dyn Fn(&(Bounds<Pixels>, bool, Bounds<Pixels>)) -> Bounds<Pixels>| {
            let exact: Vec<Bounds<Pixels>> = rects.iter().filter(|r| r.1).map(pick).collect();
            let all: Vec<Bounds<Pixels>> = rects.iter().map(pick).collect();
            let base = if exact.is_empty() { all } else { exact };
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
            area
        };
        // Тень и обводка — по округлённым ячейкам (резкие края); фон
        // позиционируется по неокруглённым (CSS 2.1 §17.5.1, как у Blink).
        let area = union(&|r| r.0);
        let positioning = union(&|r| r.2);
        let all: Vec<Bounds<Pixels>> = rects.iter().map(|(b, _, _)| *b).collect();
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
                origin: gpui::point(
                    area.origin.x + gpui::px(sh.x),
                    area.origin.y + gpui::px(sh.y),
                ),
                size: area.size,
            };
            if sh.blur > 0.0 {
                // Коробка — сам охват, смещение — в тени: примитив вырезает
                // тень под СВОЕЙ коробкой (патч gpui `Shadow::box_bounds`),
                // и сдвинутый охват вырезал бы не то место.
                window.paint_drop_shadows(
                    area,
                    gpui::Corners::default(),
                    &[gpui::BoxShadow {
                        color: colour.to_hsla(),
                        offset: gpui::point(gpui::px(sh.x), gpui::px(sh.y)),
                        // σ = половина радиуса CSS (как в `apply::apply_paint`).
                        blur_radius: gpui::px(sh.blur * 0.5),
                        spread_radius: gpui::px(sh.spread),
                        inset: false,
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
        // Обводка ряда/группы — вокруг охвата ТОЧНЫХ ячеек, снаружи и без
        // маски (css-ui-4 §outline: рамка вне коробки, раскладку не трогает).
        // Тот же кольцевой квад, что у резкой тени выше.
        if let Some(o) = &self.style.outline {
            let em = match self.style.font_size {
                Some(crate::value::Len::Px(v)) => v,
                _ => 16.0,
            };
            let px_of = |l: Option<crate::value::Len>| match l {
                Some(crate::value::Len::Px(v)) => v,
                Some(crate::value::Len::Em(k)) => k * em,
                _ => 0.0,
            };
            let w = px_of(o.width);
            let out = px_of(o.offset) + w;
            if let (true, true, Some(colour)) =
                (o.style != Some(0), w > 0.0, o.color.or(self.style.color))
            {
                let ring = Bounds {
                    origin: gpui::point(area.origin.x - gpui::px(out), area.origin.y - gpui::px(out)),
                    size: gpui::size(
                        area.size.width + gpui::px(2.0 * out),
                        area.size.height + gpui::px(2.0 * out),
                    ),
                };
                let mut quad = gpui::fill(ring, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px(w),
                    right: gpui::px(w),
                    bottom: gpui::px(w),
                    left: gpui::px(w),
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
                crate::background::paint_area(&self.style, positioning, window);
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
    /// Ранг источника (CSS 2.1 §17.6.2.1 п.4), больше — сильнее: таблица 0,
    /// группа колонок 1, колонка 2, группа рядов 3, ряд 4, ячейка 5.
    pub source: u8,
    /// Порядок в документе: раньше = выше/левее, при равенстве побеждает.
    pub doc_ix: u32,
}

pub type CellEdges = std::rc::Rc<std::cell::RefCell<Vec<EdgeCell>>>;

/// Прямоугольники элементов сетки/гибкого контейнера: их собирают пробы
/// детей, а по ним слой-художник считает середины промежутков
/// (css-gaps-1 §geometry: линейка идёт по ЦЕНТРАЛЬНОЙ ЛИНИИ промежутка).
pub type GapItems = std::rc::Rc<std::cell::RefCell<Vec<Bounds<Pixels>>>>;

thread_local! {
    static GAP_ITEMS: std::cell::RefCell<std::collections::HashMap<u64, GapItems>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Стек контейнеров с линейками промежутков при ПОСТРОЕНИИ дерева.
    /// Ровно как `CLAMP_STACK`: проба ставится только НЕПОСРЕДСТВЕННЫМ
    /// детям, поэтому сторож кладёт ключ на время сборки детей.
    static GAP_STACK: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(Vec::new());
}

pub fn gap_items_for(key: u64) -> GapItems {
    GAP_ITEMS.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

pub fn forget_gap_buffers() {
    GAP_ITEMS.with(|m| m.borrow_mut().clear());
    GAP_STACK.with(|st| st.borrow_mut().clear());
}

/// Сторож стека линеек на время сборки ДЕТЕЙ контейнера.
pub struct GapGuard(bool);

impl GapGuard {
    pub fn enter(key: u64) -> Self {
        GAP_STACK.with(|st| st.borrow_mut().push(key));
        GapGuard(true)
    }
}

impl Drop for GapGuard {
    fn drop(&mut self) {
        if self.0 {
            GAP_STACK.with(|st| {
                st.borrow_mut().pop();
            });
        }
    }
}

/// Ключ контейнера линеек, в котором строится текущий элемент.
pub fn gap_context() -> Option<u64> {
    GAP_STACK.with(|st| st.borrow().last().copied())
}

/// Проба элемента сетки: как `edge_probe`, но пишет только границы.
///
/// Абсолютная проба `top: 0; left: 0; size: 100%` ложится на ПАДДИНГ-бокс
/// элемента (содержащий блок абсолютного потомка — padding box, CSS 2.1
/// §10.1), а геометрия промежутков строится по РАМОЧНЫМ коробкам элементов
/// (css-gaps-1 §gap-grid/§gap-flex: промежуток — между краями элементов,
/// Blink `GapGeometry` берёт border-box фрагментов). `border` — толщины
/// рамки элемента [top, right, bottom, left]: на них проба расширяется
/// (`grid-gap-decorations-008`: элементы с `border: 1px`, линейки вставали
/// на 1px внутрь от первой и последней линии сетки).
pub fn gap_item_probe(items: GapItems, border: [f32; 4]) -> AnyElement {
    GapItemProbe { items, border }.into_any_element()
}

/// Элемент пробы: границы берутся БЕЗ округления к точке устройства.
/// Линейка ставится по середине промежутка между элементами, а эталон
/// кладёт её абсолютной коробкой от точного начала (`top: 64.17px`) —
/// от округлённых краёв элементов середина уезжает на долю точки, и при
/// масштабе 1.25 край линейки округлялся на строку ниже
/// (`flex-gap-decorations-048`: строка y=86 лишняя).
struct GapItemProbe {
    items: GapItems,
    border: [f32; 4],
}

impl Element for GapItemProbe {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let origin = window.layout_origin_unrounded(*state);
        let size = window.layout_size_unrounded(*state);
        let [t, r, b, l] = self.border;
        self.items.borrow_mut().push(Bounds {
            origin: gpui::point(origin.x - px(l), origin.y - px(t)),
            size: gpui::size(size.width + px(l + r), size.height + px(t + b)),
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for GapItemProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Правила линеек одной оси (css-gaps-1), уже в точках.
#[derive(Clone, Debug)]
pub struct GapAxisRule {
    /// Ширина, цвет и видимость стиля — по промежуткам (§lists).
    pub widths: crate::computed::GapList<f32>,
    pub colors: crate::computed::GapList<crate::value::Color>,
    pub styles: crate::computed::GapList<bool>,
    /// §break: 0 `none`, 1 `normal`, 2 `intersection`.
    pub brk: u8,
    /// §inset: [cap-start, cap-end, junction-start, junction-end].
    pub inset: [crate::computed::GapInset; 4],
    /// §visibility-items: 0 `normal`, 1 `all`, 2 `around`, 3 `between`.
    pub visibility: u8,
    /// `double` style: two lines of a third of the width each, the rest a
    /// gap (as the `double` border, Blink `GetDoubleBorderStripeWidths`).
    pub double: bool,
}

/// Устройство контейнера для геометрии промежутков.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GapLayout {
    /// Решётка: дорожки общие для всех элементов, спаны перекрывают промежутки.
    Grid,
    /// Строки гибкого контейнера или ленты: у каждой строки СВОИ промежутки
    /// между элементами. `stacked_vertically` — строки уложены сверху вниз
    /// (гибкая строка, ленты-ряды), иначе слева направо (гибкая колонка,
    /// ленты-колонки).
    Lines { stacked_vertically: bool },
}

/// Что и как рисовать в промежутках контейнера.
#[derive(Clone, Debug)]
pub struct GapRuleSpec {
    pub col: Option<GapAxisRule>,
    pub row: Option<GapAxisRule>,
    pub kind: GapLayout,
    /// Вертикальное письмо: промежутки колонок — горизонтальные полосы.
    pub vertical: bool,
    /// `rule-overlap: column-over-row`.
    pub column_over_row: bool,
    /// Зазоры в точках между x-дорожками и между y-дорожками, если известны.
    pub gap_x: Option<f32>,
    pub gap_y: Option<f32>,
    /// Размеры дорожек ШАБЛОНА в точках вдоль x и вдоль y, если весь список
    /// точечный. css-gaps-1 §gap-grid: «Row gap and column gap, in the
    /// context of a grid container, refer to the gutters between grid rows
    /// and grid columns» — промежуток задан ДОРОЖКАМИ, а не коробками
    /// элементов, и пустая дорожка остаётся дорожкой. Blink строит ту же
    /// геометрию из коллекции дорожек (`grid_layout_utils.cc`
    /// `BuildGridTrackGapData`: `LayoutGrid::ComputeExpandedPositions`), а
    /// элементы дают только занятость клетки по ИНДЕКСУ дорожки.
    pub tracks_x: Option<Vec<f32>>,
    pub tracks_y: Option<Vec<f32>>,
    /// `direction: rtl` контейнера: втяжки `*-inset-start/end` вдоль
    /// строчной оси считаются от правого края (css-gaps-1 §insets-start-end;
    /// `multicol-gap-decorations-direction-inset`, вторая половина).
    pub rtl: bool,
    /// Промежутки по x (по y) нумеруются справа налево (снизу вверх):
    /// значения списков назначаются в ЛОГИЧЕСКОМ порядке оси (css-gaps-1
    /// §assigning; эталоны `*-multi-value-direction`/`-writing-mode`).
    pub rev_x: bool,
    pub rev_y: bool,
    /// Паддинг контейнера [top, right, bottom, left] в точках: поле
    /// содержимого = паддинг-бокс художника минус он.
    pub pad: [f32; 4],
    /// Протяжённость главных промежутков строк (`GapLayout::Lines`): 0 — по
    /// элементам (многоколонник), 1 — гибкий контейнер, 2 — ленты. У гибкого
    /// главный промежуток идёт от начала поля содержимого (или первого
    /// элемента, если он левее) до конца поля содержимого (или центра
    /// последнего поперечного промежутка) — Blink `FlexGapAccumulator`
    /// (`SetContentStartOffsetsIfNeeded`, `content_main_end_ =
    /// container_main_end`); у лент — через всё поле содержимого по оси
    /// укладки («MainGaps span the final container content box (or the content
    /// when it overflows)», `grid_lanes_layout_algorithm.cc`).
    pub lines_extent: u8,
    /// Ленты с явным выравниванием содержимого по оси укладки
    /// (`align-content` у колоночных лент, `justify-content` у строчных —
    /// не `normal`): главные промежутки идут лишь по выровненному
    /// содержимому, без свободного места (Blink
    /// `grid_lanes_layout_algorithm.cc`: «Explicit content alignment limits
    /// gap decoration rule bounds to the aligned content instead of including
    /// free space», `explicit_content_bounds`).
    pub lanes_content_aligned: bool,
}

/// Допуск сравнения координат раскладки.
const GAP_EPS: f32 = 0.35;

/// Элемент в осях `a` — поперёк промежутка, `b` — вдоль линейки.
#[derive(Clone, Copy, Debug)]
struct GapItem {
    a0: f32,
    a1: f32,
    b0: f32,
    b1: f32,
}

impl GapItem {
    fn from_bounds(b: &Bounds<Pixels>, gap_on_x: bool) -> Self {
        let x0 = f32::from(b.origin.x);
        let y0 = f32::from(b.origin.y);
        let x1 = x0 + f32::from(b.size.width);
        let y1 = y0 + f32::from(b.size.height);
        if gap_on_x {
            GapItem { a0: x0, a1: x1, b0: y0, b1: y1 }
        } else {
            GapItem { a0: y0, a1: y1, b0: x0, b1: x1 }
        }
    }

    fn flipped(&self) -> Self {
        GapItem { a0: self.b0, a1: self.b1, b0: self.a0, b1: self.a1 }
    }

    /// Заходит ли элемент в участок `[lo, hi]` вдоль линейки.
    fn covers_b(&self, lo: f32, hi: f32) -> bool {
        self.b0 < hi - GAP_EPS && self.b1 > lo + GAP_EPS
    }

    /// Перекрывает ли элемент промежуток `[g0, g1]` (спан через него).
    fn spans_a(&self, g0: f32, g1: f32) -> bool {
        self.a0 <= g0 + GAP_EPS && self.a1 >= g1 - GAP_EPS
    }
}

/// Пересекающий зазор на пути линейки: интервал вдоль `b`; рвёт ли он
/// линейку при `intersection` (видимое пересечение); есть ли в нём поперечная
/// линейка (стык, а не cap) и её ширина.
#[derive(Clone, Copy, Debug)]
struct Crossing {
    lo: f32,
    hi: f32,
    breaks: bool,
    joins: bool,
    cross_w: f32,
}

/// Линейка одного промежутка: интервал промежутка `[g0, g1]`, протяжённость
/// `[r0, r1]`, пересечения, перекрытия спанами, скрытые по visibility участки
/// и характер концов протяжённости — стык (ширина зазора, есть ли линейка,
/// её ширина) или край контейнера (`None`).
#[derive(Clone, Debug)]
struct GapRun {
    g0: f32,
    g1: f32,
    r0: f32,
    r1: f32,
    crossings: Vec<Crossing>,
    blocked: Vec<(f32, f32)>,
    hidden: Vec<(f32, f32)>,
    start_edge: Option<(f32, bool, f32)>,
    end_edge: Option<(f32, bool, f32)>,
    index: usize,
    count: usize,
}

impl GapRun {
    /// Конец отрезка: положение после отступа из пересекающего зазора к его
    /// границе, ширина зазора (0 у края и у «висячего» конца без поперечной
    /// линейки — так считает Blink `GetMaxInsetWidth`), есть ли стык и ширина
    /// поперечной линейки.
    fn edge(&self, pos: f32, is_start: bool) -> (f32, f32, bool, f32) {
        if is_start && (pos - self.r0).abs() <= GAP_EPS {
            return match self.start_edge {
                Some((cw, joins, dw)) => (self.r0, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r0, 0.0, false, 0.0),
            };
        }
        if !is_start && (pos - self.r1).abs() <= GAP_EPS {
            return match self.end_edge {
                Some((cw, joins, dw)) => (self.r1, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r1, 0.0, false, 0.0),
            };
        }
        if let Some(c) = self
            .crossings
            .iter()
            .find(|c| c.lo - GAP_EPS <= pos && pos <= c.hi + GAP_EPS)
        {
            let at = if is_start { c.hi } else { c.lo };
            return (at, if c.joins { c.hi - c.lo } else { 0.0 }, c.joins, c.cross_w);
        }
        (pos, 0.0, false, 0.0)
    }
}

fn uniq_sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|x, y| (*x - *y).abs() <= GAP_EPS);
    v
}

/// Дорожки по оси `a`: начало — уникальные ближние края элементов, конец —
/// дальний край элемента, начатого в дорожке и не заходящего в следующую;
/// когда такого нет (все — спаны), начало следующей минус зазор.
fn tracks_a(items: &[GapItem], gap: Option<f32>) -> Vec<(f32, f32)> {
    let st = uniq_sorted(items.iter().map(|i| i.a0).collect());
    (0..st.len())
        .map(|k| {
            let next = st.get(k + 1).copied();
            let end = items
                .iter()
                .filter(|i| {
                    (i.a0 - st[k]).abs() <= GAP_EPS && next.is_none_or(|nx| i.a1 <= nx + GAP_EPS)
                })
                .map(|i| i.a1)
                .fold(f32::NEG_INFINITY, f32::max);
            let end = if end.is_finite() {
                end
            } else {
                match (next, gap) {
                    (Some(nx), Some(g)) => nx - g,
                    (Some(nx), None) => nx,
                    (None, _) => st[k],
                }
            };
            (st[k], end)
        })
        .collect()
}

/// Дорожки по ШАБЛОНУ контейнера, привязанные к наблюдённым краям элементов.
///
/// `tracks_a` знает только границы коробок, поэтому дорожка без элементов
/// пропадает: два промежутка вокруг неё сливаются в один широкий, а
/// `§visibility-items: between/around` не может спрятать линейку над пустой
/// областью — хотя спека написана именно про неё («whether a gap decoration
/// segment is painted in portions of gaps adjacent to empty areas»,
/// css-gaps-1 §visibility-items).
///
/// Шаблон даёт РАЗМЕРЫ дорожек, но не их начало на экране; начало ищется
/// перебором: первый наблюдённый край элемента — это начало какой-то из
/// дорожек. Годной считается только та привязка, при которой КАЖДЫЙ элемент
/// стоит краями на линиях сетки. Проверка отсекает поля и выравнивание
/// элемента внутри дорожки, неявные дорожки авто-размещения и
/// нерасшифрованные доли `fr`: там возвращается `None` и работает прежний
/// счёт по элементам. Элемент-спан проверку проходит: его начало — начало
/// первой дорожки пролёта, конец — конец последней.
///
/// Ограничение: сетка, у которой пуста ВСЯ первая дорожка оси, привяжется со
/// сдвигом на дорожку — перебор идёт от нулевого смещения. В своде такой пары
/// нет (у всех 44 разрежённых первая строка и первая колонка заняты).
fn template_tracks(
    sizes: &[f32],
    gap: Option<f32>,
    items: &[GapItem],
) -> Option<Vec<(f32, f32)>> {
    let g = gap?;
    if sizes.len() < 2 {
        return None;
    }
    let mut off = Vec::with_capacity(sizes.len());
    let mut y = 0.0f32;
    for w in sizes {
        off.push(y);
        y += w + g;
    }
    let first = uniq_sorted(items.iter().map(|i| i.a0).collect())
        .first()
        .copied()?;
    for shift in &off {
        let base = first - shift;
        let out: Vec<(f32, f32)> = sizes
            .iter()
            .zip(&off)
            .map(|(w, o)| (base + o, base + o + w))
            .collect();
        let fits = items.iter().all(|i| {
            out.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && out.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        });
        if fits {
            return Some(out);
        }
    }
    None
}

/// Дорожки сетки по x и по y в координатах окна.
type GridTracks = (Vec<(f32, f32)>, Vec<(f32, f32)>);

/// Дорожки без схлопнутых: схлопнутая (`auto-fit` без элементов, css-grid-1
/// §7.2.3.2 «collapsed grid track… the gutters on either side of it…
/// collapse») приходит из раскладки дорожкой нулевого размера, прижатой к
/// соседу без зазора. При ненулевом `gap` такая дорожка — не дорожка и
/// промежутков не даёт (Blink `CollapsedTrackIndexes`).
fn uncollapsed(mut tracks: Vec<(f32, f32)>, gap: f32) -> Vec<(f32, f32)> {
    // При `rtl` и обратных осях раскладка отдаёт дорожки в логическом
    // порядке — здесь нужен физический.
    tracks.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    if gap <= GAP_EPS {
        return tracks;
    }
    let n = tracks.len();
    (0..n)
        .filter(|&k| {
            let (s, e) = tracks[k];
            let empty = e - s <= GAP_EPS;
            let flush_prev = k > 0 && (s - tracks[k - 1].1).abs() <= GAP_EPS;
            let flush_next = k + 1 < n && (tracks[k + 1].0 - e).abs() <= GAP_EPS;
            !(empty && (flush_prev || flush_next))
        })
        .map(|k| tracks[k])
        .collect()
}

/// Промежутки между соседними дорожками; нулевой зазор — тоже промежуток
/// (`flex-gap-decorations-033`).
fn gaps_of(tracks: &[(f32, f32)]) -> Vec<(f32, f32)> {
    tracks
        .windows(2)
        .filter(|w| w[1].0 - w[0].1 >= -GAP_EPS)
        .map(|w| (w[0].1.min(w[1].0), w[1].0))
        .collect()
}

fn merge(mut v: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    v.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo <= last.1 + GAP_EPS => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

fn subtract(parts: Vec<(f32, f32)>, (lo, hi): (f32, f32)) -> Vec<(f32, f32)> {
    let mut out = vec![];
    for (s, e) in parts {
        if hi <= s + GAP_EPS || lo >= e - GAP_EPS {
            out.push((s, e));
            continue;
        }
        if lo > s + GAP_EPS {
            out.push((s, lo));
        }
        if hi < e - GAP_EPS {
            out.push((hi, e));
        }
    }
    out
}

/// §visibility-items: заняты ли области по сторонам промежутка `[g0, g1]` в
/// пределах участка `[lo, hi]` вдоль линейки. Спан через промежуток занимает
/// обе стороны.
fn occupied(items: &[GapItem], g0: f32, g1: f32, lo: f32, hi: f32, visibility: u8) -> bool {
    if visibility < 2 {
        return true;
    }
    let near = 2.0 * GAP_EPS;
    let before = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a1 - g0).abs() <= near || i.spans_a(g0, g1)));
    let after = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a0 - g1).abs() <= near || i.spans_a(g0, g1)));
    if visibility == 2 {
        before || after
    } else {
        before && after
    }
}

/// Решётка: линейки промежутков оси `a`. Пересекающие зазоры — промежутки
/// оси `b`; пересечение видимо (`breaks`), если хотя бы с одной стороны
/// поперечный зазор не перекрыт спаном (Blink: `kIntersection` идёт дальше
/// только при blocked-before И blocked-after); стык (`joins`) — если там есть
/// видимая поперечная линейка.
fn grid_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    gap_b: Option<f32>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
    tpl_a: Option<&[f32]>,
    tpl_b: Option<&[f32]>,
    abs: Option<(&[(f32, f32)], &[(f32, f32)])>,
) -> Vec<GapRun> {
    let flipped: Vec<GapItem> = items.iter().map(GapItem::flipped).collect();
    // Дорожки раскладки сильнее всего: это и есть коллекция дорожек сетки
    // (Blink `BuildGridTrackGapData`), с пустыми и схлопнутыми дорожками.
    // Годны, только если каждый элемент стоит краями на линиях дорожек: при
    // `rtl` и вертикальном письме раскладка отдаёт позиции в своей системе
    // отсчёта, и тогда остаётся прежний счёт по элементам и шаблону.
    let on_lines = |tracks: &[(f32, f32)], items: &[GapItem]| {
        items.iter().all(|i| {
            tracks.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && tracks.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        })
    };
    if let Some((ta, tb)) = abs
        && !ta.is_empty()
        && !tb.is_empty()
        && on_lines(ta, items)
        && on_lines(tb, &flipped)
    {
        return grid_runs_on(items, &flipped, ta.to_vec(), tb.to_vec(), rule, cross);
    }
    // Дорожки шаблона сильнее выведенных из коробок (css-gaps-1 §gap-grid;
    // Blink `BuildGridTrackGapData` строит геометрию из коллекции дорожек).
    // Привязка не сошлась — остаётся прежний счёт по элементам, картинка не
    // меняется.
    let ta = tpl_a
        .and_then(|t| template_tracks(t, gap_a, items))
        .unwrap_or_else(|| tracks_a(items, gap_a));
    let tb = tpl_b
        .and_then(|t| template_tracks(t, gap_b, &flipped))
        .unwrap_or_else(|| tracks_a(&flipped, gap_b));
    grid_runs_on(items, &flipped, ta, tb, rule, cross)
}

fn grid_runs_on(
    items: &[GapItem],
    flipped: &[GapItem],
    ta: Vec<(f32, f32)>,
    tb: Vec<(f32, f32)>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
) -> Vec<GapRun> {
    let ga = gaps_of(&ta);
    let gb = gaps_of(&tb);
    let r0 = tb.first().map_or(0.0, |t| t.0);
    let r1 = tb.last().map_or(0.0, |t| t.1);
    let n = ga.len();
    ga.iter()
        .enumerate()
        .map(|(k, &(g0, g1))| {
            let blocked = merge(
                items
                    .iter()
                    .filter(|i| i.spans_a(g0, g1))
                    .map(|i| (i.b0, i.b1))
                    .collect(),
            );
            let hidden = tb
                .iter()
                .copied()
                .filter(|&(lo, hi)| !occupied(items, g0, g1, lo, hi, rule.visibility))
                .collect();
            let sides = [ta.get(k).copied(), ta.get(k + 1).copied()];
            let crossings = gb
                .iter()
                .enumerate()
                .map(|(j, &(lo, hi))| {
                    let mut breaks = false;
                    let mut joins = false;
                    for side in sides.iter().flatten() {
                        let blocked_here = flipped.iter().any(|i| {
                            i.spans_a(lo, hi) && i.covers_b(side.0, side.1)
                        });
                        if !blocked_here {
                            breaks = true;
                        }
                        if let Some(c) = cross
                            && !blocked_here
                            && c.styles.at(j, gb.len()).unwrap_or(false)
                            && occupied(flipped, lo, hi, side.0, side.1, c.visibility)
                        {
                            joins = true;
                        }
                    }
                    let cross_w = cross.and_then(|c| c.widths.at(j, gb.len())).unwrap_or(0.0);
                    Crossing { lo, hi, breaks, joins, cross_w }
                })
                .collect();
            GapRun {
                g0,
                g1,
                r0,
                r1,
                crossings,
                blocked,
                hidden,
                start_edge: None,
                end_edge: None,
                index: k,
                count: n,
            }
        })
        .collect()
}

/// Строки гибкого контейнера по оси `a`: пересекающиеся протяжённости
/// элементов сливаются в одну строку.
fn line_groups(items: &[GapItem]) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = items.iter().map(|i| (i.a0, i.a1)).collect();
    v.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo < last.1 - GAP_EPS || (lo - last.0).abs() <= GAP_EPS => {
                last.1 = last.1.max(hi)
            }
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// Строки/ленты: `a` — ось укладки строк, `b` — ось элементов строки. Главные
/// промежутки — между строками, их пересекающие зазоры — ОБЪЕДИНЕНИЕ зазоров
/// соседних строк (окна перекрытия Blink); поперечные — между соседними
/// элементами строки, протяжённостью в пределах строки, со стыками на её
/// краях. Значения списков: главные — по строкам, поперечные — сквозной счёт.
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v98, `scout-columnwrap-2026-09b.md` I1):
/// считать главный промежуток УЖЕ `gap` не зазором, а стык без промежутка —
/// не junction (под ряды многоколонника). css-gaps 349: +12/−19 —
/// `flex-gap-decorations-001/019` 99.00, `-025/031/032/035/065…067`,
/// `column-gap-decorations-001/003/014/016/019`, `row-gap-decorations-003/010`.
/// Ряды многоколонника обходятся без него (v99: +11/−0).
fn line_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    main: Option<&GapAxisRule>,
    cross: Option<&GapAxisRule>,
    rev_cross: bool,
    extent: Option<(u8, f32, f32)>,
    content_aligned: bool,
    lanes: Option<(Vec<(f32, f32)>, f32)>,
) -> (Vec<GapRun>, Vec<GapRun>) {
    // Гибкие строки: строка — объединение поперечных протяжённостей её
    // элементов (Blink: `line_cross_start/end` строки, а не начало каждого
    // элемента), иначе при `align-items: flex-end` элементы разной высоты
    // разбегались по разным «строкам» (`flex-gap-decorations-007`).
    let flex = matches!(extent, Some((1, _, _)));
    // Ленты: полосы — дорожки оси решётки из раскладки; элемент входит в
    // каждую ленту, которую покрывает (элемент во несколько лент — запись в
    // каждой, Blink `GridLanesGapAccumulator::BuildCrossGaps`), а поперечный
    // промежуток стоит сразу перед началом следующей записи ленты: центр —
    // `ForwardStackingStart() - stacking_gap / 2` (`FinalGutterCenter`).
    let lane_gap = lanes.as_ref().map(|(_, g)| *g);
    let lines = match &lanes {
        Some((t, _)) => t.clone(),
        None if flex => line_groups(items),
        None => tracks_a(items, gap_a),
    };
    let mut r0 = items.iter().map(|i| i.b0).fold(f32::INFINITY, f32::min);
    let mut r1 = items.iter().map(|i| i.b1).fold(f32::NEG_INFINITY, f32::max);
    let inner: Vec<Vec<(f32, f32)>> = lines
        .iter()
        .map(|&(s, e)| {
            let mut row: Vec<&GapItem> = items
                .iter()
                .filter(|i| {
                    if lane_gap.is_some() {
                        i.a0 < e - GAP_EPS && i.a1 > s + GAP_EPS
                    } else if flex {
                        i.a0 >= s - GAP_EPS && i.a0 <= e + GAP_EPS
                    } else {
                        (i.a0 - s).abs() <= GAP_EPS
                    }
                })
                .collect();
            row.sort_by(|x, y| x.b0.partial_cmp(&y.b0).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(g) = lane_gap {
                return row.windows(2).map(|w| (w[1].b0 - g, w[1].b0)).collect();
            }
            row.windows(2)
                .filter(|w| w[1].b0 - w[0].b1 >= -GAP_EPS)
                .map(|w| (w[0].b1.min(w[1].b0), w[1].b0))
                .collect()
        })
        .collect();
    match extent {
        Some((1, c0, c1)) => {
            let last_cross = inner
                .last()
                .and_then(|v| v.last())
                .map(|&(lo, hi)| (lo + hi) / 2.0);
            r0 = r0.min(c0);
            r1 = last_cross.map_or(c1, |x| x.max(c1));
        }
        Some((2, _, _)) if content_aligned => {}
        Some((2, c0, c1)) => {
            r0 = c0;
            r1 = r1.max(c1);
        }
        _ => {}
    }
    let cross_total: usize = inner.iter().map(Vec::len).sum();
    let main_count = lines.len().saturating_sub(1);
    let main_w = main.and_then(|m| m.widths.first()).unwrap_or(0.0);
    let cross_w = cross.and_then(|c| c.widths.first()).unwrap_or(0.0);
    let mut mains = vec![];
    for k in 0..main_count {
        let (g0, g1) = (lines[k].1, lines[k + 1].0);
        if g1 - g0 < -GAP_EPS {
            continue;
        }
        let g0 = g0.min(g1);
        let blocked = merge(
            items
                .iter()
                .filter(|i| i.spans_a(g0, g1))
                .map(|i| (i.b0, i.b1))
                .collect(),
        );
        let windows = merge(inner[k].iter().chain(inner[k + 1].iter()).copied().collect());
        let crossings = windows
            .iter()
            .map(|&(lo, hi)| Crossing {
                lo,
                hi,
                breaks: true,
                joins: cross.is_some(),
                cross_w,
            })
            .collect();
        mains.push(GapRun {
            g0,
            g1,
            r0,
            r1,
            crossings,
            blocked,
            hidden: vec![],
            start_edge: None,
            end_edge: None,
            index: k,
            count: main_count,
        });
    }
    let mut crosses = vec![];
    let mut ix = 0usize;
    for (k, &(s, e)) in lines.iter().enumerate() {
        let before = (k > 0).then(|| (s - lines[k - 1].1, main.is_some(), main_w));
        let after = (k + 1 < lines.len()).then(|| (lines[k + 1].0 - e, main.is_some(), main_w));
        // Счёт сквозной по строкам (§assigning: «does not restart at the
        // beginning of each flex line»), внутри строки — от её логического
        // начала: при `rev_cross` крайний правый (нижний) промежуток первый.
        let n = inner[k].len();
        for (j, &(lo, hi)) in inner[k].iter().enumerate() {
            crosses.push(GapRun {
                g0: lo,
                g1: hi,
                r0: s,
                r1: e,
                crossings: vec![],
                blocked: vec![],
                hidden: vec![],
                start_edge: before,
                end_edge: after,
                index: if rev_cross { ix + n - 1 - j } else { ix + j },
                count: cross_total.max(1),
            });
        }
        ix += n;
    }
    // Ленты: поперечный промежуток, к которому примыкает элемент во обе
    // соседние ленты, идёт сквозь главный промежуток между ними — тот у
    // этого пересечения перекрыт (Blink `MarkBlockedMainGapSegments`: отрезок
    // главного промежутка, по обе стороны которого одна и та же запись,
    // заблокирован). Такие поперечные прогоны соседних лент сливаются в один.
    if lane_gap.is_some() && lines.len() > 1 {
        let mut k = 0;
        while k < crosses.len() {
            let a = &crosses[k];
            let lane = lines.iter().position(|&(_, e)| (e - a.r1).abs() <= GAP_EPS);
            let joined = lane.filter(|&l| l + 1 < lines.len()).and_then(|l| {
                let (g0, g1) = (lines[l].1, lines[l + 1].0);
                let spanned = items.iter().any(|i| {
                    i.spans_a(g0, g1)
                        && ((i.b1 - a.g0).abs() <= GAP_EPS || (i.b0 - a.g1).abs() <= GAP_EPS)
                });
                if !spanned {
                    return None;
                }
                crosses.iter().position(|b| {
                    (b.r0 - lines[l + 1].0).abs() <= GAP_EPS
                        && (b.g0 - a.g0).abs() <= GAP_EPS
                        && (b.g1 - a.g1).abs() <= GAP_EPS
                })
            });
            if let Some(j) = joined {
                let b = crosses.remove(j);
                let a = &mut crosses[if j < k { k - 1 } else { k }];
                a.r1 = b.r1;
                a.end_edge = b.end_edge;
                // Слитый прогон может слиться и со следующей лентой.
                if j < k {
                    k -= 1;
                }
                continue;
            }
            k += 1;
        }
    }
    (mains, crosses)
}

/// Слой линеек промежутков. Забирает буфер проб в `paint` (к этому моменту
/// prepaint всех детей уже прошёл — так же работает `EdgePainter`), строит
/// геометрию промежутков по границам элементов и красит отрезки линеек
/// (css-gaps-1 §geometry, §break, §inset, §visibility-items, §lists).
pub struct GapRulePainter {
    items: GapItems,
    spec: GapRuleSpec,
}

impl GapRulePainter {
    pub fn new(items: GapItems, spec: GapRuleSpec) -> Self {
        GapRulePainter { items, spec }
    }
}

impl Element for GapRulePainter {
    type RequestLayoutState = LayoutId;
    type PrepaintState = (Bounds<Pixels>, Option<GridTracks>);

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
    ) -> (LayoutId, LayoutId) {
        // Художник занимает паддинг-бокс контейнера: от него считается поле
        // содержимого (протяжённость главных промежутков строк и лент).
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> (Bounds<Pixels>, Option<GridTracks>) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        let tracks = (self.spec.kind == GapLayout::Grid || self.spec.lines_extent == 2)
            .then(|| window.parent_grid_tracks(*state))
            .flatten()
            .map(|(o, cols, rows)| {
                let (ox, oy) = (f32::from(o.x), f32::from(o.y));
                let gx = self.spec.gap_x.unwrap_or(0.0);
                let gy = self.spec.gap_y.unwrap_or(0.0);
                (
                    uncollapsed(cols.iter().map(|&(a, b)| (ox + a, ox + b)).collect(), gx),
                    uncollapsed(rows.iter().map(|&(a, b)| (oy + a, oy + b)).collect(), gy),
                )
            });
        (bounds, tracks)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        prepaint: &mut (Bounds<Pixels>, Option<GridTracks>),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let bounds = prepaint.0;
        let grid_tracks = prepaint.1.take();
        let items = std::mem::take(&mut *self.items.borrow_mut());
        // A grid's gaps come from its track collection, not from its items
        // (css-gaps-1 §gap-grid; Blink `BuildGridTrackGapData`): an empty
        // grid or subgrid still has gaps to decorate
        // (`subgrid-gap-decorations-012/015/016/017`).
        if items.is_empty() && !(self.spec.kind == GapLayout::Grid && grid_tracks.is_some()) {
            return;
        }
        let spec = &self.spec;
        // Физические семейства: промежутки, лежащие по x (линейки
        // вертикальные), и по y. `column-rule` — колонки; в вертикальном
        // письме колонки идут по y.
        let (on_x, on_y) = if spec.vertical {
            (spec.row.as_ref(), spec.col.as_ref())
        } else {
            (spec.col.as_ref(), spec.row.as_ref())
        };
        // (промежуток по x?, линейка, правило, главный промежуток строк?)
        let mut layers: Vec<(bool, GapRun, &GapAxisRule, bool)> = vec![];
        // Ленты с элементом во несколько лент: такой элемент выравнивает
        // ленты по укладке (css-grid-3 §grid-lanes-placement), и картина
        // промежутков — решётка; модель строк его не представляет.
        let kind = match spec.kind {
            GapLayout::Lines { stacked_vertically } if spec.lines_extent == 2 => {
                let it: Vec<GapItem> = items
                    .iter()
                    .map(|b| GapItem::from_bounds(b, !stacked_vertically))
                    .collect();
                let starts = uniq_sorted(it.iter().map(|i| i.a0).collect());
                let spans = it
                    .iter()
                    .any(|i| starts.iter().any(|&s| s > i.a0 + GAP_EPS && s < i.a1 - GAP_EPS));
                // С дорожками раскладки ленты строятся по ним, и элемент во
                // несколько лент представим (запись в каждой ленте).
                if spans && grid_tracks.is_none() { GapLayout::Grid } else { spec.kind }
            }
            k => k,
        };
        match kind {
            GapLayout::Grid => {
                let ix: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, true)).collect();
                let iy: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, false)).collect();
                // Ось `a` прогона — та, ПОПЕРЁК которой лежит промежуток: у
                // линеек, стоящих в промежутках по x, дорожки `a` идут по x, а
                // поперечные `b` — по y; у линеек по y — наоборот.
                let (tx, ty) = (spec.tracks_x.as_deref(), spec.tracks_y.as_deref());
                // Дорожки раскладки: в вертикальном письме сетка уже
                // повёрнута в `apply.rs`, и колонки gpui — физические x.
                let abs_x = grid_tracks.as_ref().map(|(c, r)| (c.as_slice(), r.as_slice()));
                let abs_y = grid_tracks.as_ref().map(|(c, r)| (r.as_slice(), c.as_slice()));
                if let Some(r) = on_x {
                    for mut run in grid_runs(&ix, spec.gap_x, spec.gap_y, r, on_y, tx, ty, abs_x) {
                        if spec.rev_x {
                            run.index = run.count - 1 - run.index;
                        }
                        layers.push((true, run, r, false));
                    }
                }
                if let Some(r) = on_y {
                    for mut run in grid_runs(&iy, spec.gap_y, spec.gap_x, r, on_x, ty, tx, abs_y) {
                        if spec.rev_y {
                            run.index = run.count - 1 - run.index;
                        }
                        layers.push((false, run, r, false));
                    }
                }
            }
            GapLayout::Lines { stacked_vertically } => {
                // Ось укладки строк — `a`: главные промежутки лежат по ней.
                let it: Vec<GapItem> = items
                    .iter()
                    .map(|b| GapItem::from_bounds(b, !stacked_vertically))
                    .collect();
                let (main, cross) = if stacked_vertically { (on_y, on_x) } else { (on_x, on_y) };
                let gap_b = if stacked_vertically { spec.gap_x } else { spec.gap_y };
                let lane_tracks = (spec.lines_extent == 2)
                    .then(|| grid_tracks.as_ref())
                    .flatten()
                    .map(|(c, r)| {
                        let t = if stacked_vertically { r.clone() } else { c.clone() };
                        (t, gap_b.unwrap_or(0.0))
                    })
                    .filter(|(t, _)| !t.is_empty());
                let gap_a = if stacked_vertically { spec.gap_y } else { spec.gap_x };
                // Главные промежутки лежат по оси укладки строк, поперечные —
                // по оси элементов строки; каждая нумеруется от своего
                // логического начала.
                let (rev_main, rev_cross) = if stacked_vertically {
                    (spec.rev_y, spec.rev_x)
                } else {
                    (spec.rev_x, spec.rev_y)
                };
                let extent = (spec.lines_extent != 0).then(|| {
                    let [pt, pr, pb, pl] = spec.pad;
                    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                    let (x1, y1) = (
                        x0 + f32::from(bounds.size.width),
                        y0 + f32::from(bounds.size.height),
                    );
                    if stacked_vertically {
                        (spec.lines_extent, x0 + pl, x1 - pr)
                    } else {
                        (spec.lines_extent, y0 + pt, y1 - pb)
                    }
                });
                let (mains, crosses) = line_runs(
                    &it,
                    gap_a,
                    main,
                    cross,
                    rev_cross,
                    extent,
                    spec.lanes_content_aligned,
                    lane_tracks,
                );
                if let Some(r) = main {
                    for mut run in mains {
                        if rev_main {
                            run.index = run.count - 1 - run.index;
                        }
                        layers.push((!stacked_vertically, run, r, true));
                    }
                }
                if let Some(r) = cross {
                    for run in crosses {
                        layers.push((stacked_vertically, run, r, false));
                    }
                }
            }
        }
        // §overlap: по умолчанию ряды поверх колонок — колонки красятся первыми.
        let col_on_x = !spec.vertical;
        let first_on_x = if spec.column_over_row { !col_on_x } else { col_on_x };
        let draw = |window: &mut Window, gap_on_x: bool, run: &GapRun, rule: &GapAxisRule, main_like: bool| {
            if !rule.styles.at(run.index, run.count).unwrap_or(false) {
                return;
            }
            let w = rule.widths.at(run.index, run.count).unwrap_or(0.0);
            if w <= 0.0 {
                return;
            }
            let Some(colour) = rule.colors.at(run.index, run.count) else {
                return;
            };
            let c = (run.g0 + run.g1) / 2.0;
            if spec.kind == GapLayout::Grid && !spec.vertical && gap_on_x {
                gap_fragment_tail::paint(window, bounds, run, rule, colour.to_hsla());
            }
            // Отрезок вдоль строчной оси (горизонтальный в горизонтальном
            // письме) при `rtl` считает start/end от правого края.
            let flip = spec.rtl && !spec.vertical && !gap_on_x;
            for (s, e) in segments(run, rule, main_like, flip) {
                let rect = if gap_on_x {
                    Bounds {
                        origin: gpui::point(gpui::px(c - w / 2.0), gpui::px(s)),
                        size: gpui::size(gpui::px(w), gpui::px(e - s)),
                    }
                } else {
                    Bounds {
                        origin: gpui::point(gpui::px(s), gpui::px(c - w / 2.0)),
                        size: gpui::size(gpui::px(e - s), gpui::px(w)),
                    }
                };
                // Края — к точке устройства, как края коробок раскладки
                // (округление абсолютной координаты, `TaffyLayoutEngine::
                // layout_bounds`): иначе шейдер рисует долю точки, а эталон —
                // абсолютная коробка — ровную строку.
                let scale = window.scale_factor().max(0.01);
                let edge = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
                let (l, t) = (edge(rect.origin.x), edge(rect.origin.y));
                let (r, b) = (
                    edge(rect.origin.x + rect.size.width),
                    edge(rect.origin.y + rect.size.height),
                );
                let rect = Bounds {
                    origin: gpui::point(l, t),
                    size: gpui::size(r - l, b - t),
                };
                let third = (w / 3.0).round();
                if rule.double && third >= 1.0 {
                    // Two lines across the rule's width, each a third of it
                    // (rounded like the `double` border in `render.rs`).
                    let (a0, a1) = (c - w / 2.0, c + w / 2.0);
                    for (p0, p1) in [(a0, a0 + third), (a1 - third, a1)] {
                        let (q0, q1) = (edge(gpui::px(p0)), edge(gpui::px(p1)));
                        let band = if gap_on_x {
                            Bounds { origin: gpui::point(q0, t), size: gpui::size(q1 - q0, b - t) }
                        } else {
                            Bounds { origin: gpui::point(l, q0), size: gpui::size(r - l, q1 - q0) }
                        };
                        window.paint_quad(gpui::fill(band, colour.to_hsla()));
                    }
                    continue;
                }
                window.paint_quad(gpui::fill(rect, colour.to_hsla()));
            }
        };
        for pass in [true, false] {
            for (on_x, run, rule, main_like) in &layers {
                if (*on_x == first_on_x) == pass {
                    draw(window, *on_x, run, rule, *main_like);
                }
            }
        }
    }
}

impl IntoElement for GapRulePainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

thread_local! {
    static CELL_EDGES: std::cell::RefCell<std::collections::HashMap<u64, CellEdges>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn cell_edges_for(key: u64) -> CellEdges {
    CELL_EDGES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

/// Фоны ячеек сросшейся модели: прямоугольник и цвет, снятые пробой ячейки
/// на ПОДГОТОВКЕ кадра; красит их `CellBgPainter` — слой, лежащий в сетке
/// ПЕРЕД кромками и ячейками. Так фон ячейки оказывается под кромками, а
/// содержимое ячейки — над ними, как у Blink: сросшиеся кромки идут в фазе
/// `kDescendantBlockBackgroundsOnly` (`box_fragment_painter.cc:952-957`,
/// «Collapsed borders paint *after* children have painted their
/// backgrounds»), а строчное, плавающее и позиционированное содержимое
/// ячеек — в более поздних фазах, то есть поверх кромок.
pub type CellBgs = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, gpui::Hsla)>>>;

/// Проба фона ячейки: холст во всю коробку ячейки записывает её рамку и
/// цвет на подготовке кадра (та же механика, что `edge_probe`).
pub fn cell_bg_probe(bgs: CellBgs, colour: gpui::Hsla) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            bgs.borrow_mut().push((bounds, colour));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Слой фонов ячеек сросшейся таблицы (см. `CellBgs`).
pub struct CellBgPainter {
    bgs: CellBgs,
}

impl CellBgPainter {
    pub fn new(bgs: CellBgs) -> Self {
        CellBgPainter { bgs }
    }
}

impl Element for CellBgPainter {
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
        // Пробы пишут на подготовке, вся подготовка кадра идёт до отрисовки —
        // здесь прямоугольники ЭТОГО ЖЕ кадра.
        let bgs = std::mem::take(&mut *self.bgs.borrow_mut());
        for (bounds, colour) in bgs {
            window.paint_quad(gpui::fill(bounds, colour));
        }
    }
}

impl IntoElement for CellBgPainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Проба кромок: как проба фона, пишет в PREPAINT границы и рамки ячейки.
/// Метка «коробка СЕТКИ»: проба ничего не рисует, а сообщает слою кромок,
/// где кончаются дорожки. Граница сетки есть ВСЕГДА, даже когда рисующей
/// кромки у таблицы нет, и смешивать эти два понятия нельзя (замеры «крайние
/// линии только с `source == 0`» и «нулевая проба таблицы» — обе давали
/// +21/-25).
pub const GRID_BOX: u8 = u8::MAX;

/// Проба ГРАНИЦЫ СЕТКИ: координаты те же, что у пробы кромок таблицы
/// (`inset` — ширины её рамочного места), но без самих кромок.
pub fn grid_probe(edges: CellEdges, inset: [f32; 4]) -> AnyElement {
    edge_probe(
        edges,
        [0.0; 4],
        [crate::value::Color::default(); 4],
        [0; 4],
        GRID_BOX,
        0,
        inset,
    )
}

pub fn edge_probe(
    edges: CellEdges,
    widths: [f32; 4],
    colors: [crate::value::Color; 4],
    styles: [u8; 4],
    source: u8,
    doc_ix: u32,
    inset: [f32; 4],
) -> AnyElement {
    // Exact (unrounded) layout bounds: each collapsed border band is snapped
    // once, from the grid line it is centred on (CSS 2.1 §17.6.2). Bands
    // built from independently rounded cell edges left a device-pixel gap
    // between two bands whose exact edges coincide.
    gpui::canvas_with_unrounded_bounds(
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
            edges.borrow_mut().push(EdgeCell {
                bounds,
                widths,
                colors,
                styles,
                source,
                doc_ix,
            });
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

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-collapsedborders-2026-09.md`,
// 5 хунков): маска «do-not-fill» для ячейки с охватом — ячейка отдаёт
// слою кромок свой прямоугольник, и линию строго внутри него не
// заливают ни ряд, ни группа, ни колонка, ни стол (1:1 Blink
// `MarkInnerBordersAsDoNotFill`, `table_borders.cc:555`; набор S по
// css-tables-3 строится только из `table-cell`).
// Срез 246 пар семьи, база тем же списком: 228 -> 228, **+0 / −0**.
// Модель верна, но ни одной пары не двигает: конфликт §17.6.2.1
// разбирается ДВАЖДЫ — в раскладке (`render.rs: win_edges`, только
// ячейки и стол, простой `max()`) и здесь, по всем шести источникам.
// Смысл появится, когда обе модели сведут в одну, как `TableBorders`
// у Blink; порознь маска — мёртвый код.
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
        // Начало сетки по каждой оси — отдельно от того, кто эту линию красит.
        let grid_lo = cells
            .iter()
            .find(|c| c.source == GRID_BOX)
            .map(|c| (f32::from(c.bounds.origin.x), f32::from(c.bounds.origin.y)));
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
            if c.source == GRID_BOX {
                continue;
            }
            let bnd = c.bounds;
            let (x0, y0) = (f32::from(bnd.origin.x), f32::from(bnd.origin.y));
            let (x1, y1) = (
                x0 + f32::from(bnd.size.width),
                y0 + f32::from(bnd.size.height),
            );
            let is_table = c.source == 0;
            let mut side = |list: &mut Vec<Cand>, line: f32, a: f32, b: f32, i: usize, out: i8| {
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
        // Снимок вертикалей для стыков: горизонталь тянется в угол на
        // половину ВЕРТИКАЛЬНОЙ кромки, а не своей (§17.6.2: кромки
        // центрированы на линиях сетки, и ширина стыка задаётся
        // перпендикуляром). Своя полуширина рисовала ус там, где вертикали
        // нет вовсе: `border-top-width: 96px` вылезал на 48 точек за край.
        let vert_spans: Vec<(f32, f32, f32, f32, u8)> = vert
            .iter()
            .map(|c| (c.line, c.a, c.b, c.w, c.style))
            .collect();
        let half_at = |x: f32, y: f32| -> f32 {
            let mut widest = 0.0f32;
            for c in &vert_spans {
                if (c.0 - x).abs() >= 0.75 || y < c.1 - 0.25 || y > c.2 + 0.25 {
                    continue;
                }
                // Погашенная вертикаль не рисуется, значит и заливать под
                // неё угол нечем.
                if c.4 == 1 {
                    return 0.0;
                }
                widest = widest.max(c.3);
            }
            widest / 2.0
        };
        // Симметрично для вертикалей: в стык вертикаль тянется на половину
        // ГОРИЗОНТАЛЬНОЙ кромки.
        let horiz_spans: Vec<(f32, f32, f32, f32, u8)> = horiz
            .iter()
            .map(|c| (c.line, c.a, c.b, c.w, c.style))
            .collect();
        let half_at_h = |y: f32, x: f32| -> f32 {
            let mut widest = 0.0f32;
            for c in &horiz_spans {
                if (c.0 - y).abs() >= 0.75 || x < c.1 - 0.25 || x > c.2 + 0.25 {
                    continue;
                }
                if c.4 == 1 {
                    return 0.0;
                }
                widest = widest.max(c.3);
            }
            widest / 2.0
        };
        // Стык кромок решается ПРИОРИТЕТОМ, а не осью: прежде горизонтали
        // рисовались ПОСЛЕ вертикалей и, протянутые в углы, всегда накрывали
        // стык своим цветом. У Blink стык достаётся кромке, победившей в
        // разборе §17.6.2.1 (`table_painters.cc`, `CollapsedBorderPainter`:
        // края отрезка подрезаются/растягиваются по соседней перпендикулярной
        // кромке в зависимости от того, кто сильнее). Поэтому отрезки обеих
        // осей копятся с ключом победителя и красятся по возрастанию ключа —
        // сильнейшая кромка ложится последней и забирает угол
        // (`border-conflict-element-001e`: синяя вертикаль первой ячейки
        // против жёлтой горизонтали второй — в эталоне угол синий).
        // При равном ключе вертикаль идёт первой — прежний порядок.
        type SegKey = (f32, u8, u8, u32);
        let mut segs: Vec<(SegKey, bool, Bounds<Pixels>, crate::value::Color)> = Vec::new();
        let mut draw = |cands: &mut Vec<Cand>, vertical: bool, grid_lo: Option<f32>| {
            cands.sort_by(|p, q| {
                p.line
                    .partial_cmp(&q.line)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Крайние линии таблицы: кромка не центрируется, а рисуется
            // внутрь бокса (наружная половина у браузеров уходит в поля,
            // эталоны считают рамку частью коробки).
            // Начало сетки — не «первая нарисованная кромка», а край ДОРОЖЕК.
            // У браузера коробка таблицы раздаётся наружу на половину
            // победившей кромки, а сама кромка красится центрировано. Выноса
            // у нас нет: сетка стоит там, где у браузера ВНЕШНИЙ край
            // коробки, — поэтому кромку НАЧАЛЬНОЙ линии вжимаем внутрь, её
            // наружная половина и занимает недостающий вынос. Конец сетки
            // координату не сдвигает: там центр.
            let lo_line = grid_lo.unwrap_or_else(|| cands.first().map(|c| c.line).unwrap_or(0.0));
            let mut i = 0;
            while i < cands.len() {
                let mut j = i + 1;
                while j < cands.len() && (cands[j].line - cands[i].line).abs() < 0.75 {
                    j += 1;
                }
                let group = &cands[i..j];
                let outward = group
                    .iter()
                    .find_map(|c| (c.outward != 0).then_some(c.outward));
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
                    // Кромка стола лежит теперь на ТОЙ ЖЕ линии сетки, что и
                    // кромки краевых ячеек, поэтому центрируются ОБЕ
                    // (§17.6.2). Проверка: коробка стола начинается на
                    // `линия − outer_win/2`, победившая полоса шириной `w`
                    // занимает `линия ± w/2`, и при `w == outer_win` это ровно
                    // `край … край + w` — прежний вжим внутрь давал ту же
                    // полосу побайтно. При более узкой рамке стола полоса
                    // стола и не рисуется: линию забирает более широкая ячейка.
                    // Вжим по `lo_line` остаётся там, где пробы стола в группе
                    // нет вовсе: у такой таблицы коробка совпадает с внешними
                    // краями ячеек (на этом держится замер «нулевая проба
                    // таблицы», CSS2 +21/−25).
                    let edge_dir = if outward.is_none() && (line - lo_line).abs() < 0.75 {
                        Some(1)
                    } else {
                        None
                    };
                    let (lo, hi) = match edge_dir {
                        Some(-1) => (line - win.w, line),
                        Some(_) => (line, line + win.w),
                        None => (line - win.w / 2.0, line + win.w / 2.0),
                    };
                    // Продление В УГЛЫ — только у сплошных (`style >= 9`):
                    // пересечение иначе оставалось пустым квадратом, а
                    // продление пунктирных рисовало лишние усы. Обе оси
                    // тянутся на полуширину ПЕРПЕНДИКУЛЯРНОЙ кромки; кто из
                    // них накроет угол, решает порядок по ключу (см. `segs`).
                    let (a, b) = if win.style >= 9 {
                        if vertical {
                            (a - half_at_h(a, line), b + half_at_h(b, line))
                        } else {
                            (a - half_at(a, line), b + half_at(b, line))
                        }
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
                    segs.push((
                        (win.w, win.style, win.source, u32::MAX - win.doc_ix),
                        vertical,
                        rect,
                        win.colour,
                    ));
                }
                i = j;
            }
        };
        draw(&mut vert, true, grid_lo.map(|g| g.0));
        draw(&mut horiz, false, grid_lo.map(|g| g.1));
        segs.sort_by(|p, q| {
            p.0.partial_cmp(&q.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(p.1.cmp(&q.1).reverse())
        });
        // Snap every band edge to the device pixel grid the same way layout
        // bounds are rounded (`round()` of the absolute edge), so two bands
        // meeting at one exact coordinate share one device edge.
        let scale = window.scale_factor();
        let snap = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
        for (_, _, rect, colour) in segs {
            let x0 = snap(rect.origin.x);
            let y0 = snap(rect.origin.y);
            let x1 = snap(rect.origin.x + rect.size.width);
            let y1 = snap(rect.origin.y + rect.size.height);
            let rect = Bounds {
                origin: gpui::point(x0, y0),
                size: gpui::size(x1 - x0, y1 - y0),
            };
            window.paint_quad(gpui::fill(rect, colour.to_hsla()));
        }
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
#[derive(Clone)]
pub struct ClampEntry {
    pub bounds: Bounds<Pixels>,
    /// Высота строки в точках; 0 — блок без собственного текста.
    pub line: f32,
    /// Строки не считаются (элемент внутри вложенного BFC).
    pub skip_count: bool,
    /// Коробка с ЗАДАННОЙ высотой: фрагментировать нечего, пересечённая
    /// точкой среза она прячется целиком.
    pub fixed_height: bool,
    /// Нижние рамка и паддинг коробки в точках. Проба меряет ПАДДИНГ-БОКС,
    /// а фрагментированная коробка своих нижних рамки и паддинга не теряет
    /// (css-overflow-4 §5.3): на них укорачивается бюджет строк и на них же
    /// удлиняется итоговый срез. Для строчных проб — ноль.
    pub bp_after: f32,
    /// Порядковый номер абзаца ВНУТРИ клэмп-контейнера, выданный при
    /// построении. `None` — проба коробки, а не абзаца: знак обрыва на
    /// неё не садится. Номер, а не совпадение по геометрии, потому что
    /// сопоставлять надо КАДРЫ: срез считается по прошлому кадру, а
    /// применяется на следующем.
    pub seq: Option<u32>,
    /// Сколько строк этому абзацу оставлено УЖЕ на этом кадре. Признак
    /// того, что абзац укорочен нами: его собственный низ за срез больше
    /// не выходит, и без этой защёлки «что-то срезано» пропало бы, а
    /// кадр запросил бы себя заново (css-overflow-4 §5.3: вставка знака
    /// «must not cause a reevaluation of the effects of `continue`»).
    pub clamped: Option<usize>,
    /// Пустая поточная блочная коробка (без содержимого): сама по себе —
    /// возможная точка среза МЕЖДУ блоками (css-overflow-4 §5.3).
    pub empty: bool,
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
    /// Знак обрыва АВТО-режима, посчитанный на прошлом кадре:
    /// ключ контейнера → (номер абзаца, сколько его строк остаётся).
    /// Абзац в контейнере ровно один — тот, на чьей последней строке
    /// перед точкой среза стоит знак (css-overflow-4 §5.3).
    static CLAMP_PARA: std::cell::RefCell<std::collections::HashMap<u64, (u32, usize)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Счётчик абзацев контейнера при ПОСТРОЕНИИ поддерева.
    static CLAMP_SEQ: std::cell::RefCell<std::collections::HashMap<u64, u32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn clamp_lines_for(key: u64) -> ClampLines {
    CLAMP_LINES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

/// Выдать следующему абзацу клэмп-контейнера его номер. Счётчик сбрасывает
/// вход в сам контейнер (`ClampGuard::enter`), поэтому нумерация одна и та
/// же на каждом кадре, пока не меняется дерево.
pub fn clamp_next_seq(key: u64) -> u32 {
    CLAMP_SEQ.with(|m| {
        let mut m = m.borrow_mut();
        let n = m.entry(key).or_insert(0);
        let v = *n;
        *n += 1;
        v
    })
}

pub fn clamp_cut(key: u64) -> Option<f32> {
    CLAMP_CUTS.with(|m| m.borrow().get(&key).copied())
}

/// Бюджет строк абзаца со знаком обрыва: (номер абзаца, сколько строк
/// оставить). Считает `ClampCut::paint` прошлого кадра.
pub fn clamp_para(key: u64) -> Option<(u32, usize)> {
    CLAMP_PARA.with(|m| m.borrow().get(&key).copied())
}

pub fn forget_clamp_buffers() {
    CLAMP_LINES.with(|m| m.borrow_mut().clear());
    CLAMP_CUTS.with(|m| m.borrow_mut().clear());
    CLAMP_STACK.with(|st| st.borrow_mut().clear());
    CLAMP_PARA.with(|m| m.borrow_mut().clear());
    CLAMP_SEQ.with(|m| m.borrow_mut().clear());
    PARA_BUDGET.with(|c| c.set(None));
    PARA_ROWS.with(|m| m.borrow_mut().clear());
    PARA_TAG.with(|c| c.set(None));
}

/// Сторож стека clamp-контекста на время построения поддерева.
pub struct ClampGuard(bool);

impl ClampGuard {
    /// Вход в сам clamp-контейнер.
    pub fn enter(key: u64) -> Self {
        CLAMP_STACK.with(|st| st.borrow_mut().push((key, false)));
        // Нумерация абзацев контейнера начинается заново на каждом кадре:
        // иначе номер рос бы от кадра к кадру и бюджет прошлого кадра
        // никогда не находил бы своего абзаца.
        CLAMP_SEQ.with(|m| {
            m.borrow_mut().insert(key, 0);
        });
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

thread_local! {
    /// Настоящие строки абзацев клэмп-контейнера на этом кадре: (ключ,
    /// номер абзаца) → (верх, низ) строк в координатах окна. Пишет их
    /// отрисовка абзаца (`lines::Paragraph::paint`), забирает `ClampCut`
    /// (он рисуется последним ребёнком контейнера). Без них строки
    /// абзаца делились бы поровну, а строка с крупным кеглем или руби
    /// выше прочих (css-overflow-4 §5.3: точка среза — между строчными
    /// коробками, их высоты свои).
    static PARA_ROWS: std::cell::RefCell<std::collections::HashMap<(u64, u32), Vec<(f32, f32)>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// (ключ, номер) абзаца, который сейчас будет собран.
    static PARA_TAG: std::cell::Cell<Option<(u64, u32)>> = const { std::cell::Cell::new(None) };
}

pub fn set_para_tag(v: Option<(u64, u32)>) {
    PARA_TAG.with(|c| c.set(v));
}

pub fn take_para_tag() -> Option<(u64, u32)> {
    PARA_TAG.with(|c| c.take())
}

/// Отрисовка абзаца сообщает его строки (см. `PARA_ROWS`).
pub fn publish_para_rows(tag: (u64, u32), rows: Vec<(f32, f32)>) {
    PARA_ROWS.with(|m| {
        m.borrow_mut().insert(tag, rows);
    });
}

fn take_para_rows(key: u64) -> std::collections::HashMap<u32, Vec<(f32, f32)>> {
    PARA_ROWS.with(|m| {
        let mut m = m.borrow_mut();
        let tags: Vec<(u64, u32)> = m.keys().filter(|k| k.0 == key).copied().collect();
        tags.into_iter()
            .filter_map(|t| m.remove(&t).map(|mut v| {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                (t.1, v)
            }))
            .collect()
    })
}

thread_local! {
    /// Бюджет строк ТЕКУЩЕГО собираемого абзаца (только авто-режим).
    static PARA_BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Положить бюджет строк для абзаца, который сейчас будет собран.
pub fn set_para_budget(v: Option<usize>) {
    PARA_BUDGET.with(|c| c.set(v));
}

/// Забрать бюджет (и опустошить ячейку). Опустошение обязательно: куски
/// абзаца строят вложенные абзацы (`inline-block`), и им чужой бюджет
/// доставаться не должен.
pub fn take_para_budget() -> Option<usize> {
    PARA_BUDGET.with(|c| c.take())
}

/// Проба строк: канвас в элементе с текстом (или блоке), пишет границы и
/// высоту строки в prepaint своего кадра.
pub fn clamp_probe(
    lines: ClampLines,
    line: f32,
    skip_count: bool,
    fixed_height: bool,
    bp_after: f32,
    seq: Option<u32>,
    clamped: Option<usize>,
) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line,
                skip_count,
                fixed_height,
                bp_after,
                seq,
                clamped,
                empty: false,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Проба пустой поточной блочной коробки клэмп-контейнера: только её
/// положение. Точка среза после неё (§5.3 «a point between two in-flow
/// block-level sibling boxes») отделяет предыдущую строку от точки, и знака
/// обрыва на той строке нет (`line-clamp-auto-039/032`).
pub fn clamp_empty_probe(lines: ClampLines) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line: 0.0,
                skip_count: false,
                fixed_height: false,
                bp_after: 0.0,
                seq: None,
                clamped: None,
                empty: true,
            });
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
    /// `text-box-trim: trim-end` контейнера в точках: последняя строка
    /// ПЕРЕД точкой обрыва — последняя отформатированная, и её конец
    /// срезается (`text-box-trim-line-clamp-*`). Ноль — среза нет.
    trim_end: f32,
}

impl ClampCut {
    pub fn new(key: u64, lines: ClampLines, limit: Option<u32>, max_h: Option<f32>) -> Self {
        ClampCut {
            key,
            lines,
            limit,
            max_h,
            trim_end: 0.0,
        }
    }

    /// Срез конца последней видимой строки (`text-box-trim`).
    pub fn trim_end(mut self, v: f32) -> Self {
        self.trim_end = v.max(0.0);
        self
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
        let all_entries = std::mem::take(&mut *self.lines.borrow_mut());
        // Пустые коробки — только для выбора знака обрыва (ниже); в строки,
        // блоки и признак «за точкой есть содержимое» они не входят.
        let empties: Vec<ClampEntry> = all_entries.iter().filter(|e| e.empty).cloned().collect();
        let entries: Vec<ClampEntry> = all_entries.into_iter().filter(|e| !e.empty).collect();
        let para_rows = take_para_rows(self.key);
        // Строки текстового вклада: настоящие, если абзац их сообщил и они
        // лежат в его коробке; иначе — высота пробы поровну на строки.
        let split = |e: &ClampEntry| -> Vec<(f32, f32)> {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if let Some(r) = e.seq.and_then(|s| para_rows.get(&s))
                && !r.is_empty()
                && r.iter().all(|(a, b)| *a >= y0 - 0.5 && *b <= y0 + h + 0.5)
            {
                return r.clone();
            }
            let n = (h / e.line).round().max(1.0) as usize;
            let step = h / n as f32;
            (0..n)
                .map(|i| (y0 + i as f32 * step, y0 + (i + 1) as f32 * step))
                .collect()
        };
        let top = f32::from(bounds.origin.y);
        // Строки: у текстового вклада их bounds.height / line штук.
        let mut rows: Vec<(f32, f32, bool)> = vec![]; // (верх, низ, считается)
        // (верх, низ, заданная высота, нижние рамка+паддинг)
        let mut blocks: Vec<(f32, f32, bool, f32)> = vec![];
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                for (a, b) in split(e) {
                    rows.push((a, b, !e.skip_count));
                }
            } else if h > 0.0 {
                blocks.push((y0, y0 + h, e.fixed_height, e.bp_after));
            }
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cut: Option<f32> = self.max_h.map(|m| top + m);
        let trim_end = self.trim_end;
        // Счётный режим (css-overflow-4 §5.3): точка среза — СРАЗУ после N-й
        // считаемой строки. Это готовая строка, а не бюджет высоты, поэтому
        // правила авто-режима ниже (вычет нижних рамки и паддинга, посадка на
        // верх пересечённой строки) к ней не применяются: после `c2 -= bp`
        // точка уходила внутрь N-й строки, и строка пропадала
        // (`line-clamp-012/022`, `webkit-line-clamp-050`). Абзацу N-й строки
        // отдаётся бюджет строк — тот же `CLAMP_PARA`, что у авто-режима, и
        // «…» ставит строчный слой. Высоту даёт УКОРОЧЕННОЕ содержимое — с
        // полями, схлопыванием и заданной высотой предков (Blink: всё за
        // точкой `is_hidden_for_paint`, размер — по видимому); потолок нужен,
        // только когда за точкой есть другое содержимое.
        let mut num_para: Option<(u32, usize)> = None;
        let mut by_count = false;
        if let Some(limit) = self.limit.filter(|n| *n > 0) {
            // (верх строки, низ строки, номер абзаца, номер строки в абзаце)
            let mut marks: Vec<(f32, f32, u32, usize)> = vec![];
            for e in entries.iter().filter(|e| e.line > 0.0 && !e.skip_count) {
                let Some(seq) = e.seq else { continue };
                let h = f32::from(e.bounds.size.height);
                if h <= 0.0 {
                    continue;
                }
                for (i, (a, b)) in split(e).into_iter().enumerate() {
                    marks.push((a, b, seq, i + 1));
                }
            }
            marks.sort_by(|a, b| a.0.total_cmp(&b.0));
            match marks.get(limit as usize - 1).copied() {
                Some((_, bottom, seq, k)) if cut.is_none_or(|c| bottom <= c + 0.5) => {
                    // Остаток СВОЕГО абзаца — знак нужен, потолок нет: абзац
                    // укоротит бюджет.
                    let own_rest = entries.iter().any(|e| {
                        e.line > 0.0
                            && e.seq == Some(seq)
                            && f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height)
                                > bottom + 0.5
                    });
                    // Другое содержимое за точкой: чужой абзац, выходящий за
                    // неё (в том числе несчитаемый), или коробка, начатая после.
                    let follows = entries.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        let h = f32::from(e.bounds.size.height);
                        h > 0.0
                            && if e.line > 0.0 {
                                e.seq != Some(seq) && y0 + h > bottom + 0.5
                            } else {
                                y0 >= bottom - 0.5
                            }
                    });
                    if own_rest || follows || entries.iter().any(|e| e.clamped.is_some()) {
                        num_para = Some((seq, k));
                    }
                    by_count = true;
                    if follows {
                        // Коробки, содержащие точку, фрагментированы в ней и
                        // уносят свои нижние рамку и паддинг (§5.3); коробка с
                        // заданной высотой не фрагментируется — видна целиком.
                        let mut add = 0.0f32;
                        let mut floor = bottom;
                        for (y0, y1, fixed, bp) in &blocks {
                            if *y0 < bottom && bottom < *y1 {
                                if *fixed {
                                    floor = floor.max(*y1);
                                } else {
                                    add += *bp;
                                }
                            }
                        }
                        cut = Some((bottom + add).max(floor));
                    }
                }
                // `max-height` теснее N строк — дальше как в авто-режиме.
                Some(_) => {}
                None => {
                    if self.max_h.is_none() {
                        // Строк меньше предела — среза нет.
                        cut = None;
                    }
                }
            }
        }
        // Блок, СОДЕРЖАЩИЙ точку среза, фрагментируется по последней
        // влезающей строке — ПРЯЧЕТСЯ целиком только коробка с заданной
        // высотой: её не фрагментировать (css-overflow-4 §line-clamp).
        // Строка, пересечённая точкой, не показывается половинкой:
        // срез поднимается к её верху.
        if let Some(c) = cut.filter(|_| !by_count) {
            // Коробка с ЗАДАННОЙ высотой не фрагментируется: пересечённая
            // точкой среза, она прячется целиком (css-overflow-4 §5.3).
            let mut c2 = c;
            for (y0, y1, fixed, _) in &blocks {
                if *fixed && *y0 < c2 && c2 < *y1 {
                    c2 = *y0;
                }
            }
            // Коробка БЕЗ заданной высоты фрагментируется по последней
            // влезающей строке, но нижние рамку и паддинг с собой уносит:
            // бюджет строк на них укорачивается, а итоговый срез — на
            // столько же удлиняется (`line-clamp-auto-019`: 2+14+4×32+14+2
            // = 160 = ровно потолок `max-height: 5lh`).
            let crossed: Vec<(f32, f32)> = blocks
                .iter()
                .filter(|(y0, y1, fixed, _)| !*fixed && *y0 < c2 && c2 < *y1)
                .map(|(y0, _, _, bp)| (*y0, *bp))
                .collect();
            let bp: f32 = crossed.iter().map(|(_, bp)| *bp).sum();
            c2 -= bp;
            // Строка, которая влезает только СРЕЗАННОЙ (`text-box-trim:
            // trim-end` — последняя строка перед обрывом срезается), остаётся:
            // `max-height: 285px` при строке 100 и срезе 25 — три строки
            // (3·100 − 25 = 275), а не две (`line-clamp-auto-001/002`).
            for (y0, y1, _) in &rows {
                if *y0 < c2 && c2 < *y1 - trim_end - 0.5 {
                    c2 = *y0;
                }
            }
            // И точка обрыва садится на срезанный низ последней видимой
            // строки: при числовом пределе — всегда, в авто-режиме — только
            // когда строки идут дальше точки (иначе обрыва нет и срез уже
            // сделал хвост `blocks()`).
            if trim_end > 0.0
                && (self.limit.is_some() || rows.iter().any(|r| r.1 > c2 + 0.5))
                && let Some(b) = rows
                    .iter()
                    .map(|r| r.1)
                    .filter(|y1| *y1 - trim_end <= c2 + 0.5)
                    .max_by(|a, b| a.total_cmp(b))
            {
                c2 = c2.min(b - trim_end);
            }
            // Пересечённая коробка, в которую с её верхними рамкой и паддингом
            // не влезло НИ ОДНОЙ строки, не фрагментируется: точка среза — между
            // ней и предыдущим соседом (§5.3 «between two in-flow block-level
            // sibling boxes»; `line-clamp-auto-024`: 224, а не 240). Её нижние
            // рамка и паддинг не нужны; охватывающие непустые свои уносят.
            // Строка «влезла» — по СРЕЗАННОМУ низу (`trim_end`, как в цикле
            // выше): без поправки последняя видимая строка под `text-box-trim`
            // не считалась бы, и срез уходил бы к верху её коробки.
            let holds = |y0: f32| {
                rows.iter().any(|(r0, r1, countable)| {
                    *countable && *r0 >= y0 - 0.5 && *r1 - trim_end <= c2 + 0.5
                })
            };
            match crossed
                .iter()
                .filter(|(y0, _)| !holds(*y0))
                .map(|(y0, _)| *y0)
                .reduce(f32::min)
            {
                Some(empty_top) => {
                    let keep: f32 = crossed
                        .iter()
                        .filter(|(y0, _)| holds(*y0) && *y0 < empty_top)
                        .map(|(_, bp)| *bp)
                        .sum();
                    cut = Some(empty_top + keep);
                }
                None => cut = Some(c2 + bp),
            }
        }
        // ★ ЗАМЕРЕНО (10.09, `scout-clampmarker-2026-09c.md`, 21 хунк):
        // срез 416 пар (семья `line-clamp` + схлопывание полей) 259 ->
        // 260. Взяты `line-clamp-auto-003` 1.82 -> 0.00 и `-047`
        // 1.62 -> 0.12; четвёрка `-018`…`-021` стояла на 0.39-0.40 и
        // встала РОВНО на 0.00 — знак наконец в конце текста строки.
        // Потеря одна: `-022` 0.39 -> 2.61 — тот же тест, что `-021`,
        // но `max-height: 5.5lh` вместо `5lh` при том же эталоне;
        // полстроки сверх бюджета у нас пускают лишнюю строку. Долг
        // отдельным подкорнем CLAMP-HALF-LINE.
        // Знак обрыва в АВТО-режиме (`limit == None`). Рисует его НЕ этот
        // слой: сюда возвращается только БЮДЖЕТ строк — номер абзаца в
        // контейнере и сколько его строк остаётся выше среза, — а «…»
        // ставит уже проверенный `lines::clamp_lines`/`paint_line`: в
        // конце ТЕКСТА строки, с выключкой и направлением письма
        // (css-overflow-4: знак «is placed at the end of the line box
        // reducing the space available to the other contents of the
        // line», а для bidi — анонимный строчный с уровнем bidi-абзаца).
        // Так же развязан и Blink: блочный слой отдаёт признак
        // `IsAtClampPoint`, а ширину знака получает разрыватель строк
        // (`inline_layout_algorithm.cc:1247` `SetupLineClampEllipsis` →
        // `SetLineClampEllipsisWidth`). Прежний набросок рисовал знак у
        // ПРАВОГО края коробки — мимо конца текста, мимо выключки и
        // мимо rtl.
        // Числовой предел, который `max-height` перехватил раньше N-й строки
        // (`line-clamp: 4 auto` при `max-height: 3lh`), — та же точка
        // обрыва, что в авто-режиме: §5.3 берёт ПЕРВУЮ из двух точек, и знак
        // встаёт на последнюю строку перед ней (`line-clamp-041`).
        let para = cut.filter(|_| !by_count).and_then(|c| {
            // Есть ли что резать. Как только бюджет применён, абзац УЖЕ
            // укорочен и сам за срез не выходит — признак защёлкивается
            // применённым бюджетом, иначе кадры зациклились бы:
            // обрезали → влезло → сняли → снова не влезло.
            let overflow = entries.iter().any(|e| e.clamped.is_some())
                || entries.iter().any(|e| {
                    f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height) > c + 0.5
                });
            if !overflow {
                return None;
            }
            // Знак садится на ПОСЛЕДНЮЮ строку перед точкой среза — в том
            // числе когда точка стоит МЕЖДУ блоками и сам абзац видим
            // целиком. Абзацы в своём контексте форматирования
            // пропускаются: точкой среза их строки быть не могут.
            // Последняя строка перед точкой — во ВЛОЖЕННОМ контексте
            // форматирования (несчитаемая): знак не ставится вовсе, а не
            // уходит на предыдущий считаемый абзац (css-overflow-4 §5.3:
            // многоточие — на последней строке ПЕРЕД точкой среза в этом
            // BFC; `line-clamp-auto-034/039`: «Line 4» без знака).
            entries
                .iter()
                .filter(|e| e.line > 0.0 && (e.skip_count || e.seq.is_some()))
                .filter_map(|e| {
                    let seq = if e.skip_count { None } else { e.seq };
                    let h = f32::from(e.bounds.size.height);
                    if h <= 0.0 {
                        return None;
                    }
                    let rows = split(e);
                    // Точка `c` уже стоит на СРЕЗАННОМ низу последней
                    // строки — полный низ этой строки ниже на `trim_end`.
                    let k = rows
                        .iter()
                        .filter(|(_, b)| *b <= c + trim_end + 0.5)
                        .count();
                    (k >= 1).then(|| (rows[k - 1].1, seq, k))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                // Пустая блочная коробка между последней строкой и точкой
                // среза: точка — после неё (последняя возможная), и строка
                // точке уже не предшествует — знака нет (§5.3).
                .filter(|(bottom, _, _)| {
                    !empties.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        e.empty
                            && f32::from(e.bounds.size.height) <= 0.5
                            && y0 >= *bottom - 0.5
                            && y0 <= c + 0.5
                    })
                })
                .and_then(|(_, seq, k)| seq.map(|s| (s, k)))
        });
        // Бюджет одного и того же абзаца только УЖИМАЕТСЯ: рост числа
        // строк на следующем кадре — это отражение нашей же правки, а не
        // новое измерение. Правило конечно (бюджет строго убывает и не
        // меньше единицы), поэтому кадр не может просить себя без конца.
        // Счётный режим несёт свой бюджет (см. выше); авто-режим — свой.
        let para = if by_count { num_para } else { para };
        let prev_para = clamp_para(self.key);
        let para = match (prev_para, para) {
            (Some((ps, pk)), Some((s, k))) if ps == s && k > pk => Some((ps, pk)),
            (_, v) => v,
        };
        if prev_para != para {
            CLAMP_PARA.with(|m| {
                let mut m = m.borrow_mut();
                match para {
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
        let rel = cut.map(|c| (c - top).max(0.0));
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("HTML_CLAMP_DBG").is_ok());
            *ON
        } {
            eprintln!(
                "CLAMPCUT key={} rows={} blocks={} limit={:?} max_h={:?} rel={:?}",
                self.key,
                rows.len(),
                blocks.len(),
                self.limit,
                self.max_h,
                rel
            );
        }
        // Гистерезис: мелкие колебания точки (обрезка двигает схлопнутые
        // поля, точка плывёт на доли строки) не перезаписывают её — иначе
        // пары мигали между прогонами. Крупный сдвиг — честный пересчёт.
        // «Измерено: резать нечего» хранится БЕСКОНЕЧНОСТЬЮ, а не пустотой:
        // пустота значит «ещё не мерили», и `styled_div_with` подставлял бы
        // запасной потолок `N × line-height` навсегда (`webkit-line-clamp-029`:
        // все строки в своём контексте, считать нечего, а коробка резалась на
        // три строки из пяти). Запасной потолок остаётся только первому кадру.
        let v = rel.unwrap_or(f32::INFINITY);
        let prev = clamp_cut(self.key);
        let changed = match prev {
            Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
            Some(a) => a.is_finite() != v.is_finite(),
            None => true,
        };
        if changed {
            CLAMP_CUTS.with(|m| {
                m.borrow_mut().insert(self.key, v);
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
///
/// `border` — ширины сторон ячейки в порядке верх-право-низ-лево. Абсолютный
/// ребёнок в taffy лежит ВНУТРИ рамки, поэтому канвас меряет поле подкладки,
/// а §17.5.1 велит вести фон полосы «from the top of the cells to the bottom
/// of the cells», то есть по внешним краям рамок: ячейка с
/// `border-bottom: 60px` и пустым содержимым давала полосе нулевую высоту.
pub fn cell_rect_probe(
    rects: RowRects,
    exact: bool,
    shift: (f32, f32),
    border: [f32; 4],
) -> AnyElement {
    CellProbe {
        child: Some(
            gpui::div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .into_any_element(),
        ),
        rects,
        exact,
        shift,
        border,
    }
    .into_any_element()
}

/// Проба ячейки (`cell_rect_probe`): записывает и округлённый прямоугольник
/// (маски краски), и неокруглённый (область позиционирования фона полосы).
/// Поле подкладки ячейки округляется от её внутреннего края рамки: 25px
/// рамки при 1.25 сдвигали его на 0.2px, и плитка `top right` у tbody
/// вставала на точку правее эталона (`background-position-applies-to-001a`).
struct CellProbe {
    child: Option<AnyElement>,
    rects: RowRects,
    exact: bool,
    shift: (f32, f32),
    border: [f32; 4],
}

impl CellProbe {
    fn outer(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let (shift, border) = (self.shift, self.border);
        // Сдвиг краски относительно коробки ячейки: в сросшейся модели
        // фоновая сетка начинается от середины рамки таблицы.
        Bounds {
            origin: gpui::point(
                bounds.origin.x + gpui::px(shift.0 - border[3]),
                bounds.origin.y + gpui::px(shift.1 - border[0]),
            ),
            size: gpui::size(
                bounds.size.width + gpui::px(border[1] + border[3]),
                bounds.size.height + gpui::px(border[0] + border[2]),
            ),
        }
    }
}

impl Element for CellProbe {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Запись В PREPAINT: подготовка ВСЕХ элементов идёт до отрисовки,
        // и полоса фона читает прямоугольники СВОЕГО кадра — с записью в
        // paint она рисовала прошлый кадр и мигала на каждой смене раскладки.
        let unrounded = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        // Неокруглённое берётся, только пока оно в пределах точки
        // устройства от округлённого (иначе — другой кадр отсчёта).
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        let unrounded = if near(unrounded.left(), bounds.left())
            && near(unrounded.top(), bounds.top())
            && near(unrounded.right(), bounds.right())
            && near(unrounded.bottom(), bounds.bottom())
        {
            unrounded
        } else {
            bounds
        };
        self.rects
            .borrow_mut()
            .push((self.outer(bounds), self.exact, self.outer(unrounded)));
        self.child.as_mut().unwrap().prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for CellProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Строка вертикального письма: `writing-mode: vertical-rl` и `vertical-lr`.
///
/// Поворота мало: у повёрнутого текста меняются местами ширина и высота, и
/// раскладка обязана считать их поменянными — иначе строка занимает место как
/// горизонтальная и наезжает на соседей. Поэтому это свой элемент: он
/// измеряет текст сам, отдаёт раскладке перевёрнутый размер, а на отрисовке
/// разворачивает содержимое на четверть оборота по часовой стрелке.
/// `text-combine-upright`: составной знак в вертикальной строке.
///
/// Абзац вертикального письма рисуется ПОВОРОТОМ на четверть по часовой;
/// сжатый кусок обязан остаться стоячим — он контр-поворачивается вокруг
/// СВОЕГО ЦЕНТРА (квадрат кегля переходит в себя) и ужимается по строчной
/// оси в один кегль (css-writing-modes-3 §9.1).
pub struct CombinedUpright {
    child: Option<AnyElement>,
    /// Кегль — сторона квадрата, который кусок занимает в строке.
    em: f32,
    /// Сжимать ли содержимое в кегль: у `text-combine-upright` — да, у
    /// стоячего `inline-block` с горизонтальным письмом — нет, он просто
    /// переполняет свой квадрат.
    compress: bool,
    natural: gpui::Size<Pixels>,
}

impl CombinedUpright {
    pub fn new(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: true,
            natural: gpui::Size::default(),
        }
    }

    /// Стоячая коробка без сжатия (см. поле `compress`).
    pub fn upright_box(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: false,
            natural: gpui::Size::default(),
        }
    }
}

impl Element for CombinedUpright {
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
        let space = gpui::size(
            gpui::AvailableSpace::MaxContent,
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = self
            .child
            .as_mut()
            .unwrap()
            .layout_as_root(space, window, cx);
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("VT_DBG").is_ok());
            *ON
        } {
            eprintln!("VT natural={:?}", self.natural);
        }
        let mut style = gpui::Style::default();
        let side = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(px(self.em)),
        ));
        style.size.width = side.clone();
        style.size.height = side;
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
        let child = self.child.as_mut().unwrap();
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
        let matrix = combined_geometry::transform(
            bounds,
            self.natural,
            self.em,
            self.compress,
            window.scale_factor(),
        );
        let child = self.child.as_mut().unwrap();
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}

impl IntoElement for CombinedUpright {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

thread_local! {
    /// Двухкадровый замер повёрнутого блока: ключ абзаца → фактическая
    /// высота содержимого при РЕШЁННОЙ длине строки (см. `prepaint`).
    /// Первый кадр заявляет ширину по свободному замеру, второй — по факту;
    /// стенд и так ждёт устоявшийся кадр (как пробы ячеек).
    static VT_MEASURED: std::cell::RefCell<std::collections::HashMap<u64, Pixels>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Счётчик ВХОЖДЕНИЙ базового ключа за кадр: два вертикальных абзаца с
    /// одинаковым текстом и числом узлов (повторяющиеся ячейки) делили один
    /// ключ, и замер одного применялся к другому. Порядок обхода кадра
    /// детерминирован — порядковый номер вхождения стабилен между кадрами.
    static VT_SEQ: std::cell::RefCell<std::collections::HashMap<u64, u64>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Замер ЛЕВОГО края коробки корня (ключ — соль документа): фон холста
    /// позиционируется от коробки корня (CSS 2.2 §14.2), а она при
    /// `vertical-rl` по содержимому и прижата к правому краю окна — её край
    /// известен только после раскладки. Пишет подготовка тела, читает
    /// отрисовка холста того же кадра (подготовка всего дерева идёт раньше
    /// отрисовки); прошлое значение — запасное.
    static ROOT_LEFT: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Левый край коробки корня (см. `ROOT_LEFT`).
pub fn root_left_prev(key: u64) -> Option<f32> {
    ROOT_LEFT.with(|c| c.borrow().get(&key).copied())
}

/// Обёртка, которая на подготовке записывает левый край своей коробки минус
/// `offset` (поля тела и рамка/отбивка корня) в `ROOT_LEFT` — раскладку не
/// меняет: узел раскладки — сам ребёнок.
pub struct RecordRootLeft {
    child: AnyElement,
    key: u64,
    offset: f32,
}

pub fn record_root_left(child: AnyElement, key: u64, offset: f32) -> AnyElement {
    RecordRootLeft { child, key, offset }.into_any_element()
}

impl Element for RecordRootLeft {
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
        (self.child.request_layout(window, cx), ())
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
        let left = f32::from(bounds.origin.x) - self.offset;
        ROOT_LEFT.with(|c| c.borrow_mut().insert(self.key, left));
        self.child.prepaint(window, cx);
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
        self.child.paint(window, cx);
    }
}

impl IntoElement for RecordRootLeft {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Санитария начала кадра (`render`/`render_block`): счётчик вхождений
/// VT-ключей обнуляется, а НЕЗАКРЫТЫЕ слои позиционированных выбрасываются —
/// пойманная паника прошлого кадра оставляла слой навсегда, и `late_close`
/// следующей страницы отдавал чужие элементы.
/// Забыть двухкадровые замеры вертикальных абзацев — при смене документа:
/// ключи солятся документом, но мусор копился бы бесконечно.
pub fn forget_vt_measures() {
    VT_MEASURED.with(|c| c.borrow_mut().clear());
    ROOT_LEFT.with(|c| c.borrow_mut().clear());
    // Рамка повёрнутого абзаца живёт только на время его подготовки; если
    // подготовка оборвалась паникой, `set(prev)` не выполнится, и рамка
    // протекла бы в следующие страницы — щупы горизонтальных абсолютов
    // считались бы повёрнутыми (`abspos-*` из CSS2 уходили в красное).
    VT_FRAME.with(|c| c.set(None));
}

pub fn frame_sanitize() {
    // Рамка повёрнутого абзаца не должна пережить страницу: сброс при смене
    // документа приходит позже начала следующего рендера, и щупы
    // горизонтальных абсолютов считались повёрнутыми (`css-sizing/aspect-
    // ratio/abspos-014..021`: 0.00 в одиночку и 2.36 в пачке).
    VT_FRAME.with(|c| c.set(None));
    VT_SEQ.with(|c| c.borrow_mut().clear());
    LATE.with(|s| s.borrow_mut().clear());
    // Слой начального содержащего блока — тот же расходник кадра: пойманная
    // паника оставила бы его открытым навсегда, и следующий документ клал бы
    // свои внепоточные элементы в чужой слой.
    ICB.with(|s| s.borrow_mut().clear());
    CB.with(|s| s.borrow_mut().clear());
    CB_FIXED.with(|s| s.borrow_mut().clear());
    // Реестр якорей — расходник кадра того же рода: пишется на подготовке,
    // читается там же, к следующей сборке дерева обязан быть пуст.
    crate::anchor::reset();
}

/// Ключ с порядковым номером вхождения базового ключа в этом кадре.
pub fn vt_seq_key(base: u64) -> u64 {
    VT_SEQ.with(|c| {
        let mut m = c.borrow_mut();
        let n = m.entry(base).or_insert(0);
        *n += 1;
        base ^ n.wrapping_mul(0x517C_C1B7_2722_0A95)
    })
}

thread_local! {
    /// Сборщик внутреннего строчного размера повёрнутого текста (`None` —
    /// закрыт): наибольшая длина строки всех `VerticalText`, разложенных,
    /// пока он открыт.
    pub static VT_INLINE_MAX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

pub struct VerticalText {
    child: Option<AnyElement>,
    /// Естественный размер содержимого до поворота.
    natural: gpui::Size<Pixels>,
    /// Потолок заявляемой высоты (см. `claiming_height`).
    claim_cap: Option<Pixels>,
    /// Ключ двухкадрового замера (текст абзаца + соль документа).
    key: Option<u64>,
    /// Предел строки от родителя: если строка УЖЕ помещается, высота
    /// заявляется честно — иначе гибкая ячейка считает коробку нулевой и
    /// `justify-content` уводит рисунок из виду (table-cell-align-005).
    fit_limit: Option<Pixels>,
    inline_constraint: Option<crate::computed::orthogonal::InlineConstraint>,
    inline_keyword: Option<crate::computed::orthogonal::InlineKeyword>,
    /// `writing-mode: sideways-lr` — поворот ПРОТИВ часовой стрелки.
    /// css-writing-modes-4, таблица Abstract-Physical Mapping: у `sideways-lr`
    /// line-left = НИЗ, line-right = ВЕРХ, over = ЛЕВО (у всех остальных
    /// вертикальных письмён line-left = верх, over = право). Blink различает
    /// эти два случая ровно так же — `paint/line_relative_rect.cc:69-75`:
    /// `AffineTransform(0, 1, -1, 0, …)` против `AffineTransform(0, -1, 1, 0, …)`.
    ccw: bool,
    /// Ячейка вертикальной таблицы: мерить содержимое по МИНИМАЛЬНОМУ
    /// вдоль строки, а не по максимальному. Тогда заявленная высота
    /// повёрнутой коробки — вклад ячейки в меру её КОЛОНКИ (css-tables-3
    /// §computing-column-measures), и дорожку считает решётка, а не
    /// инлайн-размер всего стола. Ставится из `render.rs` (`col_min`).
    col_min: bool,
    /// `vertical-lr` при повороте по часовой: строки поданы снизу вверх, и
    /// первая строка — ЛЕВАЯ колонка (у `vertical-rl` — правая).
    lr: bool,
    /// Первая строка для базовой по оси x: шрифт, кегль, высота строки
    /// (`None` — `normal`) и центральная ли доминантная базовая.
    first_line: Option<(gpui::Font, Pixels, Option<Pixels>, bool)>,
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
        //
        // Ячейка вертикальной таблицы меряется по МИНИМАЛЬНОМУ содержимому
        // вдоль строки: её вклад в дорожку колонки — это min-content
        // («the outer min-content width of each cell that spans the column»,
        // css-tables-3 §computing-column-measures), а не длина всей строки.
        // Обёртка до поворота стоит при этом БЕЗ жёсткой ширины
        // (`render.rs`, `col_min`), поэтому запрос доходит до самого абзаца:
        // `Paragraph` на `AvailableSpace::MinContent` отвечает
        // `probe.min_content(window)` (`lines.rs`). Дальше `natural.width` —
        // инлайн-мера колонки (её заявит высотой `fit_within`), а
        // `natural.height` — число строк при этой мере, то есть блочная
        // толщина ряда.
        let space = gpui::size(
            if self.col_min {
                gpui::AvailableSpace::MinContent
            } else {
                gpui::AvailableSpace::MaxContent
            },
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = if let Some(constraint) = self.inline_constraint {
            orthogonal_measure::measure(self.child.as_mut().unwrap(), constraint, self.inline_keyword, window, cx)
        } else {
            self.child.as_mut().unwrap().layout_as_root_unrounded(space, window, cx)
        };
        // Внутренний строчный размер повёрнутого текста — длина его самой
        // длинной строки (`natural.width` горизонтального абзаца до
        // поворота). Наружу высотой он не заявляется (см. ниже), и пробе
        // флоата хоста полос (`band_flow::intrinsic`, письмо вертикальное)
        // его взять неоткуда — сборщик кладёт его сюда, когда открыт.
        VT_INLINE_MAX.with(|c| {
            if let Some(v) = c.get() {
                c.set(Some(v.max(f32::from(self.natural.width))));
            }
        });
        let mut style = gpui::Style::default();
        // Ordinary orthogonal blocks claim the independently measured inline size;
        // other contexts let their existing parent-sizing contract decide height.
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("VT_DBG").is_ok());
            *ON
        } {
            eprintln!("VT2 natural={:?} cap={:?}", self.natural, self.claim_cap);
        }
        // Факт прошлого кадра сильнее свободного замера: перенос строк при
        // решённой длине меняет число колонок, а свободный замер его не
        // видит (text-combine-upright-line-breaking-rules-001).
        let claim = if self.inline_keyword.is_some() {
            self.natural.height
        } else { self
            .key
            .and_then(|k| VT_MEASURED.with(|c| c.borrow().get(&k).copied()))
            .unwrap_or(self.natural.height) };
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("VT_DBG").is_ok());
            *ON
        } {
            eprintln!("VT3 key={:?} claim={:?}", self.key, claim);
        }
        style.size.width = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(claim),
        ));
        vertical_line_baseline::apply(
            &mut style,
            self.child.as_mut().unwrap(),
            self.natural,
            self.ccw,
            self.lr,
            self.first_line.as_ref(),
            window,
            cx,
        );
        if self.inline_constraint.is_some() {
            style.size.height = self.natural.width.into();
        } else if let Some(cap) = self.claim_cap
            && self.natural.width >= cap
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(cap),
            ));
        } else if let Some(limit) = self.fit_limit
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (02.10): допуск +1 на привязку к пикселю
            // (`natural.width <= limit + 1`, заявка `min(natural, limit)`).
            // Обёртка до поворота стоит ЖЁСТКОЙ шириной в предел, и при пределе
            // 394 (рамка 3) замер возвращает 394.4 — строгое `<=` заявку
            // отклоняет, коробка схлопывается в рамку. С допуском:
            // `sizing-orthog-v{lr,rl}-in-htb-007/010/019/022` 0.68-0.83 → 0.00
            // (+8), но `-008/009/011/020/021/023/024` (обе стороны) 0.00-0.20 →
            // 0.55-0.75 (−14): у них эталон — абсолютная вертикальная коробка с
            // `height: auto`, и она схлопывается так же; короткой строке нужна
            // её длина, а жёсткая ширина даёт всегда предел. Возвращать только
            // вместе с замером длины строки и правкой абсолютного эталона.
            && self.natural.width <= limit
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(self.natural.width),
            ));
        }
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
            let sized = child.layout_as_root_unrounded(space, window, cx);
            // Факт для следующего кадра: высота содержимого при решённой
            // длине строки — она и есть настоящая ширина повёрнутого блока.
            if let Some(k) = self.key
                && sized.height > gpui::px(0.)
            {
                VT_MEASURED.with(|c| {
                    let mut map = c.borrow_mut();
                    if map.len() >= 256 {
                        map.clear();
                    }
                    map.insert(k, sized.height);
                });
            }
        }
        // Щупу статической позиции нужна и СТОРОНА поворота: отображение
        // до-поворотной точки в экранную у `sideways-lr` зеркально (см. `vt_map`).
        let prev_ccw = VT_CCW.with(|c| c.replace(self.ccw));
        let prev = VT_FRAME.with(|c| c.replace(Some(bounds)));
        child.prepaint_at(bounds.origin, window, cx);
        VT_FRAME.with(|c| c.set(prev));
        VT_CCW.with(|c| c.set(prev_ccw));
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
        // ЗАМЕРЕНО И ОТКАЧЕНО (01.09): статическая позиция абсолюта внутри
        // ВЕРТИКАЛЬНОЙ строки. Корень виден числом: щуп пишет дырку в
        // ДО-поворотных координатах (матрица ниже применяется только на
        // отрисовке), а стиль строки собран горизонтальным
        // (`render.rs`: `horizontal.vertical = None`), поэтому вынесенный
        // абсолют считается горизонтальным — печать дала `vert=false`,
        // коробку 80×16 вместо 16×80 и сдвиг по поперечной оси.
        // Пробовал вдвоём: (1) отображать дырку через тот же поворот,
        // (2) возвращать вынесенному абсолюту письмо. Срез из 24 пар
        // `static-position/v{lr,rl}-*` — 0/24 и до, и после, а две пары
        // (`vrl-rtl-*-in-multicol`) ушли 4.32 → «красное видно».
        // Осталось незакрытым: коробка растягивается на всю строку (16×400
        // вместо 16×80), и `display: inline` под абсолютом доходит сюда
        // блочным (`line=Some(16)` во всех шести местах документа), поэтому
        // берётся блочный рукав и смещение вдоль строки теряется.
        let scale_factor = window.scale_factor();
        let dev = |v: Pixels| v.scale(scale_factor);
        // Поворот на четверть по часовой стрелке вокруг левого верхнего угла
        // уводит содержимое влево от коробки; сдвиг на её ширину возвращает
        // его на место.
        //
        // `sideways-lr` (css-writing-modes-4, Abstract-Physical Mapping):
        // строчная ось идёт СНИЗУ ВВЕРХ, ascender смотрит ВЛЕВО — значит
        // поворот ПРОТИВ часовой. Он уводит содержимое ВВЕРХ от коробки,
        // поэтому возвращает его сдвиг на ВЫСОТУ, а не на ширину.
        // После такого поворота первая горизонтальная строка сама оказывается
        // ЛЕВОЙ колонкой, а её начало — у нижнего края: подача строк снизу
        // вверх (`lines_reversed`) больше не нужна, см. `render.rs`.
        let (shift, angle) = if self.ccw {
            (
                gpui::point(
                    dev(bounds.origin.x),
                    dev(bounds.origin.y + bounds.size.height),
                ),
                -std::f32::consts::FRAC_PI_2,
            )
        } else {
            (
                gpui::point(
                    dev(bounds.origin.x + bounds.size.width),
                    dev(bounds.origin.y),
                ),
                std::f32::consts::FRAC_PI_2,
            )
        };
        let matrix = gpui::TransformationMatrix::unit()
            .translate(shift)
            .rotate(gpui::Radians(angle))
            .translate(gpui::point(dev(-bounds.origin.x), dev(-bounds.origin.y)));
        let child = self.child.as_mut().unwrap();
        // Свой слой с ПОСЛЕ-поворотными границами: порядок отрисовки сцена
        // считает по границам примитивов, а у повёрнутого текста они
        // ДО-трансформные — лежат вне своей коробки, перекрытие с фоном не
        // видно, и градиент соседа красился ПОВЕРХ глифа
        // (table-cell-align-005: с третьей ячейки текст пропадал под фоном).
        window.paint_layer(bounds, |window| {
            window.with_transformation(matrix, |window| child.paint(window, cx));
        });
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
thread_local! {
    /// Рамка `VerticalText`, внутри которого сейчас идёт подготовка.
    ///
    /// Повёрнутый абзац подготавливается в ДО-ПОВОРОТНОЙ системе, а матрица
    /// поворота живёт только в `paint`. Щуп статической позиции пишет дырку
    /// именно в подготовке, поэтому без этой рамки `LatePlace` читает
    /// до-поворотную точку как экранную, и коробка уезжает на колонку.
    static VT_FRAME: std::cell::Cell<Option<Bounds<Pixels>>> =
        const { std::cell::Cell::new(None) };
    /// Сторона поворота этой рамки: `sideways-lr` вертится против часовой.
    static VT_CCW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Экранная точка для до-поворотной, если мы внутри повёрнутого абзаца.
/// Mapping follows the actual clockwise/counter-clockwise text frame.
/// Идёт ли сейчас подготовка ПОВЁРНУТОГО абзаца (`VerticalText`).
pub fn in_rotated_frame() -> bool {
    VT_FRAME.with(|c| c.get()).is_some()
}

fn vt_map(hole: Bounds<Pixels>, thickness: Pixels) -> Bounds<Pixels> {
    let Some(vt) = VT_FRAME.with(|c| c.get()) else {
        return hole;
    };
    let pre_x = hole.origin.x - vt.origin.x;
    let pre_y = hole.origin.y - vt.origin.y;
    // `sideways-lr` вертится ПРОТИВ часовой (см. `VerticalText::paint`):
    // до-поворотная `(px, py)` от угла рамки становится экранной
    // `(x + py, y + h - px - thickness)` — зеркало обычного случая по обеим
    // осям (css-writing-modes-4: строчная ось снизу вверх, over слева).
    if VT_CCW.with(|c| c.get()) {
        return Bounds {
            origin: gpui::point(
                vt.origin.x + pre_y,
                vt.origin.y + vt.size.height - pre_x - thickness,
            ),
            size: hole.size,
        };
    }
    Bounds {
        origin: gpui::point(
            vt.origin.x + vt.size.width - pre_y - thickness,
            vt.origin.y + pre_x,
        ),
        size: hole.size,
    }
}

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
    /// СОБСТВЕННОЕ письмо элемента вертикально (поток вокруг — нет):
    /// статическая позиция такого абсолюта смещена на его ширину —
    /// блок vrl вешается своим блок-началом, правым краем
    /// (abs-pos-border-offset-003, ref: left 55 = контент-лево + width).
    pub own_vertical: bool,
    /// Коробка ЗАМЕЩАЕМАЯ. Голый заместитель решает уравнение §10.6.4 —
    /// «Absolutely positioned, non-replaced elements»; у замещаемых своё,
    /// §10.6.2, где высота берётся от природного размера, а не от
    /// содержащего блока. ★ ЗАМЕРЕНО: без этого различения абсолютный
    /// `<iframe>` с процентной высотой обваливался с 0.02 в «красное видно»
    /// (`absolute-replaced-height-012/019/026/033`).
    pub replaced: bool,
    /// Ось, которую уже разрешил СОДЕРЖАЩИЙ БЛОК (край в `inset` задан):
    /// сдвиг по ней не нужен. `(x, y)`. Умолчание `(false, false)` — правятся
    /// обе оси, то есть прежнее поведение верхнего слоя.
    ///
    /// Слою начального содержащего блока нужна ровно ПУСТАЯ ось: заданную
    /// считает раскладка от области просмотра, а статическая позиция нужна
    /// только там, где обе стороны `auto` (§10.3.7, §10.6.4).
    pub fixed_axes: (bool, bool),
    /// Толщина колонки строки в повёрнутом абзаце: на неё отступает экранная
    /// точка дырки от правого края рамки. Ноль — коробка вне поворота.
    pub line_thickness: f32,
    /// Дырка уже отображена из повёрнутого абзаца в экранную систему
    /// (`vt_map`): дальше её ставит не горизонтальный рукав щупа, а свой —
    /// сдвиг только по свободной оси, при `rtl` — от НИЖНЕГО края.
    pub rotated: bool,
    /// Поле по СВОБОДНОЙ оси, в точках. Базовый сдвиг ставит коробку ровно в
    /// дырку, а по CSS от статической позиции её отодвигает собственное поле.
    pub free_margin: (f32, f32),
    /// Доля вдоль СТРОЧНОЙ оси, где стоит статическая точка: 0 — начало
    /// строки, 0.5 — середина, 1 — конец. Её задаёт `text-align` содержащего
    /// блока: гипотетическая коробка строчного абсолюта лежит в строке и
    /// выравнивается вместе с ней (CSS 2.1 §10.3.7 «где коробка была бы при
    /// `position: static`», css-align-3 §abspos). `None` — прежний ход:
    /// начало строки по `direction` (0 при ltr, 1 при rtl).
    pub line_align: Option<f32>,
    /// Щуп — БЛОЧНАЯ распорка во всю ширину содержащего блока
    /// (`spot_probe(_, true)`), а не точечный щуп в строке.
    ///
    /// Порода щупа решает, вешается ли коробка при `direction: rtl` на
    /// статическую точку своим ПРАВЫМ краем (§10.3.7: «otherwise set 'right'
    /// to the static position»). У распорки — да: её правый край и есть
    /// правый край содержащего блока. У точечного щупа в строке — нет: точка
    /// уже готова, а вычет своей ширины уводил бы коробку влево целиком
    /// (откат `htb-rtl-*` 08-19).
    ///
    /// Прежде порода узнавалась косвенно, по `hole.size.width > 0`. Признак
    /// ложен у распорки в содержащем блоке НУЛЕВОЙ ширины:
    /// `containing-block-020/022` (`div{width:0; padding:1in}`) уезжали ровно
    /// на свою ширину. Blink держит эти два шага раздельно: полосу
    /// выравнивания прибавляет величиной
    /// (`block_layout_algorithm.cc:1734-1745`, `available_inline_size` может
    /// быть нулём), а сторону — отдельным `InsetBias::kEnd`
    /// (`absolute_utils.cc:34`).
    pub block_strut: bool,
    /// Выравнивание в ПРЯМОУГОЛЬНИКЕ СТАТИЧЕСКОЙ ПОЗИЦИИ (css-position-3
    /// §static-position-rectangle; css-align-3 §justify-abspos,
    /// §align-abspos): физические доли `(x, y)`. По строчной оси прямоугольник
    /// — дырка блочной распорки (края содержимого родителя), коробка встаёт
    /// в долю `x` свободного места; по блочной он нулевой, и коробка висит от
    /// статической точки на долю `y` своей высоты. `None` — прежний путь.
    pub self_align: Option<(f32, f32)>,
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
thread_local! {
    /// Слои НАЧАЛЬНОГО содержащего блока: внепоточные элементы, которым не
    /// нашлось позиционированного предка. По §10.1 их содержащий блок —
    /// область просмотра, а не ближайший родитель, поэтому они дописываются
    /// последними детьми документа.
    /// Пара `(SpotCell, AnyElement)`: по ПУСТОЙ оси элемент стоит на
    /// статической позиции, и её сообщает щуп с его места в потоке.
    static ICB: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// Слой БЛИЖАЙШЕГО содержащего блока: абсолютная коробка, чей родитель
    /// содержащим блоком не является, переезжает сюда.
    ///
    /// Раскладка под нами считает края абсолютной коробки от НЕПОСРЕДСТВЕННОГО
    /// родителя — понятия «позиционированный предок» у неё нет. §10.1 требует
    /// ближайшего предка с `position` не `static`, поэтому коробка собирается
    /// на своём месте, а детём становится этому предку.
    static CB: std::cell::RefCell<Vec<Vec<(SpotCell, AnyElement)>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Параллельно `CB`: содержит ли коробку слоя и `position: fixed`.
    static CB_FIXED: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Открыть слой содержащего блока вокруг детей позиционированной коробки.
pub fn cb_open() {
    cb_open_with(false);
}

/// Открыть слой содержащего блока; `fixed_cb` — коробка содержит и
/// `position: fixed` (трансформ, `contain: layout|paint`, css-transforms-1
/// §transform-rendering: «establishes a containing block for all
/// descendants»). Такой слой забирает фиксированных потомков в обход
/// промежуточных позиционированных предков (`cb_push_fixed`).
pub fn cb_open_with(fixed_cb: bool) {
    CB.with(|s| s.borrow_mut().push(Vec::new()));
    CB_FIXED.with(|s| s.borrow_mut().push(fixed_cb));
}

/// Отдать `position: fixed` слою ближайшего предка, содержащего `fixed`
/// (не ближайшего позиционированного, CSS 2.1 §10.1 п.3 + css-transforms-1).
/// Слоя нет — элемент возвращается, рисовать на месте.
pub fn cb_push_fixed(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    let at = CB_FIXED.with(|f| f.borrow().iter().rposition(|x| *x));
    match at {
        Some(i) => CB.with(|s| match s.borrow_mut().get_mut(i) {
            Some(layer) => {
                layer.push((spot, el));
                None
            }
            None => Some(el),
        }),
        None => Some(el),
    }
}

/// Забрать накопленное верхним слоем содержащего блока и закрыть его.
pub fn cb_close() -> Vec<AnyElement> {
    CB_FIXED.with(|s| s.borrow_mut().pop());
    CB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Отдать элемент слою ближайшего содержащего блока. Слоя нет — элемент
/// возвращается, рисовать на месте.
pub fn cb_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    CB.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

/// Открыть слой ICB: документ, блок ленты или вложенный документ.
pub fn icb_open() {
    ICB.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем ICB и закрыть его.
pub fn icb_close() -> Vec<AnyElement> {
    ICB.with(|s| s.borrow_mut().pop())
        .unwrap_or_default()
        .into_iter()
        .map(|(spot, el)| icb_place(spot, el))
        .collect()
}

/// Заместитель слоя ICB — БЕЗ обёртки, в отличие от `spot_place`.
///
/// Любая коробка вокруг стала бы для раскладки содержащим блоком абсолютного
/// ребёнка (понятия «позиционированный предок» у раскладки нет), и края
/// считались бы от неё, а не от области просмотра — то есть ровно то, ради
/// чего затеян вынос. `LatePlace` своей коробки не заводит: он отдаёт
/// `layout_id` ребёнка.
fn icb_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    LatePlace {
        child: Some(child),
        spot,
    }
    .into_any_element()
}

/// Открыт ли слой начального содержащего блока.
///
/// Поддерево ленты прокрутки строится в замыкании, а зовёт его
/// `ScrollArea::request_layout` — уже ПОСЛЕ `icb_close()`. Вынимать оттуда
/// кандидатов можно только пока слой ещё есть.
pub fn icb_active() -> bool {
    ICB.with(|s| !s.borrow().is_empty())
}

/// Отдать элемент слою ICB. Слоя нет — элемент возвращается, рисовать на
/// месте.
pub fn icb_push(spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
    ICB.with(|s| match s.borrow_mut().last_mut() {
        Some(layer) => {
            layer.push((spot, el));
            None
        }
        None => Some(el),
    })
}

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

/// Есть ли в открытом верхнем слое накопленное содержимое. Спрашивает цикл
/// детей контейнера перед позиционированным соседом: слой выпускается
/// раньше него, чтобы абсолют на статической позиции красился в порядке
/// дерева (CSS 2.1 прил. E, шаг 8), а не поверх всех позиционированных
/// соседей после него.
pub fn late_pending() -> bool {
    LATE.with(|s| s.borrow().last().is_some_and(|layer| !layer.is_empty()))
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
    if full && spot.get().vertical {
        // Вертикальное письмо: блочный поток горизонтален (контейнер — ряд),
        // распорка встаёт колонкой нулевой ширины на месте элемента в потоке.
        probe = probe.w_0().h_full();
    } else if full {
        probe = probe.w_full();
    } else {
        // Внутри строки распорка прижимается к её ВЕРХУ: на базовой линии
        // содержимое уезжало бы под строку.
        probe = probe.w_0();
        probe.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    spot_geometry::probe(spot, full, probe.into_any_element())
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
    // Спан с СОБСТВЕННЫМ вертикальным письмом в горизонтальном содержащем
    // блоке и с заданной осью: нулевая обёртка вставала после абзаца и сама
    // становилась точкой отсчёта (`abs-pos-non-replaced-vlr-121`: зелёный
    // на y = 0 при эталоне 160). Голый заместитель считает сдвиг от дырки
    // щупа сам; `!vertical` обязателен — вертикальные классы с обёрткой
    // зелёные (`vlr-087`, `vlr-119`). Разбор:
    // `target/scout-vabs-stretch-2026-09.md`, часть 5.
    // ★ ЗАМЕРЕНО: снятие `own_vertical` целиком (коммит 26b36d3) взяло 26
    // пар `abs-pos-non-replaced-v*`, но обвалило четыре
    // `absolute-replaced-height-012/019/026/033` с 0.02 в «красное видно»:
    // абсолютный `<iframe>` с процентной высотой — ЗАМЕЩАЕМЫЙ, у него
    // §10.6.2, а не §10.6.4. Признак содержащего блока (`ortho_limit`) в
    // роли различителя ЗАМЕРЕН И ОТКАЧЕН: он истинен в обоих случаях,
    // срез 480 пар дал +0/−28. Разводит именно замещаемость.
    if now.fixed_axes != (false, false)
        && !now.vertical
        && (now.own_vertical || !now.replaced)
    {
        return LatePlace {
            child: Some(child),
            spot,
        }
        .into_any_element();
    }
    // При вертикальном письме поток строк идёт поперёк: распорка тянется по
    // высоте, а не по ширине, иначе она уводила бы содержимое вниз. Ветка
    // нужна только позиции В СТРОКЕ (next_line): блочный заместитель без неё
    // стоит в горизонтальном потоке блоков и с вертикальной обёрткой уезжал
    // за край ячейки.
    let vertical_flow = now.vertical && now.next_line.is_some();
    // Блочный заместитель в вертикальном письме: контейнер — ряд, любая
    // распорка с размером двигала бы соседей. Обёртка нулевая, положение
    // целиком считает сдвиг заместителя (LatePlace).
    if now.vertical && now.next_line.is_none() {
        return gpui::div()
            .w_0()
            .h_0()
            .flex_shrink_0()
            .child(LatePlace {
                child: Some(child),
                spot,
            })
            .into_any_element();
    }
    let mut wrap = if vertical_flow {
        // Распорка НУЛЕВАЯ и по строчной оси тоже: при вертикальном письме
        // `h_full` отдавал контейнеру ДО-ПОВОРОТНУЮ высоту, равную
        // собственной ширине абсолюта, и коробка контейнера росла ровно на
        // неё (замерено на голой пробе: рамка 122.4 CSS против 42.4 у
        // эталона, излишек 80 = ширина абсолюта). Место абсолюта считает
        // сдвиг (`LatePlace`), распорке размер не нужен.
        gpui::div().h_0().w_0().flex_shrink_0().flex().flex_col()
    } else {
        gpui::div().w_full().h_0().flex_shrink_0().flex().flex_row()
    };
    wrap = wrap.items_start();
    // Начало строчной оси — правый край: при письме справа налево в
    // горизонтальном режиме и при vertical-rl (блочный поток идёт от правого
    // края, css-writing-modes §block-flow). У vertical-lr `direction`
    // строчную ось держит вертикальной, горизонталь остаётся левой.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): якорь по `direction` и в вертикальном
    // письме (css-writing-modes-4 §7.1: `direction` ведёт строчную ось) плюс
    // rtl-ветки `LatePlace` с отражением по y для вертикального контейнера —
    // css-writing-modes 570 -> 570, L-a12 408 -> 408: ни одна пара не
    // сдвинулась. Класс «rtl в вертикальном письме» (12 пар, 2.67) держится
    // не на якоре, а на статической точке rtl-строки (корень B части 5
    // `target/scout-vabs-stretch-2026-09.md`).
    let anchor_end = if now.vertical {
        now.vertical_rl
    } else {
        now.rtl
    };
    wrap = if anchor_end {
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
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        let now = self.spot.get();
        let shift = match (now.hole, now.next_line) {
            // Блочный: слева — край содержимого родителя (там же, где стоит сам
            // заместитель), сверху — низ строки, в которой он оказался.
            // Поперёк потока строк заместитель уже стоит у нужного края (его
            // ставит распорка ряда), поэтому правится только та ось, вдоль
            // которой идут строки.
            (Some(hole), Some(line)) if now.vertical && now.vertical_rl => {
                gpui::point(hole.origin.x - px(line) - bounds.origin.x, px(0.0))
            }
            (Some(hole), Some(line)) if now.vertical => {
                gpui::point(hole.origin.x + px(line) - bounds.origin.x, px(0.0))
            }
            (Some(hole), Some(line)) => {
                gpui::point(px(0.0), hole.origin.y + px(line) - bounds.origin.y)
            }
            // Начало строчной оси при письме справа налево — правый край:
            // поперёк потока заместителя уже выровняла обёртка (justify_end /
            // items_end), правится только ось потока строк.
            // Блочный заместитель в вертикальном письме: начало блочного
            // потока при vertical-rl — ПРАВЫЙ край, коробка уходит от него
            // влево (css-writing-modes §block-flow).
            (Some(hole), None) if now.vertical => {
                let x = if now.vertical_rl {
                    hole.origin.x - bounds.size.width
                } else {
                    hole.origin.x
                };
                gpui::point(x - bounds.origin.x, hole.origin.y - bounds.origin.y)
            }
            // Письмо справа налево: заместитель вешается ПРАВЫМ краем на
            // начало строчной оси (CSS 2.1 §10.3.7: статическая позиция в rtl
            // отсчитывается от правого края). У блочной распорки во всю
            // ширину правый край — правый край содержимого, у точечной в
            // строке — сама точка.
            // Правокрайняя формула верна для БЛОЧНОЙ распорки во всю
            // ширину; у ТОЧЕЧНОГО щупа в строке ширина нулевая, и вычитание
            // своей ширины уводило коробку влево на неё целиком
            // (htb-rtl-*: регресс 08-19).
            // ЗАМЕРЕНО, ЭФФЕКТА НЕТ (03.09): в вертикальном письме отсчитывать
            // `rtl` от НИЖНЕГО края дырки, а не от правого (css-writing-modes-3
            // §7.1: `direction` переворачивает строчную ось, а она вертикальна).
            // Правка рассуждением верна, но на двенадцати пробах `apw-*` не
            // сдвинула НИ ОДНОЙ сотой, и сборка HEAD без неё даёт те же числа.
            // Прежняя запись «apw-l 2.67 -> 0.00» была ЛОЖНОЙ: список пар
            // собирался конкатенацией, `"\a"` давал байт BEL, обе стороны не
            // грузились, и пустая страница сходилась с пустой. Списки строить
            // только через `Path`, проверять `od -c`.
            // Возвращать вместе с независимым решением осей (корень B): до
            // боевых пар этот рукав просто не доезжает.
            // Повёрнутый абзац: дырка уже экранная (`vt_map`). Строчная ось
            // здесь вертикальна, и при `direction: rtl` она идёт снизу вверх
            // — щуп отмечает НИЖНИЙ край коробки, а не верхний
            // (`abs-pos-non-replaced-vrl-126`: коробка стояла ровно на свою
            // высоту ниже нужного). Горизонтальные rtl-рукава ниже к такой
            // дырке не относятся — они сдвигают по x.
            (Some(hole), None) if now.rotated && now.rtl => gpui::point(
                hole.origin.x - bounds.origin.x,
                hole.origin.y - bounds.size.height - bounds.origin.y,
            ),
            (Some(hole), None) if now.rotated => hole.origin - bounds.origin,
            // Гейт — ПОРОДА щупа, а не размер дырки. Полоса выравнивания
            // может быть нулевой (содержащий блок нулевой ширины), но сторона
            // отсчёта от этого не меняется: §10.3.7 при rtl вешает на
            // статическую точку `right`, а не `left`. Blink разводит эти два
            // шага явно — прибавка `available_inline_size` (может быть нулём,
            // `block_layout_algorithm.cc:1740`) и `InsetBias::kEnd`
            // (`absolute_utils.cc:34`).
            // Прямоугольник статической позиции с `justify-self`/`align-self`
            // (`Spot::self_align`): правило выше строк rtl — сторону уже
            // посчитал `render::static_self_align` (при rtl `start` — доля 1,
            // та же формула, что у рукава ниже) (`align-self-static-position-
            // 001/006/008`, `justify-self-static-position-001`).
            (Some(hole), None) if now.block_strut && now.self_align.is_some() => {
                let (kx, ky) = now.self_align.unwrap_or_default();
                gpui::point(
                    hole.origin.x + (hole.size.width - bounds.size.width) * kx - bounds.origin.x,
                    hole.origin.y - bounds.size.height * ky - bounds.origin.y,
                )
            }
            (Some(hole), None) if now.rtl && now.block_strut => gpui::point(
                hole.origin.x + hole.size.width * now.line_align.unwrap_or(1.0)
                    - bounds.size.width
                    - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            // Статическая позиция по СВОБОДНОЙ строчной оси при `direction:
            // rtl` содержащего блока: §10.3.7 вешает на точку не `left`, а
            // `right`, то есть коробка стоит на ней ПРАВЫМ краем и растёт
            // назад (Blink: `static_position.h: ConvertToLogical` даёт
            // `kInlineEnd`, `absolute_utils.cc: GetStaticPositionInsetBias`
            // переводит его в `InsetBias::kEnd`).
            //
            // Гейт — набор осей, а не одно `now.rtl`. `(false, true)` ставят
            // РОВНО две ветки `atom_element` с `x_set != y_set`
            // (замещаемая и незамещаемая коробка с заданным только `top`
            // и/или `bottom`), и обе уходят `Piece::Overlay`: абзац остаётся
            // ТЕКСТОВЫМ, а дырку щупу даёт `lines.rs: point_of`. Замер по
            // снимкам всех двенадцати боевых пар
            // `abs-pos-non-replaced-v{lr,rl}-{128,129,160,161,176,177,192,
            // 193,208,209,224,225}`: зелёный квадрат стоит на x 168.0..248.0
            // при эталонных 88.0..168.0 — расхождение РОВНО в свою ширину 80
            // и ни в чём больше (вне двух квадратов разошедшихся точек 0).
            //
            // Чисто статическая коробка `(false, false)` сюда НЕ пускается
            // намеренно: её абзац уходит `Piece::Atom` в гибкий ряд
            // (`inline.rs: as_wrapped_row`), ряд при `rtl` не разворачивается,
            // и щуп садится в его конец — замер тех же снимков даёт x = 328.0
            // (правый край содержащего блока) при верных 168.0. Вычитание
            // своей ширины там сложило бы вторую ошибку с первой — это и есть
            // откат 08-19 из комментария выше. Блочный щуп во всю ширину
            // забирает рукав ВЫШЕ (`hole.size.width > 0`), поэтому семья
            // `css-position/static-position/htb-*` не задета.
            (Some(hole), None) if now.rtl && now.fixed_axes == (false, true) => gpui::point(
                hole.origin.x - bounds.size.width - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            (Some(hole), None) if now.rtl => gpui::point(
                hole.origin.x - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            // Ширину коробки прибавляет только БЛОЧНЫЙ щуп
            // (`abs-pos-border-offset-003`: ортогональный `.parent` прижат к
            // правому краю vrl-контейнера); строчный щуп 0×0 сдвига не
            // получает.
            // Прижим к ПРАВОМУ краю дырки верен только когда сторону задаёт
            // `direction: rtl` САМОГО КОНТЕЙНЕРА (`now.rtl` — направление
            // потока, а не собственное письмо коробки: css-writing-modes-4
            // §7.1, Blink `absolute_utils.cc` берёт сторону у
            // `container_writing_direction`). При `direction: ltr` статическая
            // позиция — левый край дырки без добавки (рукав ниже).
            (Some(hole), None)
                if now.own_vertical && !now.vertical && now.rtl && hole.size.width > px(0.0) =>
            {
                gpui::point(
                    hole.origin.x + bounds.size.width - bounds.origin.x,
                    hole.origin.y - bounds.origin.y,
                )
            }
            // Строка выровнена не по началу: статическая точка едет вдоль неё
            // на долю `line_align` (ltr вешает на неё ЛЕВЫЙ край коробки).
            (Some(hole), None) => gpui::point(
                hole.origin.x + hole.size.width * now.line_align.unwrap_or(0.0)
                    - bounds.origin.x,
                hole.origin.y - bounds.origin.y,
            ),
            (None, _) => gpui::point(px(0.0), px(0.0)),
        };
        // Ось, которую задал содержащий блок, раскладка уже разрешила — щуп
        // её не трогает; по свободной оси к дырке добавляется поле (§10.3.7:
        // от статической позиции коробку отодвигает `margin`).
        let shift = gpui::point(
            if now.fixed_axes.0 {
                px(0.0)
            } else {
                shift.x + px(now.free_margin.0)
            },
            if now.fixed_axes.1 {
                px(0.0)
            } else {
                shift.y + px(now.free_margin.1)
            },
        );
        let child = self.child.as_mut().unwrap();
        window.set_layout_placed_origin(*layout_id, bounds.origin + shift);
        child.prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Заместитель своего контекста краски не заводит (сдвиг — в
        // подготовке): собиратель шага 8 (`gpui::PaintLast`) проходит его
        // насквозь, как обычную коробку.
        let child = self.child.as_mut().unwrap();
        gpui::paint_reopen(|| child.paint(window, cx));
    }
}

impl IntoElement for LatePlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Коробка, повешенная на точку своим строчным НАЧАЛОМ.
///
/// Текстовый путь абзаца кладёт кусок вне потока ЛЕВЫМ верхним углом на точку
/// (`lines.rs: point_of` -> `prepaint_at`). При `direction: rtl` строчное
/// начало — ПРАВЫЙ край (CSS 2.1 §10.3.7: «otherwise, set 'right' to the
/// static position»; css-position-3 §staticpos-rect: прямоугольник
/// статической позиции «positioned at its inline-start static position»),
/// поэтому коробку надо сдвинуть назад ровно на свою ширину. Обёртка
/// возвращает раскладке `layout_id` ребёнка, то есть в потоке ничего не
/// добавляет и ничего не меряет заново.
///
/// Blink: `geometry/static_position.h:86` даёт `kInlineEnd` при `!IsLtr()`,
/// `absolute_utils.cc:27-37 GetStaticPositionInsetBias` переводит его в
/// `InsetBias::kEnd`.
///
/// Поля в сдвиг не входят — ровно как и у ltr-ветки, где на точку садится край
/// РАМОЧНОЙ коробки, а не отбивочной. Обе стороны считаются одинаково, и
/// разница проявилась бы только у абсолюта с ненулевым боковым полем.
pub struct InlineStartHang {
    child: Option<AnyElement>,
}

impl InlineStartHang {
    pub fn new(child: AnyElement) -> Self {
        Self { child: Some(child) }
    }
}

impl Element for InlineStartHang {
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
        let child = self.child.as_mut().unwrap();
        let shift = gpui::point(px(0.0) - bounds.size.width, px(0.0));
        window.with_exact_element_offset(shift, |window| child.prepaint(window, cx));
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

impl IntoElement for InlineStartHang {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Маска обрезающего предка для ОТЛОЖЕННОГО слоя (`z-index > 0`).
///
/// GPUI рисует отложенные элементы после всего дерева и восстанавливает им
/// стек id и стилей, но не маску содержимого: `overflow: hidden/clip`
/// предка их не режет (`overflow-clip-margin-011..022`). Наружная обёртка
/// стоит в потоке и на `prepaint` запоминает текущую маску, внутренняя —
/// внутри `deferred` — рисует ребёнка под ней. Фиксированному слою маска
/// не нужна: его содержащий блок — окно.
pub type MaskCell = std::rc::Rc<std::cell::RefCell<Option<gpui::ContentMask<Pixels>>>>;

pub struct MaskKeep {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl IntoElement for MaskKeep {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for MaskKeep {
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
        (self.child.request_layout(window, cx), ())
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
        *self.cell.borrow_mut() = Some(window.content_mask());
        self.child.prepaint(window, cx);
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
        self.child.paint(window, cx);
    }
}

pub struct MaskUse {
    pub cell: MaskCell,
    pub child: AnyElement,
}

impl IntoElement for MaskUse {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for MaskUse {
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
        (self.child.request_layout(window, cx), ())
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
        let mask = self.cell.borrow().clone();
        window.with_content_mask(mask, |window| self.child.prepaint(window, cx));
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
        let mask = self.cell.borrow().clone();
        window.with_content_mask(mask, |window| self.child.paint(window, cx));
    }
}

/// Слой `filter: url(#id)`: SVG с прямоугольником цвета фона под этим
/// фильтром, растрированный resvg по размеру коробки. Холст вдвое больше
/// коробки, прямоугольник в центре — область фильтра по умолчанию выходит
/// за коробку на десятую часть с каждой стороны, заданная — сколько
/// угодно.
pub struct FilterLayer {
    pub def: String,
    pub id: String,
    pub fill: String,
}

impl IntoElement for FilterLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

thread_local! {
    /// Растры слоёв фильтра по (разметка, размер): кадр за кадром одно и то же.
    static FILTER_RASTERS: std::cell::RefCell<
        std::collections::HashMap<(String, u32, u32), std::sync::Arc<gpui::RenderImage>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Область `<filter filterUnits="userSpaceOnUse">` в точках от угла коробки
/// (filter-effects-1 §5: «origin at the top-left corner of the bounding
/// client rect … viewport … width and height of the bounding client rect»);
/// проценты — от размера коробки. Для `objectBoundingBox` — None: такая
/// область и так внутри холста −50 %…150 %.
fn user_space_region(def: &str, w: f32, h: f32) -> Option<[f32; 4]> {
    let open = &def[def.find("<filter")?..];
    let open = &open[..open.find('>')?];
    if !open.contains("userSpaceOnUse") {
        return None;
    }
    let attr = |name: &str| -> Option<&str> {
        let key = format!(" {name}=\"");
        let at = open.find(&key)? + key.len();
        let rest = &open[at..];
        Some(&rest[..rest.find('"')?])
    };
    let num = |name: &str, base: f32, def: f32| -> f32 {
        match attr(name).map(str::trim) {
            Some(v) if v.ends_with('%') => v[..v.len() - 1]
                .trim()
                .parse::<f32>()
                .map_or(def, |p| p / 100.0 * base),
            Some(v) => v.trim_end_matches("px").trim().parse().unwrap_or(def),
            None => def,
        }
    };
    Some([
        num("x", w, -0.1 * w),
        num("y", h, -0.1 * h),
        num("width", w, 1.2 * w),
        num("height", h, 1.2 * h),
    ])
}

impl Element for FilterLayer {
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
        style.inset = gpui::Edges {
            top: gpui::px(0.0).into(),
            right: gpui::px(0.0).into(),
            bottom: gpui::px(0.0).into(),
            left: gpui::px(0.0).into(),
        };
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
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if w < 0.0 || h < 0.0 {
            return;
        }
        // Холст в системе коробки: полкоробки запаса с каждой стороны
        // покрывают умолчание −10 %/120 %, а область `userSpaceOnUse` задана
        // от угла коробки и может лежать где угодно — холст расширяется до
        // неё (у коробки 0×0 он из неё одной и состоит,
        // `empty-element-with-filter-002/004`).
        let (mut x0, mut y0, mut x1, mut y1) = (-0.5 * w, -0.5 * h, 1.5 * w, 1.5 * h);
        if let Some([rx, ry, rw, rh]) = user_space_region(&self.def, w, h) {
            x0 = x0.min(rx).max(-4096.0);
            y0 = y0.min(ry).max(-4096.0);
            x1 = x1.max(rx + rw).min(4096.0);
            y1 = y1.max(ry + rh).min(4096.0);
        }
        // Края холста — на точках устройства: растр ложится один к одному,
        // без пересэмплирования. Полкоробки запаса при масштабе 1.25 давали
        // дробный угол (−62.5 точки), и вся заливка расплывалась на
        // полточки (`filter-chained-url-url-001`, `svg-feimage-005`).
        let sf = window.scale_factor();
        let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let x0 = ((ox + x0) * sf).floor() / sf - ox;
        let y0 = ((oy + y0) * sf).floor() / sf - oy;
        let x1 = ((ox + x1) * sf).ceil() / sf - ox;
        let y1 = ((oy + y1) * sf).ceil() / sf - oy;
        let (cw, ch) = (x1 - x0, y1 - y0);
        if cw <= 0.0 || ch <= 0.0 {
            return;
        }
        // Начало пользовательских координат — угол коробки (сдвиг группы).
        // У коробки 0×0 фильтр — на группе: `<rect>` нулевого размера
        // выключает отрисовку (SVG 2 §10.2), а `feFlood` даёт выход и без
        // источника.
        let body = if w > 0.0 && h > 0.0 {
            format!(
                r##"<rect width="{w}" height="{h}" fill="{}" filter="url(#{})"/>"##,
                self.fill, self.id
            )
        } else {
            format!(r##"<g filter="url(#{})"/>"##, self.id)
        };
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}" viewBox="0 0 {cw} {ch}"><defs>{}</defs><g transform="translate({} {})">{body}</g></svg>"##,
            self.def, -x0, -y0
        );
        let (pw, ph) = ((cw * sf).round(), (ch * sf).round());
        let key = (markup.clone(), pw as u32, ph as u32);
        let image = FILTER_RASTERS.with(|m| m.borrow().get(&key).cloned()).or_else(|| {
            let img = crate::svg::rasterize(&markup, pw, ph)?;
            FILTER_RASTERS.with(|m| m.borrow_mut().insert(key.clone(), img.clone()));
            Some(img)
        });
        let Some(image) = image else { return };
        let area = Bounds {
            origin: gpui::point(bounds.origin.x + px(x0), bounds.origin.y + px(y0)),
            size: gpui::size(px(cw), px(ch)),
        };
        let _ = window.paint_image_with_sampling(area, gpui::Corners::default(), image, 0, false, gpui::ImageSampling::Linear);
    }
}
