//! CSS-операнды группы mask; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetMask {
    /// Опорная коробка clip-path: 0 border, 1 margin, 2 padding, 3 content.
    pub clip_ref: Option<u8>,
    /// `clip-path` задан ОДНИМ словом коробки (`margin-box`, `padding-box`,
    /// …): обрезка краями этой коробки (css-masking-1 §5.1 «If specified by
    /// itself, uses the edges of the specified box … as clipping path»).
    pub clip_bare_box: bool,
    /// Маска-изображение (`mask-image: url(...)|<gradient>`): источник
    /// строкой до растра при сборке группы.
    pub mask_image: Option<Vec<ImageSource>>,
    pub clip_margin: Option<f32>,
    /// Коробка отсчёта края обрезки: 0 content, 1 padding, 2 border;
    /// None — умолчание (padding-box).
    pub clip_margin_box: Option<u8>,
    /// `clip-path: polygon(…)`: вершины в долях или точках коробки.
    pub clip_polygon: Option<Vec<(Len, Len)>>,
    /// Правило намотки полигона `clip-path: polygon(evenodd, …)`.
    pub clip_polygon_evenodd: bool,
    /// `clip-path`/`mask`: обрезка по кругу или скруглённому прямоугольнику.
    /// Хранится долей радиуса от меньшей стороны либо радиусом в точках.
    pub clip_round: Option<f32>,
    /// The uniform `round` radius of `inset()`/`rect()`/`xywh()` as written
    /// (points or a percentage of the reference box).
    pub clip_round_len: Option<Len>,
    /// `clip-path: circle(...)|ellipse(...)` с параметрами: сырые аргументы
    /// формы (`shape:circle(...)`). Радиусы и центр зависят от размера
    /// коробки — он известен только отрисовке, поэтому форма растрируется
    /// маской буфера группы (см. `background::source`).
    pub clip_shape: Option<String>,
    /// `mask-size`: размер плитки маски; None — auto (интринзик картинки).
    pub mask_size: Option<(Len, Len)>,
    /// `mask-repeat`: пооосный запрет мощения (no-x, no-y) — ПЕРВОГО слоя.
    pub mask_no_repeat: Option<(bool, bool)>,
    /// То же ПО СЛОЯМ (css-masking-1 §7.6, `<repeat-style>#`): запись
    /// `no-repeat, repeat` задаёт свою укладку каждому слою. Список короче
    /// набора слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_repeat_list: Option<Vec<(bool, bool)>>,
    /// Per-layer `space`/`round` axes (css-masking-1 §7.6 →
    /// css-backgrounds-3 §3.4): 2 space, 3 round, anything else as
    /// `mask_repeat_list` says.
    pub mask_repeat_modes: Option<Vec<(u8, u8)>>,
    /// `mask-size: contain|cover` (1|2): вписывание по интринзику.
    pub mask_fit: Option<u8>,
    /// `mask-mode: luminance` — маскирует светимость, а не альфа.
    pub mask_luminance: Option<bool>,
    /// `mask-mode: alpha` — альфа и для ссылки на `<mask>` (css-masking-1
    /// §7.2: `match-source` берёт `mask-type` определения).
    pub mask_alpha_mode: Option<bool>,
    /// `mask-type: alpha` у элемента `<mask>` (css-masking-1 §7.16).
    pub mask_type_alpha: Option<bool>,
    /// `mask-origin`: коробка укладки плитки (0 border, 2 padding, 3 content).
    pub mask_origin: Option<u8>,
    /// Готовые коробки маски в CSS-точках (укладка t/r/b/l от коробки
    /// слоя внутрь; окраска — то же либо None = без обрезки) — для SVG-детей,
    /// у которых fill-/stroke-/view-box считает `svg::masked_layers`, а не
    /// рамка и отбивка (`render::grouped`).
    pub mask_box_override: Option<([f32; 4], Option<[f32; 4]>)>,
    /// Пользовательская единица SVG-ребёнка в CSS-точках (масштаб `viewBox`
    /// или `zoom`); 0 — не задано (= 1). Интринзик плитки маски у такого
    /// ребёнка считается в его единицах (`paint::effects::grouped_element::Grouped::mask_scale`).
    pub mask_user_scale: f32,
    /// `mask-clip`: коробка окраски маски; вне её элемент скрыт. 255 — no-clip.
    pub mask_clip: Option<u8>,
    /// `mask-composite` по слоям: 0 add, 1 subtract, 2 intersect, 3 exclude.
    pub mask_composite: Option<Vec<u8>>,
    /// `clip: rect(t r b l)` (CSS 2.1 §11.1.2, только absolute): координаты
    /// видимой области от углов border-box; None в позиции — auto (край).
    pub clip_rect: Option<[Option<f32>; 4]>,
    /// Тот же прямоугольник, но КАК НАПИСАН: единицы шрифта на разборе ещё не
    /// меряются, а `clip: rect(1em, …)` без них читался как `auto` и не
    /// обрезал вовсе (`visufx/clip-079/080/091/092`). Сводится к точкам в
    /// `resolve_em`, где кегль и метрики семейства уже известны.
    pub clip_len: Option<[Option<Len>; 4]>,
    /// `clip-path: inset(t r b l ...)`: срезы краёв видимой области.
    pub clip_inset: Option<[Len; 4]>,
    /// `clip-path: rect(t r b l)` — координаты КРАЁВ от верхнего-левого
    /// угла; None = auto (край коробки).
    pub clip_edges: Option<[Option<Len>; 4]>,
    /// `clip-path: xywh(x y w h)` — прямоугольник от угла.
    pub clip_xywh: Option<[Len; 4]>,
    /// `mask-position`: смещение плитки; доля — от свободного места
    /// (коробка минус плитка), как у `background-position`.
    pub mask_pos: Option<(Len, Len)>,
    /// Смещение отсчитано от ПРАВОГО/НИЖНЕГО края (`right 30px bottom 25px`).
    pub mask_pos_far: (bool, bool),
    /// `mask-position` ПО СЛОЯМ (css-masking-1 §7.7, `<position>#`):
    /// `(x, y, от правого края, от нижнего края)`. Список короче набора
    /// слоёв повторяется (css-backgrounds-3 §2.2).
    pub mask_pos_list: Option<Vec<(Len, Len, bool, bool)>>,
    /// `mask-border-source/-slice/-width/-outset/-repeat` (css-masking-1 §8);
    /// `None` — все части начальные. Not inherited.
    pub mask_border: Option<BorderImage>,
    /// `mask-border-mode: luminance` (§8.7); initial `alpha`. Not inherited.
    pub mask_border_luminance: bool,
}
