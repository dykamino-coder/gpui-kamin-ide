//! CSS-операнды группы box_layout; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetBoxLayout {
    /// CSS Scroll Snap: container policy and descendant snap-area geometry.
    pub snap: crate::style::computed::snap::SnapStyle,
    pub display: Option<Display>,
    /// `flex-basis: content` — основа берётся ПО СОДЕРЖИМОМУ, и заданный
    /// главный размер при ней игнорируется. Ключевого значения у раскладки
    /// под нами нет, поэтому признак хранится отдельно, а основа ставится в
    /// `auto`: этого достаточно, если снять главный размер.
    /// Свой пиксель ЗАМЕЩАЕМОГО элемента: размер из атрибутов `width`/`height`
    /// (у холста — умолчание 300×150). Хранится отдельно от `width`, потому
    /// что `flex-basis: content` снимает ЗАДАННЫЙ главный размер, но свой
    /// пиксель оставляет.
    pub attr_width: Option<Len>,
    pub attr_height: Option<Len>,
    /// Размер по оси пришёл из АТРИБУТА (`<canvas width>`), а не из CSS:
    /// у холста атрибуты — природный размер (HTML §4.12.5), не `width`
    /// (§15.3.10 их к нему не относит), и в сетке ось может растянуться.
    pub attr_sized: (bool, bool),
    /// `container-type: size | inline-size` — элемент стал контейнером
    /// запросов размера и подсеткой быть не может (css-grid-2
    /// §subgrid-listing). Отдельным полем, а НЕ через `contain_size`: голое
    /// `contain: size` подсетку не отменяет — это отдельно проверяют случаи
    /// 8 и 9 `independent-formatting-context.html`.
    pub container_type: ContainerType,
    pub width: Option<Len>,
    pub height: Option<Len>,
    pub min_width: Option<Len>,
    pub min_height: Option<Len>,
    pub max_width: Option<Len>,
    pub max_height: Option<Len>,
    pub padding: Sides,
    pub margin: Sides,
    /// The box's own margins as cascaded, kept before block layout rewrites
    /// `margin` for collapsing (CSS 2.1 §8.3.1). The `margin-box` reference
    /// box of `clip-path` is the border box outset by THESE margins
    /// (css-masking-1 §1.3.1.1, Blink `LayoutBox::MarginBoxRect`): a collapsed
    /// top margin shrank it (`clip-path-polygon-008`). Set only for `margin-box`.
    pub own_margin: Option<Sides>,
    /// A matching scrollbar pseudo style excludes root viewport-unit reduction.
    pub custom_scrollbar: bool,
    /// CSS scrollbar colors, width and forced overflow axes (HTML only).
    pub scroll: super::scroll::ScrollStyle,
    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    /// `resize` — по каким осям элемент тянется мышью.
    pub resize: Option<(bool, bool)>,
    /// `float`: -1 — влево, 1 — вправо, 0 — не обтекается.
    pub float: Option<i8>,
    /// `clear` — сторона, с которой обтекание обрывается перед этим блоком:
    /// -1 слева, 1 справа, 0 с обеих. Стороны различаются, потому что
    /// `clear: left` мимо правого флоата проходит насквозь (CSS 2.1 §9.5.2).
    pub clear: Option<i8>,
    /// Ширина пришла из ЛОГИЧЕСКОГО `inline-size` при вертикальном письме:
    /// физически это высота (оси не переставляются, см. `resolve_logical`) —
    /// потребители обязаны отличать её от настоящего `width`
    /// (table-cell-align-005 против table-progression-htb-001).
    pub width_from_inline: bool,
    /// `calc-size(<basis>, <expr>)` у `width`, `height`, `min-width`,
    /// `min-height` (css-values-5 §calc-size): `(mul, add, max, min)` над
    /// размером основы-ключевого слова; само свойство при этом `auto`, а
    /// выражение применяет раскладка (`taffy::Style::calc_size`).
    pub calc_size: [Option<(f32, f32, f32, f32)>; 4],
    /// The size slot came from `stretch` (stored as `Len::Pct(1.0)`), in the
    /// order width, height, min-width, max-width, min-height, max-height.
    /// css-sizing-4 §4.1 fills the containing block with the margin box, so
    /// it is not a percentage for the content-box contract.
    pub stretch_size: [bool; 6],
    /// Retained calc-size sizing basis: preferred, minimum, maximum, X/Y.
    pub calc_size_basis: [Option<CalcSizeBasis>; 6],
    /// calc-size calculations for max-width and max-height.
    pub calc_size_max: [Option<(f32, f32, f32, f32)>; 2],
}
