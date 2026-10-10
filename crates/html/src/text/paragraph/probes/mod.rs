//! Пробы и записи раскладки: места атомов, экстенты руби, оформление, базовые линии, LayoutTap.

mod baseline_probe;
mod ruby_extents;
pub(super) use crate::text::paragraph::probes::baseline_probe::BaselineProbe;
pub(super) use crate::text::paragraph::probes::baseline_probe::LayoutTap;
pub use crate::text::paragraph::probes::ruby_extents::RubyExtents;
pub use crate::text::paragraph::probes::ruby_extents::RubyOverhangInfo;
pub use crate::text::paragraph::probes::ruby_extents::collect_ruby_extents;
pub use crate::text::paragraph::probes::ruby_extents::ruby_base_with_overhang;
pub use crate::text::paragraph::probes::ruby_extents::ruby_extent;
pub(super) use crate::text::paragraph::probes::ruby_extents::take_ruby_base_sink;

use gpui::{AnyElement, LayoutId};

/// Как атом встаёт в строке по вертикали (`vertical-align`, CSS 2.1 §10.8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtomAlign {
    /// По базовой линии родителя, поднятой на `v` точек (`baseline` — ноль;
    /// длина, процент, `sub`/`super` — свой подъём).
    Shift(f32),
    /// Середина коробки — на высоте базовой линии плюс половина x-высоты.
    Middle,
    /// Верх коробки — по верху текстовой области родителя.
    TextTop,
    /// Низ коробки — по низу текстовой области родителя.
    TextBottom,
    /// Верх коробки — по верху строчной коробки.
    Top,
    /// Низ коробки — по низу строчной коробки.
    Bottom,
}

/// Атом в строке: элемент-обёртка со щупом базовой линии.
pub(super) struct AtomSlot {
    pub(crate) at: usize,
    pub(crate) el: AnyElement,
    pub(crate) align: AtomAlign,
    pub(crate) probe: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узел самой обёртки (см. `LayoutTap`).
    pub(crate) root: std::rc::Rc<std::cell::Cell<Option<LayoutId>>>,
    /// Узлы уровней аннотаций руби (`ruby_extent`): `true` — под базой.
    pub(crate) extents: RubyExtents,
    /// Атом за последней строкой (оборван `line-clamp`): не ставится и не
    /// рисуется.
    pub(crate) hidden: bool,
}

/// Получает ли знак `c` метку акцента (css-text-decor-3 §5.3): нет у
/// разделителей (Z*), управляющих и неназначенных (Cc, Cf, Cn) и у
/// пунктуации (P*), кроме знаков, что по NFKD сводятся к `#`, `%`, `‰`, `‱`,
/// `٪`, `؉`, `؊`, `&`, `⁊`, `@`, `§`, `¶`, `⁋`, `⁓`, `〽` (здесь — сами они и
/// их полноширинные и малые формы).
pub(super) fn emphasized(c: char) -> bool {
    use unicode_properties::{GeneralCategory as G, GeneralCategoryGroup, UnicodeGeneralCategory};
    if c.is_whitespace() {
        return false;
    }
    match c.general_category_group() {
        GeneralCategoryGroup::Separator => false,
        GeneralCategoryGroup::Other => {
            !matches!(c.general_category(), G::Control | G::Format | G::Unassigned)
        }
        GeneralCategoryGroup::Punctuation => matches!(
            c,
            '#' | '%'
                | '\u{2030}'
                | '\u{2031}'
                | '\u{066A}'
                | '\u{0609}'
                | '\u{060A}'
                | '&'
                | '\u{204A}'
                | '@'
                | '\u{00A7}'
                | '\u{00B6}'
                | '\u{204B}'
                | '\u{2053}'
                | '\u{303D}'
                | '\u{FF03}'
                | '\u{FF05}'
                | '\u{FF06}'
                | '\u{FF20}'
                | '\u{FE5F}'
                | '\u{FE6A}'
                | '\u{FE60}'
                | '\u{FE6B}'
        ),
        _ => true,
    }
}

/// Кусок текста с украшениями (css-text-decor-3 §2.1).
#[derive(Clone, Debug)]
pub struct DecorSpan {
    /// Отрезок байт текста абзаца.
    pub range: std::ops::Range<usize>,
    /// Украшения куска от внешнего к внутреннему.
    pub items: Vec<DecorItem>,
    /// `text-decoration-skip-ink` не `none` (css-text-decor-4 §4.3).
    pub skip_ink: bool,
    /// `text-decoration-skip-spaces`: 1 `start`, 2 `end`, 4 `all`.
    pub skip_spaces: u8,
}

/// Одно украшение куска.
#[derive(Clone, Debug)]
pub struct DecorItem {
    pub decor: crate::style::computed::Decor,
    /// Шрифт украшающей коробки (метрики линий).
    pub font: gpui::Font,
    /// Украшенный прогон (Blink «decorated run»): смежные куски с тем же
    /// украшением — от них считаются `text-decoration-inset` при `slice`.
    pub group: std::ops::Range<usize>,
}

/// Кусок со знаком акцента (`text-emphasis`, css-text-decor-3 §5).
#[derive(Clone, Debug)]
pub struct EmphSpan {
    /// Отрезок байт текста абзаца.
    pub range: std::ops::Range<usize>,
    /// Знак под текстом (`text-emphasis-position: under`).
    pub under: bool,
    /// Кегль знака — половина кегля базы (§5.3: как аннотация руби).
    pub size: f32,
    /// `line-height` куска в точках: аннотация встаёт на край его строчной
    /// коробки, а не на край кегля (как `<rt>` над базой руби).
    pub line_height: f32,
    /// Сам знак (первая буква строки или знак формы).
    pub mark: String,
    /// `text-emphasis-color`; пусто — цвет текста.
    pub color: Option<gpui::Hsla>,
}

/// Замер атома в точках: высота коробки полей (по §10.8 выравнивается
/// именно она) и базовая линия от её верха. Ширина уходит продвижением
/// распорки (`letter_spans`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtomBox {
    pub(crate) at: usize,
    pub(crate) h: f32,
    pub(crate) base: f32,
    pub(crate) align: AtomAlign,
    /// Аннотации руби над и под коробкой (`ruby_extent`).
    pub(crate) over: f32,
    pub(crate) under: f32,
    /// Ruby annotation overhang onto the preceding content (css-ruby-1 §4.4):
    /// the atom is placed this far left of its spacer.
    pub(crate) shift: f32,
}
