//! CSS-операнды группы cascade; значение за общим COW-указателем.
use super::super::*;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetCascade {
    /// Winning explicit-inherit longhands, consumed before effective zoom.
    pub explicit_inherit: Vec<crate::style::cascade::inherited_value::Property>,
    /// Место в каскаде применяемого сейчас объявления: им логические и
    /// физические стороны и размеры решают спор (UA `padding-inline-start:
    /// 40px` против авторского `padding-top: 0` в вертикальном письме —
    /// `line-box-direction-vrl-019`).
    pub decl_order: DeclOrder,
    pub side_order: SideOrder,
    /// Ненаследуемые длинные свойства реестра с каскадным значением
    /// `inherit` (css-cascade-5 §7.3.1): вычисленное значение родителя
    /// копирует стадия вычисленных значений (`style::cascade::compute`) —
    /// одним реестром для сторон, размеров, рамки, контура, фона,
    /// выравнивания и прочих свойств реестра.
    pub inherit_longhands: crate::style::css::LonghandSet,
    /// `inherit` у ненаследуемых свойств вне реестра длинных свойств:
    /// повтор мостовой (`background-repeat`), слой (`z-index`), обводка,
    /// вид (`display`), плитка и её место (`background-image`,
    /// `background-position`), обрезка (`clip`), сокращение шрифта,
    /// преобразование регистра.
    pub inherit_bits: u32,
    pub font_weight_step: i8,
    /// `font-size: larger` (+1) / `smaller` (−1): шаг по таблице ключевых
    /// кеглей от кегля родителя (§15.7). Разрешает `style::cascade::inherit`.
    pub font_size_step: i8,
    /// `unicode-bidi: bidi-override` (и тег `<bdo>`) — порядок знаков задан
    /// силой, разбор двунаправленности внутри куска не работает.
    pub bidi_override: Option<bool>,
    /// Explicit inheritance of the otherwise non-inherited unicode-bidi property.
    pub bidi_inherit: bool,
    /// `unicode-bidi: isolate` — кусок не влияет на порядок соседей.
    pub bidi_isolate: Option<bool>,
    /// `unicode-bidi: embed` — свой уровень встраивания (RLE/LRE … PDF). Без
    /// него (`normal`) `direction` строчного элемента порядка знаков НЕ
    /// меняет (css-writing-modes-3 §2.2: «normal — the element does not open
    /// an additional level of embedding»). Атрибут `dir` ставит его сам
    /// (`dom`: `apply_direction`).
    pub bidi_embed: Option<bool>,
    /// `unicode-bidi: plaintext` — сторона письма решается для каждого абзаца
    /// между жёсткими разрывами. HTML ставит это правило на `dir="auto"`.
    pub bidi_plaintext: Option<bool>,
    /// `zoom` (css-viewport-1 §zoom-property): СВОЙ множитель элемента, как
    /// написан; `None` — не задан. `0`/`0%` по спеке читаются единицей.
    /// Читает его стадия вычисленных значений (`style::cascade::compute::zoom`).
    pub zoom: Option<f32>,
    /// Длины стиля ещё не домножены на действующий зум: стиль — выход
    /// каскада. Вычисленный стиль и стиль коробки раскладки несут
    /// используемые точки, и стадия зума их не трогает.
    pub zoom_pending: bool,
    /// Element used color scheme, shared by CSS system colours and native controls.
    pub dark_color_scheme: bool,
    /// Preferred scheme exposed by this element to an embedded document.
    pub embedding_dark_scheme: bool,
    /// Specified initial color, resolved specially for quirks HTML tables.
    pub color_initial: bool,
    /// Parse-time acceptance, armed only for a capability query.
    pub declaration_parse: super::ParseResult,
    /// The declaration being applied is a CSS-wide keyword (css-cascade-5 §7.3):
    /// slots whose grammar does not resolve the keyword take their empty
    /// (initial) representation. Set only around one property dispatch.
    pub css_wide_value: bool,
    /// `color` with a function of `currentcolor` (`color-mix()`, relative
    /// color): computed against the PARENT's color when merged (css-color-4
    /// §6.4, css-color-5 §2), then inherited as the absolute `color`. Own style only.
    pub color_function: Option<crate::style::values::value::StyleColor>,
    /// Группы тегов `font_features`, заданные своими `font-variant-*`
    /// (разряд — группа `props::font::variants::GROUP_TAGS`): незаданная
    /// группа наследуется от родителя отдельно от прочих (css-fonts-4 §6).
    pub font_variant_set: u8,
}
