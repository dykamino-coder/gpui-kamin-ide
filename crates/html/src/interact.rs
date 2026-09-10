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
    /// `mask-mode: luminance` — гасит светимостью, а не альфой.
    pub mask_luminance: bool,
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
    /// `mask-composite` по слоям: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub mask_composite: Vec<u8>,
}

impl Grouped {
    pub fn new(child: AnyElement) -> Self {
        Grouped {
            child: Some(child),
            blur: 0.0,
            opacity: 1.0,
            blend: 0,
            polygon: Vec::new(),
            polygon_evenodd: false,
            poly_expand: [0.0; 4],
            mask: None,
            mask_size: None,
            mask_fit: 0,
            mask_no_repeat: (false, false),
            mask_repeat_list: Vec::new(),
            mask_luminance: false,
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
        // Вершины считаются от ОПОРНОЙ коробки формы (bounds ± края:
        // margin-box шире, content-box уже); проценты — доли её сторон,
        // точки — как есть (clip-path-polygon-008).
        let [et, er, eb, el] = self.poly_expand;
        let base = Bounds {
            origin: gpui::point(bounds.origin.x - px(el), bounds.origin.y - px(et)),
            size: gpui::size(
                bounds.size.width + px(el + er),
                bounds.size.height + px(et + eb),
            ),
        };
        let coord = |l: crate::value::Len, side: Pixels| -> Pixels {
            match l {
                crate::value::Len::Pct(p) => side * p,
                crate::value::Len::Px(v) => px(v),
                _ => px(0.0),
            }
        };
        let polygon: Vec<gpui::Point<Pixels>> = self
            .polygon
            .iter()
            .map(|(fx, fy)| {
                gpui::point(
                    base.origin.x + coord(*fx, base.size.width),
                    base.origin.y + coord(*fy, base.size.height),
                )
            })
            .collect();
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
            if layers.len() > 1 || referenced {
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
                        // Ссылка на определение в документе: растр под
                        // коробку, светимость вместо альфы.
                        let (image, tile, lum) = if let Some(id) = l.strip_prefix("svgsnap:") {
                            (
                                rasterize_mask_def(id, bw, bh, false)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else if let Some(id) = l.strip_prefix("clipsnap:") {
                            (
                                rasterize_mask_def(id, bw, bh, true)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else if let Some((file, frag)) =
                            l.rsplit_once('#').filter(|(f, _)| f.ends_with(".svg"))
                        {
                            // Маска из внешнего рисунка (`url(file.svg#id)`).
                            let markup = svg_fragment(file, frag)?
                                .replace("clip-rule", "fill-rule");
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
                            let intr = source.intrinsic();
                            let (tw, th) = (intr.w.unwrap_or(bw), intr.h.unwrap_or(bh));
                            // Плитка кладётся не в угол, а в точку СВОЕГО
                            // слоя: без этого `mask-position: top, bottom`
                            // сваливал оба слоя в (0,0) (mask-position-5).
                            let (ox, oy) = pos_of(i, tw, th);
                            (
                                source.raster((tw, th))?,
                                [ox * sf, oy * sf, tw * sf, th * sf],
                                false,
                            )
                        };
                        Some(crate::background::MaskLayer {
                            image,
                            tile,
                            no_repeat: repeat_of(i),
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
                let img = crate::background::compose_mask_layers(&built, cw, ch)?;
                return Some((
                    img,
                    Bounds {
                        origin: gpui::point(bounds.origin.x + px(ol), bounds.origin.y + px(ot)),
                        size: gpui::size(px(bw), px(bh)),
                    },
                    // Светимость уже учтена при сборке полотна.
                    3,
                ));
            }
            let source = crate::background::source(src)?;
            let (img, tw, th) = match &source {
                crate::background::Source::Raster(img) => {
                    let s = img.size(0);
                    let (iw, ih) = (s.width.0 as f32, s.height.0 as f32);
                    // contain/cover: один множитель от пропорции интринзика.
                    let k = match self.mask_fit {
                        1 => Some((bw / iw.max(1.0)).min(bh / ih.max(1.0))),
                        2 => Some((bw / iw.max(1.0)).max(bh / ih.max(1.0))),
                        _ => None,
                    };
                    match (k, self.mask_size) {
                        (Some(k), _) => (img.clone(), iw * k, ih * k),
                        (None, Some((x, y))) => (img.clone(), len(x, bw, iw), len(y, bh, ih)),
                        (None, None) => (img.clone(), iw, ih),
                    }
                }
                _ => {
                    // У рисунка может быть свой размер — contain/cover
                    // считаются от него; без интринзика плитка = коробка.
                    let intr = source.intrinsic();
                    let fit = match (self.mask_fit, intr.w, intr.h) {
                        (1, Some(iw), Some(ih)) => {
                            let k = (bw / iw.max(1.0)).min(bh / ih.max(1.0));
                            Some((iw * k, ih * k))
                        }
                        (2, Some(iw), Some(ih)) => {
                            let k = (bw / iw.max(1.0)).max(bh / ih.max(1.0));
                            Some((iw * k, ih * k))
                        }
                        _ => None,
                    };
                    let (tw, th) = match (fit, self.mask_size) {
                        (Some(t), _) => t,
                        // `auto` в паре (`auto 50px`) — интринзик своей оси.
                        (None, Some((x, y))) => (
                            len(x, bw, intr.w.unwrap_or(bw)),
                            len(y, bh, intr.h.unwrap_or(bh)),
                        ),
                        // auto: у рисунка со своим размером плитка — он
                        // (mask-repeat-1: свг 50x50 мостится по коробке),
                        // без интринзика — коробка.
                        (None, None) => (
                            intr.w.unwrap_or(bw),
                            intr.h.unwrap_or(bh),
                        ),
                    };
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
                        _ => source.raster((tw, th))?,
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
                Bounds {
                    origin: gpui::point(
                        bounds.origin.x + px(ol + ox),
                        bounds.origin.y + px(ot + oy),
                    ),
                    size: gpui::size(px(tw.max(1.0)), px(th.max(1.0))),
                },
                ((self.mask_no_repeat.0 as u32)
                    | ((self.mask_no_repeat.1 as u32) << 1)
                    | ((self.mask_luminance as u32) << 2)),
            ))
        });
        // Коробка окраски (`mask-clip`): вне её маска не красится — элемент
        // там скрыт (mask-size-contain-clip-padding).
        let mask_clip = self
            .mask_clip_off
            .map(|[ct, cr, cb, cl]| {
                [
                    cl,
                    ct,
                    (f32::from(bounds.size.width) - cl - cr).max(0.0),
                    (f32::from(bounds.size.height) - ct - cb).max(0.0),
                ]
            })
            .or_else(|| {
                // `clip-path: inset(...)` — срезы краёв (css-shapes-1).
                self.clip_inset.map(|[t, r, b, l]| {
                    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let side = |v: crate::value::Len, s: f32| match v {
                        crate::value::Len::Px(p) => p,
                        crate::value::Len::Pct(p) => p * s,
                        _ => 0.0,
                    };
                    let (t, b) = (side(t, bh), side(b, bh));
                    let (l, r) = (side(l, bw), side(r, bw));
                    [l, t, (bw - l - r).max(0.0), (bh - t - b).max(0.0)]
                })
            })
            .or_else(|| {
                // `clip-path: rect(...)` — края от верхнего-левого угла.
                self.clip_edges.map(|[t, r, b, l]| {
                    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let side = |v: Option<crate::value::Len>, s: f32, def: f32| match v {
                        Some(crate::value::Len::Px(p)) => p,
                        Some(crate::value::Len::Pct(p)) => p * s,
                        _ => def,
                    };
                    let (t, b) = (side(t, bh, 0.0), side(b, bh, bh));
                    let (l, r) = (side(l, bw, 0.0), side(r, bw, bw));
                    [l, t, (r - l).max(0.0), (b - t).max(0.0)]
                })
            })
            .or_else(|| {
                // `clip-path: xywh(x y w h)` — прямоугольник от угла.
                self.clip_xywh.map(|[x, y, w, h]| {
                    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let side = |v: crate::value::Len, s: f32| match v {
                        crate::value::Len::Px(p) => p,
                        crate::value::Len::Pct(p) => p * s,
                        _ => 0.0,
                    };
                    [
                        side(x, bw),
                        side(y, bh),
                        side(w, bw).max(0.0),
                        side(h, bh).max(0.0),
                    ]
                })
            })
            .or_else(|| {
                // `clip: rect(t r b l)` — координаты краёв видимой области
                // от углов коробки; auto — её край (clip-rect-auto-*).
                self.clip_rect.map(|[t, r, b, l]| {
                    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let (t, l) = (t.unwrap_or(0.0), l.unwrap_or(0.0));
                    let (r, b) = (r.unwrap_or(bw), b.unwrap_or(bh));
                    if r <= l || b <= t {
                        // Вырожденная область — элемент скрыт ЦЕЛИКОМ:
                        // коробка клипа уводится за экран
                        // (clip-negative-values-001: right < left).
                        return [-1.0e7, -1.0e7, 1.0, 1.0];
                    }
                    [l, t, r - l, b - t]
                })
            })
            .map(|[x, y, w, h]| {
                let sf = window.scale_factor();
                let (dx, dy) = (
                    self.clip_shift.0 + f32::from(bounds.size.width) * self.clip_shift.2,
                    self.clip_shift.1 + f32::from(bounds.size.height) * self.clip_shift.3,
                );
                [
                    (f32::from(bounds.origin.x) + x + dx) * sf,
                    (f32::from(bounds.origin.y) + y + dy) * sf,
                    w * sf,
                    h * sf,
                ]
            });
        let child = self.child.as_mut().unwrap();
        window.paint_group(
            area,
            gpui::Corners::default(),
            self.blur,
            self.opacity,
            self.blend,
            &polygon,
            mask,
            mask_clip,
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
}

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
        // Доля × размер ПЛЮС точки: `calc(50% + 10px)` — смесь, и доля у
        // чистых точек равна нулю (css-transforms-1 §5.2).
        let ox = f32::from(bounds.size.width) * self.origin.0 + self.origin_px.0.unwrap_or(0.0);
        let oy = f32::from(bounds.size.height) * self.origin.1 + self.origin_px.1.unwrap_or(0.0);
        let origin = gpui::point(
            dev(f32::from(bounds.origin.x) + ox),
            dev(f32::from(bounds.origin.y) + oy),
        );
        let back = gpui::point(
            dev(-(f32::from(bounds.origin.x) + ox)),
            dev(-(f32::from(bounds.origin.y) + oy)),
        );
        // Матрица функций в порядке записи (css-transforms-1
        // §transform-rendering), вокруг точки отсчёта: она уводится в ноль и
        // возвращается. Проценты сдвига считаются от собственного размера —
        // он известен только здесь, на отрисовке.
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let shift = |row: [f32; 3]| (row[0] + w * row[1] + h * row[2]) * scale_factor;
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
                (f32::from(bounds.origin.x) + px) * scale_factor,
                (f32::from(bounds.origin.y) + py) * scale_factor,
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
            // Плоский путь — прежний, байт в байт.
            let matrix = gpui::TransformationMatrix::unit()
                .translate(origin)
                .compose(gpui::TransformationMatrix {
                    rotation_scale: self.lin,
                    translation: [shift(self.tr[0]), shift(self.tr[1])],
                })
                .translate(back);
            let child = self.child.as_mut().unwrap();
            window.with_transformation(matrix, |window| child.paint(window, cx));
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
            own[i][3] = (own[i][3] + w * self.m4_pct[i][0] + h * self.m4_pct[i][1]) * sf;
        }
        for j in 0..3 {
            own[3][j] /= sf;
        }
        // Точка отсчёта по трём осям, в точках устройства: T(o)·M·T(−o).
        // `ox`/`oy` посчитаны выше в css-точках; `ScaledPixels.0` — pub(crate)
        // в gpui, поэтому `origin.x.0` отсюда не читается.
        let oz = self.origin_z.unwrap_or(0.0) * sf;
        let (ox_d, oy_d) = (
            (f32::from(bounds.origin.x) + ox) * sf,
            (f32::from(bounds.origin.y) + oy) * sf,
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
            (f32::from(bounds.origin.x) + w * 0.5) * sf,
            (f32::from(bounds.origin.y) + h * 0.5) * sf,
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
        // Ребро (`rotateX(90deg)`) — не рисуется, как и прежняя нулевая
        // высота. Но в объёмном контексте ПОТОМКИ ребром не становятся
        // (transform3d-preserve3d-011: `rotateX(90)` над `rotateX(90)` =
        // 180°): краска идёт под единичной долей, а место каждый потомок
        // назначает себе сам по накопленной.
        let Some(flat) = flat else {
            if self.frame_3d.is_some() {
                let child = self.child.as_mut().unwrap();
                window.with_transformation(gpui::TransformationMatrix::unit(), |window| {
                    child.paint(window, cx)
                });
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
        window.with_transformation(flat, |window| child.paint(window, cx));
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
        let exact: Vec<Bounds<Pixels>> =
            rects.iter().filter(|(_, e)| *e).map(|(b, _)| *b).collect();
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
                origin: gpui::point(
                    area.origin.x + gpui::px(sh.x),
                    area.origin.y + gpui::px(sh.y),
                ),
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
pub fn gap_item_probe(items: GapItems) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| items.borrow_mut().push(bounds),
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
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

/// Втяжка конца в точках: положительная укорачивает, отрицательная тянет
/// наружу. Доля — от ширины пересекающего зазора (0 у cap). `overlap-join`
/// (§inset) — до дальнего края поперечной линейки: половина зазора и половина
/// её ширины; у главных промежутков строк — только половина зазора (Blink,
/// `ComputeOverlapJoinInset`).
fn inset_px(
    inset: crate::computed::GapInset,
    cw: f32,
    joins: bool,
    cross_w: f32,
    main_like: bool,
) -> f32 {
    use crate::computed::GapInset;
    use crate::value::Len;
    match inset {
        GapInset::Len(Len::Px(v)) => v,
        GapInset::Len(Len::Pct(k)) => k * cw,
        GapInset::Len(_) => 0.0,
        GapInset::OverlapJoin if joins => -(cw / 2.0) - if main_like { 0.0 } else { cross_w / 2.0 },
        GapInset::OverlapJoin => 0.0,
    }
}

/// Отрезки линейки по протяжённости: вычесть скрытые участки и (кроме `none`)
/// перекрытые спанами; при `intersection` — ещё пересекающие зазоры с видимым
/// пересечением. Концы, попавшие в зазор, отступают к его границе, затем
/// прикладывается втяжка; отрезки без длины выпадают (`flex-055`).
fn segments(run: &GapRun, rule: &GapAxisRule, main_like: bool, flip: bool) -> Vec<(f32, f32)> {
    let mut parts = vec![(run.r0, run.r1)];
    for &c in &run.hidden {
        parts = subtract(parts, c);
    }
    if rule.brk != 0 {
        for &c in &run.blocked {
            parts = subtract(parts, c);
        }
    }
    if rule.brk == 2 {
        for c in run.crossings.iter().filter(|c| c.breaks) {
            parts = subtract(parts, (c.lo, c.hi));
        }
    }
    // `flip` — линейка вдоль строчной оси при `direction: rtl`: левый конец
    // отрезка — это END-сторона, правый — START (§insets-start-end).
    let (lo_cap, lo_join, hi_cap, hi_join) = if flip { (1, 3, 0, 2) } else { (0, 2, 1, 3) };
    let mut out = vec![];
    for (s, e) in parts {
        let (s, s_cw, s_join, s_dw) = run.edge(s, true);
        let (e, e_cw, e_join, e_dw) = run.edge(e, false);
        let s2 = s + inset_px(rule.inset[if s_join { lo_join } else { lo_cap }], s_cw, s_join, s_dw, main_like);
        let e2 = e - inset_px(rule.inset[if e_join { hi_join } else { hi_cap }], e_cw, e_join, e_dw, main_like);
        if e2 - s2 > 0.05 {
            out.push((s2, e2));
        }
    }
    out
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
) -> Vec<GapRun> {
    let flipped: Vec<GapItem> = items.iter().map(GapItem::flipped).collect();
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
                            && occupied(&flipped, lo, hi, side.0, side.1, c.visibility)
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
) -> (Vec<GapRun>, Vec<GapRun>) {
    let lines = tracks_a(items, gap_a);
    let r0 = items.iter().map(|i| i.b0).fold(f32::INFINITY, f32::min);
    let r1 = items.iter().map(|i| i.b1).fold(f32::NEG_INFINITY, f32::max);
    let inner: Vec<Vec<(f32, f32)>> = lines
        .iter()
        .map(|&(s, _)| {
            let mut row: Vec<&GapItem> =
                items.iter().filter(|i| (i.a0 - s).abs() <= GAP_EPS).collect();
            row.sort_by(|x, y| x.b0.partial_cmp(&y.b0).unwrap_or(std::cmp::Ordering::Equal));
            row.windows(2)
                .filter(|w| w[1].b0 - w[0].b1 >= -GAP_EPS)
                .map(|w| (w[0].b1.min(w[1].b0), w[1].b0))
                .collect()
        })
        .collect();
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
        for &(lo, hi) in &inner[k] {
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
                index: ix,
                count: cross_total.max(1),
            });
            ix += 1;
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
        let items = std::mem::take(&mut *self.items.borrow_mut());
        if items.is_empty() {
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
        match spec.kind {
            GapLayout::Grid => {
                let ix: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, true)).collect();
                let iy: Vec<GapItem> = items.iter().map(|b| GapItem::from_bounds(b, false)).collect();
                // Ось `a` прогона — та, ПОПЕРЁК которой лежит промежуток: у
                // линеек, стоящих в промежутках по x, дорожки `a` идут по x, а
                // поперечные `b` — по y; у линеек по y — наоборот.
                let (tx, ty) = (spec.tracks_x.as_deref(), spec.tracks_y.as_deref());
                if let Some(r) = on_x {
                    for run in grid_runs(&ix, spec.gap_x, spec.gap_y, r, on_y, tx, ty) {
                        layers.push((true, run, r, false));
                    }
                }
                if let Some(r) = on_y {
                    for run in grid_runs(&iy, spec.gap_y, spec.gap_x, r, on_x, ty, tx) {
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
                let gap_a = if stacked_vertically { spec.gap_y } else { spec.gap_x };
                let (mains, crosses) = line_runs(&it, gap_a, main, cross);
                if let Some(r) = main {
                    for run in mains {
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
                    // Продление В УГЛЫ только у горизонталей: пересечение
                    // иначе оставалось пустым квадратом, а продление обеих
                    // осей рисовало лишние усы на пунктирных рамках.
                    let (a, b) = if !vertical && win.style >= 9 {
                        (a - half_at(a, line), b + half_at(b, line))
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
        draw(&mut vert, true, grid_lo.map(|g| g.0));
        draw(&mut horiz, false, grid_lo.map(|g| g.1));
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
}

impl ClampCut {
    pub fn new(key: u64, lines: ClampLines, limit: Option<u32>, max_h: Option<f32>) -> Self {
        ClampCut {
            key,
            lines,
            limit,
            max_h,
        }
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
        // (верх, низ, заданная высота, нижние рамка+паддинг)
        let mut blocks: Vec<(f32, f32, bool, f32)> = vec![];
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                let n = (h / e.line).round().max(1.0) as usize;
                let step = h / n as f32;
                for i in 0..n {
                    rows.push((
                        y0 + i as f32 * step,
                        y0 + (i + 1) as f32 * step,
                        !e.skip_count,
                    ));
                }
            } else if h > 0.0 {
                blocks.push((y0, y0 + h, e.fixed_height, e.bp_after));
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
        // Блок, СОДЕРЖАЩИЙ точку среза, фрагментируется по последней
        // влезающей строке — ПРЯЧЕТСЯ целиком только коробка с заданной
        // высотой: её не фрагментировать (css-overflow-4 §line-clamp).
        // Строка, пересечённая точкой, не показывается половинкой:
        // срез поднимается к её верху.
        if let Some(c) = cut {
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
            let bp: f32 = blocks
                .iter()
                .filter(|(y0, y1, fixed, _)| !*fixed && *y0 < c2 && c2 < *y1)
                .map(|(_, _, _, bp)| *bp)
                .sum();
            c2 -= bp;
            for (y0, y1, _) in &rows {
                if *y0 < c2 && c2 < *y1 - 0.5 {
                    c2 = *y0;
                }
            }
            cut = Some(c2 + bp);
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
        let para = cut.filter(|_| self.limit.is_none()).and_then(|c| {
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
            entries
                .iter()
                .filter(|e| e.line > 0.0 && !e.skip_count)
                .filter_map(|e| {
                    let seq = e.seq?;
                    let y0 = f32::from(e.bounds.origin.y);
                    let h = f32::from(e.bounds.size.height);
                    if h <= 0.0 {
                        return None;
                    }
                    let n = (h / e.line).round().max(1.0) as usize;
                    let step = h / n as f32;
                    let k = (1..=n).filter(|i| y0 + *i as f32 * step <= c + 0.5).count();
                    (k >= 1).then_some((y0 + k as f32 * step, seq, k))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, seq, k)| (seq, k))
        });
        // Бюджет одного и того же абзаца только УЖИМАЕТСЯ: рост числа
        // строк на следующем кадре — это отражение нашей же правки, а не
        // новое измерение. Правило конечно (бюджет строго убывает и не
        // меньше единицы), поэтому кадр не может просить себя без конца.
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
        let prev = clamp_cut(self.key);
        let changed = match (prev, rel) {
            (Some(a), Some(b)) => (a - b).abs() > 4.0,
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
    gpui::canvas(
        // Запись В PREPAINT: подготовка ВСЕХ элементов идёт до отрисовки,
        // и полоса фона читает прямоугольники СВОЕГО кадра — с записью в
        // paint она рисовала прошлый кадр и мигала на каждой смене раскладки.
        move |bounds: Bounds<Pixels>, _, _| {
            // Сдвиг краски относительно коробки ячейки: в сросшейся модели
            // фоновая сетка начинается от середины рамки таблицы.
            let bounds = Bounds {
                origin: gpui::point(
                    bounds.origin.x + gpui::px(shift.0 - border[3]),
                    bounds.origin.y + gpui::px(shift.1 - border[0]),
                ),
                size: gpui::size(
                    bounds.size.width + gpui::px(border[1] + border[3]),
                    bounds.size.height + gpui::px(border[0] + border[2]),
                ),
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
        let scale_factor = window.scale_factor();
        let dev = |v: Pixels| v.scale(scale_factor);
        let cx_ = bounds.origin.x + bounds.size.width / 2.0;
        let cy_ = bounds.origin.y + bounds.size.height / 2.0;
        // Ужатие по строчной оси содержимого: длиннее кегля — в кегль.
        let sx =
            if self.compress && self.natural.width > px(self.em) && self.natural.width > px(0.0) {
                self.em / f32::from(self.natural.width)
            } else {
                1.0
            };
        // Сначала ужатие от угла куска (кусок 2-4 кегля превращается в
        // квадрат кегля), затем контр-поворот вокруг центра квадрата —
        // квадрат переходит в себя. Порядок звеньев подобран ЗАМЕРОМ:
        // скейл после поворота мял уже повёрнутые оси (плашка 160×40).
        let matrix = gpui::TransformationMatrix::unit()
            .translate(gpui::point(dev(bounds.origin.x), dev(bounds.origin.y)))
            .scale(gpui::size(sx, 1.0))
            .translate(gpui::point(
                dev(bounds.origin.x) * -1.0,
                dev(bounds.origin.y) * -1.0,
            ))
            .translate(gpui::point(dev(cx_), dev(cy_)))
            .rotate(gpui::Radians(-std::f32::consts::FRAC_PI_2))
            .translate(gpui::point(dev(cx_) * -1.0, dev(cy_) * -1.0));
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
}

/// Санитария начала кадра (`render`/`render_block`): счётчик вхождений
/// VT-ключей обнуляется, а НЕЗАКРЫТЫЕ слои позиционированных выбрасываются —
/// пойманная паника прошлого кадра оставляла слой навсегда, и `late_close`
/// следующей страницы отдавал чужие элементы.
/// Забыть двухкадровые замеры вертикальных абзацев — при смене документа:
/// ключи солятся документом, но мусор копился бы бесконечно.
pub fn forget_vt_measures() {
    VT_MEASURED.with(|c| c.borrow_mut().clear());
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
}

impl VerticalText {
    pub fn new(child: AnyElement) -> Self {
        VerticalText {
            child: Some(child),
            natural: gpui::Size::default(),
            fit_limit: None,
            claim_cap: None,
            key: None,
            ccw: false,
            col_min: false,
        }
    }

    /// Поворот против часовой стрелки (`sideways-lr`).
    pub fn counter_clockwise(mut self, on: bool) -> Self {
        self.ccw = on;
        self
    }

    /// Включить двухкадровый замер: заявка ширины уточняется фактом
    /// прошлого кадра (перенос строк меняет число колонок).
    pub fn keyed(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }

    /// Заявить и высоту — потолком родителя, только при ПОЛНОМ зажиме
    /// (строка длиннее потолка): короче потолка коробка прижимается к
    /// содержимому сама, а заявка ломала поток соседей.
    pub fn claiming_height(mut self, cap: Pixels) -> Self {
        self.claim_cap = Some(cap);
        self
    }

    /// Заявить высоту коробки, когда строка не длиннее предела: переносу
    /// такая заявка не мешает (переносить нечего), а замер становится
    /// честным для гибких родителей.
    pub fn fit_within(mut self, limit: Pixels) -> Self {
        self.fit_limit = Some(limit);
        self
    }

    /// Мерить содержимое по МИНИМАЛЬНОМУ вдоль строки (см. поле `col_min`).
    pub fn column_min(mut self, on: bool) -> Self {
        self.col_min = on;
        self
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
        let claim = self
            .key
            .and_then(|k| VT_MEASURED.with(|c| c.borrow().get(&k).copied()))
            .unwrap_or(self.natural.height);
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
        if let Some(cap) = self.claim_cap
            && self.natural.width >= cap
        {
            style.size.height = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(cap),
            ));
        } else if let Some(limit) = self.fit_limit
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
            let sized = child.layout_as_root(space, window, cx);
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
///
/// Поворот — на четверть по часовой вокруг верхнего правого угла рамки
/// (`VerticalText::paint`): до-поворотная `(px, py)` от угла рамки становится
/// экранной `(x + w - py - thickness, y + px)`. Толщина — колонка строки, в
/// которую коробка встала.
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
}

/// Открыть слой содержащего блока вокруг детей позиционированной коробки.
pub fn cb_open() {
    CB.with(|s| s.borrow_mut().push(Vec::new()));
}

/// Забрать накопленное верхним слоем содержащего блока и закрыть его.
pub fn cb_close() -> Vec<AnyElement> {
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
    probe
        .child(gpui::canvas(
            move |bounds, _, _| {
                let mut now = spot.get();
                now.rotated = VT_FRAME.with(|c| c.get()).is_some();
                now.hole = Some(vt_map(bounds, gpui::px(now.line_thickness)));
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
    // Заданную ось решает СОДЕРЖАЩИЙ БЛОК — «padding edge of the ancestor»
    // (CSS 2.1 §10.1 п. 4.2), и высота в уравнении §10.6.4 («= height of
    // containing block») обязана быть его высотой. Любая потоковая обёртка,
    // вставленная нами ради порядка отрисовки, по умолчанию `relative` и
    // перехватывает эту роль: у неё `h_0()`, поэтому `bottom` решался против
    // НУЛЯ и коробка уезжала вверх ровно на высоту блока, а `height` из
    // `top`+`bottom` выходил отрицательным и схлопывался в ноль. `top` от
    // такой обёртки совпадал случайно — её верх и есть верх содержимого
    // блока; отсюда разрез «`top` зелёный, `bottom` красный» внутри одной
    // семьи: `vrl-038`/`-044`/`-074`/`-080`/`-086` против
    // `vrl-014`/`-020`/`-050`/`-056`/`-062`/`-068`.
    //
    // Blink промежуточной коробки не имеет вовсе: размер берётся у фрагмента
    // предка, обрезанного до отбивочной коробки
    // (`out_of_flow_layout_part.cc:823-838` -> `ContainingBlockInfo::rect`),
    // и оттуда идёт в `InsetModifiedContainingBlock::available_size`
    // («the original containing block size that the insets refer to»,
    // `absolute_utils.h:85`).
    //
    // Прежде рукав был загейтен `now.own_vertical` — под класс «спан с
    // СОБСТВЕННЫМ вертикальным письмом» (`abs-pos-non-replaced-vlr-121`,
    // `vrl-120`; разбор `target/scout-vabs-stretch-2026-09.md`, часть 5).
    // Чьё письмо вертикально, к выбору содержащего блока отношения не имеет:
    // те же подслучаи §10.6.4 при письме на СОДЕРЖАЩЕМ БЛОКЕ
    // (`vrl-014`…`-068`) ломались так же. Голый заместитель нужен всякой
    // коробке с заданной осью.
    //
    // `!vertical` обязателен: при `Spot.vertical` щуп стоит не в строке, а в
    // вертикальном потоке блоков — там обёртка своя (`w_0().h_0()`) и свой
    // разбор, вертикальные классы с ней зелёные (`vlr-087`, `vlr-119`).
    if now.fixed_axes != (false, false) && !now.vertical {
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
            (Some(hole), None) if now.rtl && hole.size.width > px(0.0) => gpui::point(
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
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (cw, ch) = (w * 2.0, h * 2.0);
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}" viewBox="0 0 {cw} {ch}"><defs>{}</defs><rect x="{}" y="{}" width="{w}" height="{h}" fill="{}" filter="url(#{})"/></svg>"##,
            self.def,
            w * 0.5,
            h * 0.5,
            self.fill,
            self.id
        );
        let sf = window.scale_factor();
        let key = (markup.clone(), (cw * sf) as u32, (ch * sf) as u32);
        let image = FILTER_RASTERS.with(|m| m.borrow().get(&key).cloned()).or_else(|| {
            let img = crate::svg::rasterize(&markup, cw * sf, ch * sf)?;
            FILTER_RASTERS.with(|m| m.borrow_mut().insert(key.clone(), img.clone()));
            Some(img)
        });
        let Some(image) = image else { return };
        let area = Bounds {
            origin: gpui::point(bounds.origin.x - px(w * 0.5), bounds.origin.y - px(h * 0.5)),
            size: gpui::size(px(cw), px(ch)),
        };
        let _ = window.paint_image(area, gpui::Corners::default(), image, 0, false);
    }
}
