//! Элемент `Grouped`.

use crate::document::state::ValueCx;
// owner: A

mod lifecycle;
mod mask_sources;
mod painting;
use mask_sources::{mask_layer_source, rasterize_mask_def, svg_fragment};

use gpui::AnyElement;

/// Сетка таблицы, у которой ширины колонок считаются по содержимому.
///
/// Поддерево, нарисованное в отдельный буфер (`filter: blur(N)`).
///
/// Размытию нужна сложенная картинка поддерева целиком: размывать каждый
/// примитив по отдельности — не то же самое, края внутри группы обязаны
/// смешаться до размытия. Патч gpui рисует детей в свой буфер и кладёт его в
/// кадр уже размытым.
// Each group stores its paint callbacks with their captured geometry.
#[allow(clippy::type_complexity)]
pub struct Grouped {
    pub(crate) child: AnyElement,
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
    /// Immutable resources captured before deferred paint of this group.
    pub(crate) mask_defs: std::sync::Arc<super::mask::MaskDefinitions>,
    /// Separate decoration and overflow contours for shaped corners.
    pub(crate) corner_clip: Option<super::corner_clip::CornerClip>,
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
    /// CSS Images 3 §5.1: raster masks share the element image orientation.
    pub mask_orient_none: bool,
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
    pub(crate) under:
        Option<Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<crate::svg::path_scene::Scene>>>,
    /// Накладка НАД буфером группы, вне его маски: кольцо рамки
    /// `border-shape` у коробки с обрезкой переполнения — содержимое режется
    /// ВНУТРЕННИМ контуром (css-borders-4 §border-shape-overflow-interaction),
    /// а рамка лежит снаружи него и поверх обрезанных детей. Колбэк — как у
    /// `under`.
    pub(crate) over:
        Vec<Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<crate::svg::path_scene::Scene>>>,
    /// Вынос слоёв `under`/`over` за border-box (l t r b) сверх области
    /// группы: тени и контур `corner-shape` лежат снаружи её маски.
    pub deco_ext: [f32; 4],
    /// Заливки по пути (маска `pathdef:` × цвет) под и над буфером:
    /// резкие тени и контур `corner-shape`; колбэк — как у `under`.
    pub(crate) under_fills: Option<FillLayer>,
    pub(crate) over_fills: Option<FillLayer>,
    /// Множитель интринзика плитки маски: у SVG-ребёнка маска живёт в ЕГО
    /// пользовательских единицах (css-masking-1 §7.4 `auto` — размер
    /// картинки в системе координат элемента), и при `viewBox` 50×50
    /// рисунок-маска кроет 100×100 CSS-точек (mask-origin-3, mask-clip-2).
    /// Задаёт `svg::masked_layers` через `Computed::mask_user_scale`.
    pub mask_scale: f32,
    /// Кольцо `corner-shape` НАД содержимым замещаемого элемента (запись,
    /// цвет, отступы content-box от border-box t/r/b/l): содержимое
    /// обрезается внутренним контуром (css-borders-4 §corner-shape-rendering),
    /// поэтому внутри content-box кольцо ложится поверх картинки.
    pub ring_over: Option<(String, crate::style::values::value::Color, [f32; 4])>,
    /// Values of the document the group's mask and clip lengths come from.
    pub(crate) values: crate::document::state::ValueDoc,
}

/// Слой заливок: записи `pathdef:` в точках слоя и их цвета.
pub(crate) type FillLayer = Box<
    dyn Fn(
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
    ) -> Vec<(
        crate::svg::path_scene::Scene,
        crate::style::values::value::Color,
    )>,
>;

impl Grouped {
    pub fn new(child: AnyElement, vcx: ValueCx<'_>) -> Self {
        Grouped {
            values: vcx.keep(),
            under: None,
            over: Vec::new(),
            deco_ext: [0.0; 4],
            under_fills: None,
            over_fills: None,
            mask_scale: 1.0,
            ring_over: None,
            child,
            blur: 0.0,
            opacity: 1.0,
            blend: 0,
            spill: false,
            polygon: Vec::new(),
            polygon_evenodd: false,
            poly_expand: [0.0; 4],
            mask: None,
            mask_defs: vcx.doc.paint.definitions(),
            corner_clip: None,
            mask_size: None,
            mask_fit: 0,
            mask_no_repeat: (false, false),
            mask_repeat_list: Vec::new(),
            mask_repeat_modes: Vec::new(),
            clip_round: None,
            mask_luminance: false,
            mask_alpha_mode: false,
            mask_orient_none: false,
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
