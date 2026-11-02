//! CSS-операнды группы flex; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetFlex {
    pub flex_dir: Option<FlexDir>,
    pub flex_wrap: Option<bool>,
    /// `wrap-reverse` — строки укладываются с противоположного края.
    pub flex_wrap_reverse: Option<bool>,
    /// `flex-wrap: balance` (css-flexbox-2 §5.2) — строки режет
    /// балансировщик; ортогонально `wrap`/`wrap-reverse`.
    pub flex_balance: Option<bool>,
    /// `flex-line-count` (css-flexbox-2 §5.3) — минимум строк у balance;
    /// умолчание 1.
    pub flex_line_count: Option<u16>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub basis_content: Option<bool>,
    pub align_items: Option<Align>,
    /// `align-self` — про сам элемент; отдельное поле, иначе он выравнивал
    /// бы своих детей вместо себя.
    pub align_self: Option<Align>,
    /// Авторское `align-self: normal` (css-align-3 §6.2): у элемента гибкого
    /// контейнера оно ведёт себя как `stretch`, а не «взять у родителя».
    pub align_self_normal: bool,
    /// Элемент гибкого РЯДА, поперечный размер которого тянет строка
    /// (`stretch` при `height: auto`). Взводит сборщик детей гибкого
    /// контейнера (`render::blocks::flow_children`, ветка `flex_context`), где виден родитель;
    /// читает обособление размера в `apply::apply_box`.
    pub cross_stretched: bool,
    pub flex_basis: Option<Len>,
    pub justify_content: Option<Justify>,
    pub gap: Option<(Option<Len>, Option<Len>)>,
    /// Коробка с `aspect-ratio` — элемент ГИБКОГО контейнера (ставит
    /// `render::blocks::ordered_children` при раскладке детей ряда/колонки). Её автоминимум по
    /// соотношению считает раскладка (css-flexbox-1 §4.5: подсказка по
    /// содержимому), а не явный минимум `apply::ratio_as_auto_min`
    /// (`flex-aspect-ratio-002/004`).
    pub flex_item_ratio: bool,
    /// `order`: визуальный порядок в гибкой строке. Раскладка под нами его не
    /// знает, поэтому детей переставляет сам сборщик дерева.
    pub order: Option<i32>,
    pub align_content: Option<Justify>,
    /// Content baseline preference; false is first, true is last (CSS Align 3 §9).
    pub align_content_baseline: Option<bool>,
    /// Задано ли `align-content` значением, ОТЛИЧНЫМ от `normal`
    /// (css-align-3 §align-block). Отдельно от `align_content`, потому что
    /// `parse_justify` роняет в `None` два разных случая: `normal`
    /// (выравнивания нет — и контекста тоже) и `baseline`/`first`/`last`
    /// (выравнивание есть, раскладка его пока не знает, но КОНТЕКСТ по спеке
    /// заводится). На блочном контейнере флаг делает коробку корнем блочного
    /// контекста форматирования; у флекса и сетки он безразличен — они и так
    /// заводят свой контекст первой же веткой `own_context`.
    pub align_content_block: bool,
    /// `justify-items`/`justify-self` — поперечная ось В СЕТКЕ.
    pub justify_items: Option<Align>,
    /// Модификатор `safe` у выравниваний (css-align §5.3): при переполнении
    /// области выравнивание падает в `start`, чтобы содержимое не обрезалось.
    /// Без него позиция сохраняется и элемент вылезает (unsafe/дефолт).
    pub justify_self_safe: bool,
    pub justify_items_safe: bool,
    pub align_self_safe: bool,
    /// `align-self: self-start`/`self-end` — начало и конец берутся по письму
    /// САМОГО элемента, а не контейнера (css-align-3 §6.2). Значение при этом
    /// остаётся физическим, а «мерить по себе» помнится здесь: зеркалит его
    /// `style::cascade::inherit`, где известны письмо элемента И письмо родителя.
    pub align_self_own_axis: bool,
    /// Ключевое слово `align-self` — ГИБКОЕ (`flex-start`/`flex-end`): его
    /// концы следуют `wrap-reverse` строки, а `start`/`end`/`self-*` — нет
    /// (css-align-3 §6.1). `Align` различия не несёт, его зеркалит
    /// `style::cascade::inherit` (`self-align-start-end-flex-001`).
    pub align_self_flex_kw: bool,
    /// `justify-self: self-start`/`self-end` — по письму САМОГО элемента, как
    /// `align_self_own_axis` (`align-self-static-position-006`).
    pub justify_self_own_axis: bool,
    /// Preserve line-left/line-right separately from flow-relative start/end.
    pub justify_self_physical: Option<bool>,
    /// `last baseline`: запасное выравнивание — `end`, а не `start`
    /// (css-align-3 §9.3; `align-self-static-position-008`,
    /// `justify-self-static-position-001`). `Align::Baseline` его не различает.
    pub align_self_last: bool,
    /// Авторское объявление `align-self` (внешний `Some`), с его значением.
    /// Нужно, чтобы отличать значение автора от приёмов сборки, которые
    /// пишут в то же поле `align_self` (блок собран колонкой flex): только
    /// авторское значение гасится у коробки вне гибкого контейнера и сетки
    /// (`style::cascade::inherit`), и только оно же переходит по `inherit` к детям —
    /// вычисленное значение родителя от гашения не меняется (css-align-3
    /// §6.1 «Applies to: flex items, grid items, and absolutely-positioned
    /// boxes»; css-cascade-4 §7.3).
    pub align_self_decl: Option<Option<Align>>,
    pub justify_self_last: bool,
    /// `align-items: last baseline` — то же для умолчания детей: раскладка
    /// получает `LastBaseline` (css-align-3 §4.2), а не первую базовую.
    pub align_items_last: bool,
    /// `justify-items: last baseline` — для лунок-колонок (поперёк лунки).
    pub justify_items_last: bool,
    pub align_items_safe: bool,
    pub justify_content_safe: bool,
    pub align_content_safe: bool,
    /// Alignment properties declared with `flow-start`/`flow-end`, one bit
    /// per property (`cascade::inherit::flow_align`).
    pub align_flow: u8,
    pub justify_self: Option<Align>,
    /// Explicit normal must not take the parent's justify-items value.
    pub justify_self_normal: bool,
    pub webkit_box: Option<bool>,
    /// Элемент КОЛОНКИ гибкого контейнера: определён ли главный размер
    /// контейнера (css-flexbox-1 §9.8 п.1). `None` — не элемент колонки.
    pub flex_main_def: Option<bool>,
    /// Элемент ГИБКОГО контейнера (родитель `display: flex | inline-flex`).
    /// Ставится сборкой детей ряда/колонки в `render::blocks`, не каскадом и
    /// не наследуется (`style::cascade::inherit` клонирует СВОЙ стиль ребёнка). Нужен
    /// таблице: у неё гибким элементом становится обёртка с подписями
    /// (css-flexbox-1 §4).
    pub flex_item: bool,
    /// Physical main axis of the parent flex container, for replaced auto minimums.
    pub flex_main_vertical: bool,
    /// Flex item of a MULTI-line flex container (`flex-wrap` other than `nowrap`):
    /// its line's cross size comes from the items' hypothetical sizes, so an
    /// orthogonal item cannot take its inline size from the line. Set with
    /// `flex_item` by `render::blocks`; not inherited.
    pub flex_line_wraps: bool,
    /// An explicit unsafe overflow position for block content alignment.
    pub align_content_unsafe: bool,
}
