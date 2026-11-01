//! CSS-операнды группы generated; значение за общим COW-указателем.
use super::super::*;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetGenerated {
    pub content_visibility: super::ContentVisibility,
    /// Локальное подавление маркера: `::marker { content: none }` или
    /// продолжение разрезанной коробки. Не наследуется.
    pub no_marker: Option<bool>,
    /// Локальное вычисленное содержимое `::marker`; не наследуется.
    pub marker_text: Option<String>,
    /// Слой `::marker` (css-lists-3 §marker-properties): ТОЛЬКО объявления
    /// самих правил `::marker` поверх таблицы агента, без копии стиля
    /// хозяина — при отрисовке накладывается на стиль пункта через
    /// `style::cascade::inherit`. Копия стиля пункта сюда не годится: она утянула бы
    /// в маркер рамку, поля и размеры самого `<li>`. Blink делает то же —
    /// текст маркера набирается стилем САМОГО `::marker`
    /// (`CreateAnonymousStyleWithDisplay(marker.StyleRef(), …)`,
    /// `list_marker.cc:267-336`). Не наследуется.
    pub marker_layer: Option<Box<Computed>>,
    /// `content: none` против `content: normal`. У `::before`/`::after`
    /// разницы нет — коробки нет в обоих случаях, — а у `::marker` `none`
    /// гасит маркер, `normal` возвращает к `list-style-*` (css-lists-3
    /// §content-property). Оба сбрасывают `content` в None, поэтому нужна
    /// отдельная метка.
    pub content_none: Option<bool>,
    /// `scroll-marker-group` (css-overflow-5): `Some(true)` — группа маркеров
    /// ПЕРЕД скроллером (`before`), `Some(false)` — после (`after`), `None` —
    /// `none`. Не наследуется.
    pub scroll_marker_group: Option<bool>,
    /// `scroll-target-group: auto` (css-overflow-5): links inside are scroll
    /// markers. Not inherited.
    pub scroll_target_group: Option<bool>,
    /// `content` псевдоэлемента — СПИСОК составляющих (css-content-3 §2):
    /// строки, `counter()`, `counters()`, `attr()` в любом порядке.
    pub content: Option<Vec<ContentItem>>,
    /// Explicit inheritance is resolved along the DOM before pseudo generation.
    pub content_inherit: bool,
    /// `quotes` (css-content-3 §4.1): пары кавычек по уровням вложенности.
    /// `None` inherits; `Some(None)` suppresses marks but keeps nesting depth.
    /// `Some(Some(empty))` is explicit auto; nonempty pairs are a custom system.
    /// Tree traversal resolves inheritance before generating pseudo content.
    pub quotes: Option<Option<Vec<(String, String)>>>,
    /// `counter-reset` — обнулить счётчик с этого узла.
    pub counter_reset: Option<String>,
    /// `counter-increment` — увеличить счётчик на этом узле.
    pub counter_increment: Option<String>,
    /// `counter-set` — присвоить счётчику значение (css-lists-3 §5).
    pub counter_set: Option<String>,
}
