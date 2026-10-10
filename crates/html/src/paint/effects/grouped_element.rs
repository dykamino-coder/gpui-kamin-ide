//! Элемент `Grouped`.
// owner: A

use crate::paint::effects::{mask_geometry, mask_size, polygon_clip, rectangular_clip};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

/// Сетка таблицы, у которой ширины колонок считаются по содержимому.
///
/// Поддерево, нарисованное в отдельный буфер (`filter: blur(N)`).
///
/// Размытию нужна сложенная картинка поддерева целиком: размывать каждый
/// примитив по отдельности — не то же самое, края внутри группы обязаны
/// смешаться до размытия. Патч gpui рисует детей в свой буфер и кладёт его в
/// кадр уже размытым.
pub struct Grouped {
    pub(crate) child: Option<AnyElement>,
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
    pub polygon: Vec<(
        crate::style::values::value::Len,
        crate::style::values::value::Len,
    )>,
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
    pub mask_size: Option<(
        crate::style::values::value::Len,
        crate::style::values::value::Len,
    )>,
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
    pub mask_pos: Option<(
        crate::style::values::value::Len,
        crate::style::values::value::Len,
    )>,
    /// Смещение от правого/нижнего края (`right 30px bottom 25px`).
    pub mask_pos_far: (bool, bool),
    /// `mask-position` ПО СЛОЯМ (css-masking-1 §7.7): `(x, y, справа, снизу)`;
    /// пусто — берётся скаляр.
    pub mask_pos_list: Vec<(
        crate::style::values::value::Len,
        crate::style::values::value::Len,
        bool,
        bool,
    )>,
    /// Края коробки укладки (`mask-origin`) от border-box внутрь: t/r/b/l.
    pub mask_origin_off: [f32; 4],
    /// Края коробки окраски (`mask-clip`); None — border-box/no-clip.
    pub mask_clip_off: Option<[f32; 4]>,
    /// `clip: rect(t r b l)`: координаты видимой области от углов коробки.
    pub clip_rect: Option<[Option<f32>; 4]>,
    /// Сдвиг коробки клипа трансформом элемента: px и доли своего размера.
    pub clip_shift: (f32, f32, f32, f32),
    /// `clip-path: inset(t r b l)`: срезы краёв; доли — от своих сторон.
    pub clip_inset: Option<[crate::style::values::value::Len; 4]>,
    pub clip_edges: Option<[Option<crate::style::values::value::Len>; 4]>,
    pub clip_xywh: Option<[crate::style::values::value::Len; 4]>,
    /// `round <radius>` of `inset()`/`rect()`/`xywh()` in points: the group
    /// composites through a rounded rectangle equal to the clip rectangle.
    pub clip_round: Option<crate::style::values::value::Len>,
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
    let markup = crate::paint::effects::mask::mask_snapshot(key)?;
    // Внутри <clipPath> правило намотки несёт `clip-rule`; растеризатор
    // рисует контур как обычный и читает только `fill-rule`
    // (clip-path-shape-002: у эталона пропадала дырка evenodd).
    let markup = markup.replace("clip-rule", "fill-rule");
    let body = if force_white {
        format!(r##"<g fill="#ffffff">{markup}</g>"##)
    } else {
        markup
    };
    crate::svg::raster::rasterize(
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
        let mask_box =
            mask_geometry::positioning_box(self.mask.as_deref(), bounds, *_state, window);
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
            let layers: Vec<String> = crate::style::css::split_args(src)
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
                && layers
                    .iter()
                    .all(|l| plain(l) && crate::paint::background::source(l).is_none())
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
            let (cx, cy, rx, ry) = crate::paint::background::shape_params(raw, bw, bh, 1.0)?;
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
                let neg = |v: crate::style::values::value::Len| match v {
                    crate::style::values::value::Len::Px(p) if p < 0.0 => -p,
                    _ => 0.0,
                };
                (
                    sl.max(neg(l)),
                    st.max(neg(t)),
                    sr.max(neg(r)),
                    sb.max(neg(b)),
                )
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
            let rule = if self.polygon_evenodd {
                "evenodd"
            } else {
                "nonzero"
            };
            Some(format!("pathdef:{rule}:{} Z", d.join(" ")))
        } else {
            None
        };
        let polygon = if poly_mask.is_some() {
            Vec::new()
        } else {
            polygon
        };
        let mask = self.mask.as_deref().or(poly_mask.as_deref()).and_then(|src| {
            // `border-shape` (css-borders-4): маска — внешний контур рамки,
            // SVG-растр на РАСШИРЕННУЮ область (обводка выходит за
            // border-box), одной плиткой без мощения; альфа = покрытие.
            if let Some(spec) = src.strip_prefix("bordershape:") {
                let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let (aw, ah) = (bw + sl + sr, bh + st + sb);
                let markup =
                    crate::paint::background::border_shape_mask_svg(spec, bw, bh, sl, st, aw, ah)?;
                let img = crate::svg::raster::rasterize(&markup, aw, ah)?;
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
                crate::style::css::split_args(src)
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
            let len = |l: crate::style::values::value::Len, side: f32, auto: f32| match l {
                crate::style::values::value::Len::Px(v) => v,
                crate::style::values::value::Len::Pct(p) => p * side,
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
                    let one = |l: crate::style::values::value::Len, free: f32, far: bool| {
                        let val = match l {
                            crate::style::values::value::Len::Pct(p) => p * free,
                            l => len(l, free, 0.0),
                        };
                        if far { free - val } else { val }
                    };
                    (one(x, bw - tw, fx), one(y, bh - th, fy))
                };
                let built: Vec<crate::paint::background::MaskLayer> = layers
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
                                crate::svg::raster::rasterize(&markup, bw, bh)?,
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
                            let d = crate::paint::background::shape_to_path(body, fw2, fh2)?;
                            let (dx, dy) = (-el, -et);
                            let markup = format!(
                                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}"><g transform="translate({dx} {dy})"><path fill="#ffffff" fill-rule="{rule}" d="{d}"/></g></svg>"##
                            );
                            (
                                crate::svg::raster::rasterize(&markup, bw, bh)?,
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
                                crate::svg::raster::rasterize(&markup, bw, bh)?,
                                [0.0, 0.0, bw * sf, bh * sf],
                                true,
                            )
                        } else {
                            let source = crate::paint::background::source(l)?;
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
                        Some(crate::paint::background::MaskLayer {
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
                let img = crate::paint::background::compose_mask_layers(&built, cw, ch)?;
                return Some((
                    img,
                    Bounds { origin, size },
                    // Светимость уже учтена при сборке полотна.
                    3,
                ));
            }
            let source = crate::paint::background::source(src)?;
            let (img, tw, th) = match &source {
                crate::paint::background::Source::Raster(img) => {
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
                        crate::paint::background::Source::Shape { raw }
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
                                crate::paint::background::shape_params(raw, rw, rh, 1.0)?;
                            let (cx, cy) = (cx0 - pl, cy0 - pt);
                            let (aw, ah) = (bw + sl + sr, bh + st + sb);
                            let img = crate::paint::background::rasterize_ellipse_px(
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
                        crate::paint::background::Source::Shape { raw } => {
                            // Точечные величины формы записаны в CSS-точках —
                            // растеризатору нужен их масштаб.
                            crate::paint::background::rasterize_shape(
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
            let (ox, oy) = match self.mask_pos {
                Some((x, y)) => {
                    // Доля — от свободного места; `right/bottom` зеркалит
                    // отсчёт (css-backgrounds §3.6, mask-position-1a).
                    let one = |l: crate::style::values::value::Len, free: f32, far: bool| {
                        let v = match l {
                            crate::style::values::value::Len::Pct(p) => p * free,
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
            && let Some(img) = crate::svg::raster::rasterize(&markup, aw, ah)
        {
            let _ = window.paint_image_with_sampling(
                layer_at,
                gpui::Corners::default(),
                img,
                0,
                false,
                gpui::ImageSampling::Linear,
            );
        }
        // css-shapes-1 §basic-shape-rect: `round` rounds the corners of the
        // clip rectangle itself; the composite quad carries those radii.
        // Percentages: Blink BasicShapeInset resolves radii against the
        // reference box; one scalar radius per corner takes the smaller axis.
        let round = match self.clip_round {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Pct(p)) => {
                p * f32::from(_prepaint.0.size.width).min(f32::from(_prepaint.0.size.height))
            }
            _ => 0.0,
        };
        let (area, corners) = match mask_clip {
            Some([x, y, w, h])
                if round > 0.0 && mask.is_none() && polygon.is_empty() && w > 0.0 && h > 0.0 =>
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
                && let Some(img) = crate::svg::raster::rasterize(&markup, aw, ah)
            {
                let _ = window.paint_image_with_sampling(
                    layer_at,
                    gpui::Corners::default(),
                    img,
                    0,
                    false,
                    gpui::ImageSampling::Linear,
                );
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
