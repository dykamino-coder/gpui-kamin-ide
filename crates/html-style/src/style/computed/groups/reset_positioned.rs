//! CSS-операнды группы positioned; значение за общим COW-указателем.
use super::super::*;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetPositioned {
    pub position: Option<Position>,
    pub inset: Sides,
    /// `anchor-name` (css-anchor-position-1 §anchor-name): имена якоря,
    /// `none` — отсутствие. Коробка с именем получает пробу
    /// (`anchor::probe_for`), пишущую её рамку в реестр кадра.
    pub anchor_name: Option<Vec<String>>,
    /// `position-anchor`; начальное `normal`.
    pub position_anchor: Option<PositionAnchor>,
    /// Неявный якорь псевдоэлемента — `node_id` порождающего элемента
    /// (§implicit: «The implicit anchor element of a pseudo-element is its
    /// originating element»). Ставит `dom::walk` после обхода детей.
    pub implicit_anchor: Option<u64>,
    /// `position-area` (§position-area): два слова области, разбор и смысл —
    /// `anchor::parse_area`. `none`/не задано — `None`.
    pub position_area: Option<crate::style::values::position_area::PositionArea>,
    /// `position-try-fallbacks` (§position-try-fallbacks): варианты позиции —
    /// имя `@position-try`-правила и/или тактика, либо `<position-area>`.
    /// Каждый кандидат отдельно строится хостом до prepaint победителя.
    pub position_try_fallbacks: Vec<crate::style::values::position_area::TryFallback>,
    /// `position-try-order` (§position-try-order-property): 0 normal,
    /// 1 most-width, 2 most-height, 3 most-block-size, 4 most-inline-size.
    pub position_try_order: u8,
    /// `position-visibility` (§position-visibility), биты `anchor::VIS_*`:
    /// 1 anchor-valid, 2 anchor-visible, 4 no-overflow; 0 — `always`
    /// (начальное значение Blink; спека просит `anchor-visible`, но без
    /// явного свойства гасить коробки по обрезке якоря слишком дорого).
    pub position_visibility: u8,
    pub icb_scroll_insets: Option<[f32; 4]>,
    /// Root unconditional gutters reduce viewport units, independently of body overflow.
    pub viewport_unit_insets: Option<[f32; 2]>,
    /// Коробка АБСОЛЮТНА, но позиционирование с неё снято ради статической
    /// позиции (`render::blocks::positioned`). Само `position` там обнуляется, а знать о нём
    /// нужно: размер по свободной строчной оси у абсолюта считается по
    /// содержимому, и без этой пометки вертикальный абзац снова растягивался
    /// бы на весь предел ортогонального потока.
    pub abs_static: bool,
    /// `z-index`: порядок наложения. У GPUI слоёв нет — вместо них отложенная
    /// отрисовка с приоритетом.
    pub z_index: Option<i32>,
    /// Накопленный относительный сдвиг строчных предков куска в точках
    /// (CSS 2.1 §9.4.3): двигает ТОЛЬКО отрисовку, места в потоке не меняет
    /// и строку не растит.
    pub rel_shift: Option<(f32, f32)>,
    /// `rel_shift` на элементе, чьи подложка и рамка (`InlineDecoration::band_color`,
    /// `InlineDecoration::border_stroke`) у куска: коробка предка едет только его сдвигом.
    pub box_shift: Option<(f32, f32)>,
    /// Абсолют/фиксированный, чей `display` до блокификации (§9.7) был
    /// строчного уровня (`inline-block`, `inline-flex`, …): статическая
    /// позиция считается для гипотетической коробки «если бы position был
    /// static» (CSS 2.1 §10.3.7, §10.6.4), то есть В СТРОКЕ. Ставит `dom`.
    pub abs_inline_level: bool,
}
