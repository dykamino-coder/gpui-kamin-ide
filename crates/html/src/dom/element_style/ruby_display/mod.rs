//! Ruby display for element_style; split out to keep the owning module within 250 lines.

mod direction;
pub(crate) use crate::dom::element_style::ruby_display::direction::apply_direction;
pub(crate) use crate::dom::element_style::ruby_display::direction::inlinify_in_ruby;

use crate::dom::*;
use crate::style::computed::{Computed, Position};
use crate::style::values::value::Len;

pub(crate) fn finish_inline_display(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    use crate::style::computed::Display;
    replaced_display::normalize(style, tag, attrs);
    let out_of_flow = style.float.is_some()
        || matches!(
            style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        );
    // Блокификация СТРОЧНЫХ вариантов под float/abspos (§9.7): каждый
    // получает свой блочный аналог, а не только `inline`.
    // §9.7: у абсолютно позиционированной коробки `float` вычисляется в
    // `none`. Пока сброса не было, `float: right; position: fixed` уезжал в
    // ряд обтекания и до выноса в слой окна не доходил (`position-fixed-007`).
    if matches!(
        style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        style.float = None;
        // Блокифицированная коробка строчного выравнивания не имеет
        // (`vertical-align` «applies to inline-level and table-cell
        // elements», CSS 2.1 §10.8.1): статическая позиция абсолюта — та же,
        // что без `sub` (`vertical-align-sub-001`: зелёный уезжал вниз и
        // открывал красный).
        style.apply_one("vertical-align", "baseline");
    }
    if out_of_flow {
        if matches!(
            style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) && matches!(
            style.display,
            Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
                | Some(Display::InlineBlock)
        ) {
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
    // не действует. Первый заход давал 0 и 0, потому что размещение самой
    // коробки рядом с флоатом тогда ещё было сломано.
    if matches!(
        style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineTable)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
    ) || style.is_caption == Some(true)
        // Абсолютная коробка вне потока: флоатов выше неё в её контексте нет,
        // и очищать нечего (§9.5.2 действует на поток).
        || matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
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
    // ПРОБОВАЛИ И ОТКАТИЛИ: обнулять боковое `auto`-поле и у коробок
    // СТРОЧНОГО уровня (§10.3.2, §10.3.9). CSS2 +1, но oldfront 2343 -> 2341:
    // у ЭЛЕМЕНТА ГИБКОГО контейнера `auto`-поле законно и забирает свободное
    // место (css-flexbox §8.1), а родителя эта функция не видит.
    // Возвращать вместе с признаком «ребёнок гибкого контейнера».
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
    // нужен (`render.rs`: `inline_level(e) || inline_display`).
    //
    // ★ ЗАМЕРЕНО: CSS2 5316 -> 5320, CSS3 2419 -> 2416 (в своде было 2415,
    // но `css-flexbox-height-animation-stretch` мигает: 1.33 / 0.00 / 1.10 на
    // одном и том же бинаре). Итого +1. Приобретено: `absolute-non-replaced-
    // width-020`, `float-non-replaced-width-008`, `clear-float-004`,
    // `floats-025`, `floats-145`. Потеряно: `float-nowrap-3/-9`,
    // `float-nowrap-hyphen-rewind-1` и тройка `text-justify-*-001` (у них
    // плавающий `<span>` стоит в ЭТАЛОНЕ) — все по одной причине: наш флоат
    // уходит в отдельный ряд обтекания и рядом со своей строкой уже не стоит.
    // Убирается настоящей коробкой флоата В строке, а не откатом блокификации.
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
