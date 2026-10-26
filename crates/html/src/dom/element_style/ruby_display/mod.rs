//! Доводка `display` элемента после каскада: блокификация и строчный уровень.
//! `finish_inline_display` приводит стиль к используемому виду коробки
//! (CSS 2.1 §9.7): у абсолютной `float` → `none` и `vertical-align: baseline`,
//! строчные и внутренние табличные виды вне потока блокифицируются вместе со
//! снятием табличных ролей; `clear` и поля снимаются там, где не применяются
//! (§9.5.2, §8.3); боковые `auto`-поля флоата — ноль (§10.3.5); голый
//! строчный тег с `float` становится блоком; затем `containment::normalize`
//! и сброс размеров у неатомарной строчной коробки. Подмодуль `direction`.

use crate::document::state::ValueCx;
mod direction;
pub(crate) use crate::dom::element_style::ruby_display::direction::apply_direction;
pub(crate) use crate::dom::element_style::ruby_display::direction::inlinify_in_ruby;

use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(crate) fn finish_inline_display(
    style: &mut Computed,
    tag: &str,
    attrs: &[(String, String)],
    vcx: ValueCx<'_>,
) {
    use crate::style::computed::Display;
    replaced_display::normalize(style, tag, attrs);
    let out_of_flow = crate::style::flow::has_float_or_absolute_position(style);
    // Блокификация СТРОЧНЫХ вариантов под float/abspos (§9.7): каждый
    // получает свой блочный аналог, а не только `inline`.
    // §9.7: у абсолютно позиционированной коробки `float` вычисляется в
    // `none`. Пока сброса не было, `float: right; position: fixed` уезжал в
    // ряд обтекания и до выноса в слой окна не доходил (`position-fixed-007`).
    if crate::style::flow::absolutely_positioned(style.position) {
        style.float = None;
        // Блокифицированная коробка строчного выравнивания не имеет
        // (`vertical-align` «applies to inline-level and table-cell
        // elements», CSS 2.1 §10.8.1): статическая позиция абсолюта — та же,
        // что без `sub` (`vertical-align-sub-001`: зелёный уезжал вниз и
        // открывал красный).
        style.apply_one("vertical-align", "baseline", vcx);
    }
    if out_of_flow {
        if crate::style::flow::absolutely_positioned(style.position)
            && matches!(
                style.display,
                Some(Display::InlineFlex)
                    | Some(Display::InlineGrid)
                    | Some(Display::InlineTable)
                    | Some(Display::InlineBlock)
            )
        {
            style.abs_inline_level = true;
        }
        match style.display {
            Some(Display::InlineFlex) => style.display = Some(Display::Flex),
            Some(Display::InlineGrid) => style.display = Some(Display::Grid),
            Some(Display::InlineTable) => style.display = Some(Display::Table),
            Some(Display::InlineBlock) => style.display = Some(Display::Block),
            // ВНУТРЕННИЕ табличные виды блокифицируются в `block`, а не в
            // `table` (§9.7 вместе с §9.2.4): вне потока ряд, группа рядов и
            // ячейка своей таблицы уже не образуют. Колонка коробки не
            // порождала вовсе (`Display::None`), и вне потока квадрат просто
            // не рисовался (`top-applies-to-006`).
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell) => {
                style.display = Some(Display::Block)
            }
            Some(Display::None) if style.col_role.is_some() => style.display = Some(Display::Block),
            _ => {}
        }
        // Метки табличных ролей снимаются вместе с видом: иначе таблица
        // подобрала бы вне-поточный узел обратно в решётку (§17.2.1).
        if matches!(style.display, Some(Display::Block)) {
            style.col_role = None;
            style.row_group_kind = None;
            style.is_caption = None;
        }
    }
    // §9.5.2 «Applies to: block-level»: у коробки НЕ блочного уровня `clear`
    // не действует.
    if matches!(
        style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineTable)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
    ) || style.is_caption == Some(true)
        // Абсолютная коробка вне потока: флоатов выше неё в её контексте нет,
        // и очищать нечего (§9.5.2 действует на поток).
        || crate::style::flow::absolutely_positioned(style.position)
    {
        style.clear = None;
        style.clear_inherit = false;
    }
    //
    // Поля к внутренним табличным видам НЕ применяются (§8.3), `clear` — только
    // к коробкам блочного УРОВНЯ (§9.5.2). Заголовок сюда не входит: он
    // блочная коробка, и поля у него законны.
    if matches!(
        style.display,
        Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
    ) || style.col_role.is_some()
    {
        style.margin = Default::default();
        style.clear = None;
        style.clear_inherit = false;
    }
    // Боковые auto-поля ПЛАВАЮЩЕГО используются нулём (§10.3.5): распирать
    // флоат от его края им нечем. До taffy `auto` доезжало как есть и уводило
    // коробку на всю свободную ширину. `float: none` разбирается в `Some(0)` —
    // поэтому сравнение со значением, а не `is_some`.
    // Не обнулять боковое `auto`-поле у коробок СТРОЧНОГО уровня (§10.3.2,
    // §10.3.9): у ЭЛЕМЕНТА ГИБКОГО контейнера `auto`-поле законно и забирает
    // свободное место (css-flexbox §8.1), а родителя эта функция не видит —
    // сначала нужен признак «ребёнок гибкого контейнера».
    if style.float.is_some_and(|f| f != 0) {
        if style.margin.left == Some(Len::Auto) {
            style.margin.left = Some(Len::Px(0.0));
        }
        if style.margin.right == Some(Len::Auto) {
            style.margin.right = Some(Len::Px(0.0));
        }
    }
    // §9.7: плавающий блокифицируется — и тот, чей строчный уровень идёт от
    // ИМЕНИ ТЕГА, а не от объявленного `display`. Пометка `inline_display`
    // ставится только на дословный `display: inline`, поэтому голый
    // `<span style="float:left">` до блокификации не доезжал вовсе: ширина и
    // высота на нём не применялись, и вместо коробки 120x120 рисовался кусок
    // строки по кеглю (`absolute-non-replaced-width-020/024`).
    // Только ПЛАВАЮЩИЙ: у абсолютного статическая позиция считается по
    // гипотезе §10.3.7 «если бы position был static», и строчный уровень ей
    // нужен (`layout::box_kind`: `inline_level(e) || inline_display`).
    //
    // Известная цена блокификации: `float-nowrap-3/-9`,
    // `float-nowrap-hyphen-rewind-1` и тройка `text-justify-*-001` (у них
    // плавающий `<span>` стоит в ЭТАЛОНЕ) — наш флоат уходит в отдельный ряд
    // обтекания и рядом со своей строкой уже не стоит. Убирается настоящей
    // коробкой флоата В строке, а не откатом блокификации.
    if style.float.is_some_and(|f| f != 0) && style.display.is_none() && INLINE_TAGS.contains(&tag)
    {
        style.display = Some(Display::Block);
    }
    containment::normalize(style, tag, out_of_flow);
    if style.inline_display != Some(true) {
        return;
    }
    if out_of_flow {
        style.display = Some(Display::Block);
        return;
    }
    let replaced = matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input"
    );
    if !replaced {
        style.width = None;
        style.height = None;
        style.min_width = None;
        style.min_height = None;
        style.max_width = None;
        style.max_height = None;
    }
}
