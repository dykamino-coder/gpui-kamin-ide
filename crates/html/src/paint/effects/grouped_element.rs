//! Элемент `Grouped`.
// owner: A

mod lifecycle;

use gpui::{AnyElement, IntoElement};

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

impl IntoElement for Grouped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
