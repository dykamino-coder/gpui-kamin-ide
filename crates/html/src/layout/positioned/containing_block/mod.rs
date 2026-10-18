//! Содержащий блок абсолютных коробок (поздняя расстановка).
// owner: A

use gpui::{Bounds, Pixels};
mod late;
pub use late::LatePlace;
pub use late::spot_place;
pub use late::spot_probe;
mod hang;
pub use hang::InlineStartHang;
mod source_paint;
pub(crate) use source_paint::split as split_source_paint;
mod layers;
pub use layers::Positioning;

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
    /// Block-flow layer already supplies the actual CB layout geometry.
    pub preserve_cb_geometry: bool,
    /// Geometry-only proxy; prepaint and paint stay at the source position.
    pub layout_only: bool,
    /// Static coordinates must leave any enclosing fragmentation contexts.
    pub viewport_static: bool,
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
    /// `self_align` is measured in an INLINE parent's static-position
    /// rectangle (zero-thick along the line, the line box across it), not in
    /// the block strut's (`static_position::inline_static_self_align`).
    pub line_rect: bool,
    /// The line box `(top, height, start x)` of a probe that a text paragraph
    /// placed as an overlay in its line (`with_line_box`).
    pub line_box: Option<(Pixels, Pixels, Pixels)>,
}

/// Hand-off of a text line's box `(top, height, start x)` from the paragraph
/// that places a static-position probe as an overlay in that line to the
/// probe: the paragraph fills it right before the probe's prepaint, the probe
/// takes it (`OverlayAt::line_probe`, `LayoutCx::line_box`).
pub type LineBoxSlot = std::rc::Rc<std::cell::Cell<Option<(Pixels, Pixels, Pixels)>>>;

pub type SpotCell = std::rc::Rc<std::cell::Cell<Spot>>;

/// The CB-ancestor index of the nearest containing block of an absolute box
/// whose parent's computed style is `parent` (see `Computed::cb_depth`).
pub(crate) fn nearest_cb_index(parent: &crate::style::computed::Computed) -> u16 {
    parent.cb_depth + u16::from(crate::text::inline::establishes_cb(parent))
}
