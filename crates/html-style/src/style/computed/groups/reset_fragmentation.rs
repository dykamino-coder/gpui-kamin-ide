//! CSS-операнды группы fragmentation; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::{Len, StyleColor};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetFragmentation {
    /// `-webkit-line-clamp`: сколько строк оставить.
    pub line_clamp: Option<u32>,
    /// `auto` в `max-lines` (css-overflow-4 §max-lines): контейнер обрезки
    /// режет и по своей высоте; `None` — начальное `auto`.
    pub clamp_auto: Option<bool>,
    /// `column-count` — на сколько колонок резать содержимое.
    pub column_count: Option<u16>,
    /// `column-width` — минимальная ширина колонки.
    pub column_width: Option<Len>,
    /// `column-height` (css-multicol-2 §ch) — заданная высота колонки;
    /// `auto` хранится отсутствием значения.
    pub column_height: Option<Len>,
    /// `column-wrap` (css-multicol-2 §cwr): `Some(true)` — `wrap`, лишние
    /// колонки уходят в новый ряд; `Some(false)` — `nowrap`, вбок; `None` —
    /// `auto`: как `wrap` при заданном `column-height`, иначе `nowrap`.
    pub column_wrap: Option<bool>,
    /// `column-gap` — зазор между колонками многоколоночного потока.
    /// Умолчание CSS — `normal`, то есть один кегль.
    pub column_gap: Option<Len>,
    /// An ancestor is a multi-column container: the box may be fragmented
    /// across columns (css-break-3 §box-splitting). Set by inheritance.
    pub in_multicol: bool,
    /// `column-fill: auto` — колонки заполняются по очереди, без баланса.
    pub column_fill_auto: Option<bool>,
    /// `column-span: all` — блок растянут на все колонки.
    pub column_span: Option<bool>,
    /// `page: <custom-ident>` — именованная страница (css-page-3 §"Using named
    /// pages"). `auto` хранится отсутствием значения: используемое значение
    /// берётся у ближайшего предка с именем (там же, шаг 1 алгоритма).
    pub page: Option<String>,
    /// `break-inside: avoid*` — коробку нельзя разрывать между колонками и
    /// страницами (css-break-3 §4.1). Свойство не разбиралось вовсе, и
    /// отличить монолит от обычной коробки было нечем.
    pub break_inside_avoid: bool,
    /// `box-decoration-break: clone` (css-break-4 §break-decoration): «Each
    /// box fragment is independently wrapped with the border, padding, and
    /// margin … The background is drawn independently in each fragment».
    /// Не наследуется; начальное `slice` = `false`.
    /// Укладку `clone` не делать веткой в общей `fill_at`: она ломает уже
    /// работавший `slice` у вложенных случаев (`clone-005.tentative`,
    /// `clone-007`); нужен отдельный проход планирования фрагментов.
    pub bdb_clone: bool,
    /// `break-before`/`break-after` (css-break-4 §3.1): принудительный разрыв
    /// колонки/страницы перед или после коробки.
    pub break_before_force: bool,
    pub break_after_force: bool,
    /// Те же свойства со ЗАПРЕЩАЮЩИМИ значениями — css-break-4 §3.1 «avoid
    /// break values»: `avoid`, `avoid-page`, `avoid-column`, `avoid-region`.
    /// Правило 1 §4.3: «A fragmented flow may break at a class A break point
    /// only if all the break-after and break-before values applicable to this
    /// break point allow it». Разбирались ТОЛЬКО принудительные значения, и
    /// сообщить движку запрет разрыва МЕЖДУ соседями было нечем.
    pub break_before_avoid: bool,
    pub break_after_avoid: bool,
    /// `margin-trim` (css-box-4 §margin-trim): биты обрезаемых ЛОГИЧЕСКИХ
    /// краёв: 1 — `block-start`, 2 — `block-end`, 4 — `inline-start`,
    /// 8 — `inline-end`. Начальное `none` (0). Блочный контейнер исполняет
    /// только блочные биты (`render::collapse_margins`), гибкий и сетка —
    /// все четыре (раскладка, `apply` переводит их в физические).
    pub margin_trim: u8,
    /// `column-rule-*`: линейка между колонками.
    pub column_rule_width: Option<Len>,
    pub column_rule_visible: Option<bool>,
    pub column_rule_color: StyleColor,
    /// `row-rule-*` (css-gaps-1 §color-style-width): линейка в ПОПЕРЕЧНОМ
    /// промежутке сетки/гибкого контейнера. Начальные значения те же, что у
    /// `column-rule-*`: `currentcolor`, `none`, `medium` — то есть без
    /// заданного стиля линейки нет.
    pub row_rule_width: Option<Len>,
    pub row_rule_visible: Option<bool>,
    pub row_rule_color: StyleColor,
    /// `column-rule-break`/`row-rule-break` (css-gaps-1 §break): 0 — `none`,
    /// 1 — `normal` (начальное), 2 — `intersection`. Шаг 1 разбирает
    /// значение, но рисует всегда непрерывно: в сетке БЕЗ спанов все стыки
    /// крестовые, и `normal` по спеке проходит сквозь них.
    pub column_rule_break: Option<u8>,
    pub row_rule_break: Option<u8>,
    /// Списки значений линеек по промежуткам (css-gaps-1 §lists); `None` —
    /// значение одно и лежит в скалярных полях выше.
    pub column_rule_widths: Option<GapList<Len>>,
    pub column_rule_styles: Option<GapList<bool>>,
    /// Single `double` rule style (css-gaps-1 §color-style-width: line styles
    /// as for borders) — painted as two lines; other styles stay solid.
    pub column_rule_double: bool,
    pub row_rule_double: bool,
    /// Rule line kinds per gap (§lists): 0 solid-like, 1 `dashed`,
    /// 2 `dotted`, 3 `double` — painted as the border side of that style
    /// (Blink `BoxBorderPainter::DrawBoxSide`).
    pub column_rule_kinds: Option<GapList<u8>>,
    pub row_rule_kinds: Option<GapList<u8>>,
    pub column_rule_colors: Option<GapList<StyleColor>>,
    pub row_rule_widths: Option<GapList<Len>>,
    pub row_rule_styles: Option<GapList<bool>>,
    pub row_rule_colors: Option<GapList<StyleColor>>,
    /// §inset: [cap-start, cap-end, junction-start, junction-end]; начальное 0.
    pub column_rule_inset: Option<[GapInset; 4]>,
    pub row_rule_inset: Option<[GapInset; 4]>,
    /// §visibility-items: 0 `normal`, 1 `all`, 2 `around`, 3 `between`.
    pub column_rule_visibility: Option<u8>,
    pub row_rule_visibility: Option<u8>,
    /// `rule-overlap: column-over-row` — колонки поверх рядов.
    pub rule_column_over_row: Option<bool>,
    /// `continue: collapse` (css-overflow-4 §5.4): коробка — контейнер
    /// обрезки строк. Не наследуется; `-webkit-legacy` — `clamp_legacy`.
    pub clamp_continue: Option<bool>,
    /// `continue: discard` (css-overflow-4 §continue): содержимое после
    /// первого разрыва региона коробки (у многоколоночника — переполняющие
    /// колонки) не рисуется. Не наследуется.
    pub clamp_discard: Option<bool>,
}
