//! Предикаты позиционирования.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::page::paged::visible_overflow;
use crate::render::block_level_in_flow;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

// Несёт ли поддерево АБСОЛЮТНОГО потомка, чей низ `shape_full` сворачивает
// в меру коробки (дотяг `oof_reach`). Только такому ребёнку стопки колонок
// переполняющие колонки нужны ради внепоточного (css-position-3
// §abspos-breaking: «The box may subsequently be broken over several
// fragmentation containers»; Blink рождает их от внепоточного —
// `column_layout_algorithm.cc` `num_new_columns`). Спуск не идёт внутрь
// коробки, обрезающей переполнение (абсолют за ней в колонках не виден:
// `out-of-flow-in-multicolumn-107`, `overflow: clip` над абсолютом
// 100000px), и внутрь вложенного многоколоночника (у его абсолютов свои
// колонки). Глубина — та же, что у меры стопки (`shape_full(c, 4, ..)`).
thread_local! {
    /// Коробки, чью меру фрагментации дотянули внепоточные потомки
    /// (`shape_full`): `node_id -> (свой размер, мера с дотягом)`.
    pub(crate) static OOF_OWN: std::cell::RefCell<std::collections::HashMap<u64, (f32, f32)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn carries_abspos(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                k.style.position == Some(crate::style::computed::Position::Absolute)
                    || (visible_overflow(&k.style)
                        && !multicol_container(&k.style)
                        && carries_abspos(k, depth - 1))
            }
            _ => false,
        })
}

/// Рисуется ли ДАЛЬШЕ по разметке позиционированное содержимое первым
/// проходом родителя — тогда `PaintLast` перевернул бы порядок шага 8.
///
/// Шаг 8 приложения E CSS 2.1 красит позиционированные потомки контекста В
/// ПОРЯДКЕ РАЗМЕТКИ, а второй проход `Div` поднимает обёрнутого лишь над
/// братьями: позиционированный ВНУТРИ следующего обычного брата (ячейка
/// `relative` в таблице после абсолютного красного индикатора,
/// `position-relative-table-*`) или абсолют на статической позиции в позднем
/// слое (`font-029`) оказались бы под ним. Брат-блок `relative`/`sticky` с
/// `z-index: auto | 0` сам уходит во второй проход и порядок сохраняет — его
/// поддерево рисуется вместе с ним. `fixed` отложен и так.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО: `PaintLast` без этого гейта — на срезе 890 пар
/// +2/−20 (все `position-relative-table-*` и `font-029` в «красное видно»);
/// с гейтом срез 3483 пары: 2757 → 2764, +7/−0.
pub(crate) fn positioned_later(rest: &[Node]) -> bool {
    fn positioned(e: &Element) -> bool {
        matches!(
            e.style.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Sticky)
                | Some(crate::style::computed::Position::Absolute)
        )
    }
    fn walk(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            positioned(e) || walk(&e.children)
        })
    }
    rest.iter().any(|n| {
        let Node::Element(e) = n else { return false };
        let late_sibling = matches!(
            e.style.position,
            Some(crate::style::computed::Position::Relative) | Some(crate::style::computed::Position::Sticky)
        ) && e.style.z_index.unwrap_or(0) == 0
            && block_level_in_flow(e);
        if late_sibling {
            return false;
        }
        positioned(e) || walk(&e.children)
    })
}

/// Есть ли ДАЛЬШЕ по разметке позиционированный элемент, который останется на
/// месте.
///
/// Слой начального содержащего блока дописывается последним ребёнком
/// документа, поэтому вынесенный рисуется поверх всего, что осталось в потоке.
/// По CSS 2.1 §9.9 шаг 8 позиционированные с `z-index: auto` рисуются В
/// ПОРЯДКЕ РАЗМЕТКИ: сосед, стоящий ПОСЛЕ, обязан лечь СВЕРХУ. Пока он
/// остаётся на месте, вынос переворачивает пару местами.
///
/// Сосед, который сам уйдёт в слой, порядок НЕ ломает: слой копится в порядке
/// сборки. `fixed` не считается: он и так рисуется отложенно, поверх всего.
pub(crate) fn stays_positioned(rest: &[Node]) -> bool {
    fn walk(nodes: &[Node], under_cb: bool) -> bool {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            let pos = e.style.position;
            let positioned = matches!(
                pos,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
                    | Some(crate::style::computed::Position::Absolute)
            );
            // Тот же предикат, что и у выноса: такой сосед уедет в слой, и
            // взаимный порядок сохранится.
            let hoisted = pos == Some(crate::style::computed::Position::Absolute)
                && !under_cb
                && e.style.z_index.unwrap_or(0) >= 0
                && (edge_set(e.style.inset.left)
                    || edge_set(e.style.inset.right)
                    || edge_set(e.style.inset.top)
                    || edge_set(e.style.inset.bottom));
            // A negative `z-index` paints in the bottom layer (CSS 2.1 §9.9,
            // step 3) whatever its document position: hoisting an earlier
            // sibling cannot reorder against it (spec-examples
            // `shape-outside-001`: `#failure-container` kept `#test` in
            // place, positioned from the collapsed `body` top, 16px low).
            let below = e.style.z_index.is_some_and(|z| z < 0);
            if positioned && below {
                // …together with its whole subtree.
                return false;
            }
            if positioned && !hoisted {
                return true;
            }
            // Вынесенный сосед уезжает в слой ВМЕСТЕ с поддеревом: его
            // позиционированные потомки рисуются внутри него и порядок с
            // выносимым не ломают. Прежде абсолютный ребёнок такого соседа
            // держал элемент на месте, и края считались от `body`, а не от
            // окна (`backdrop-filters-*`: квадрат съезжал на поле тела).
            if hoisted {
                return false;
            }
            walk(
                &e.children,
                under_cb || crate::text::inline::establishes_cb(&e.style),
            )
        })
    }
    walk(rest, false)
}

/// Задан ли край позиционированного элемента.
///
/// `left: auto` — это ОТСУТСТВИЕ края (CSS 2.1 §9.3.2: начальное значение
/// `auto`), а разбор даёт на него `Some(Len::Auto)`. Проверка `is_some()`
/// читала явный `auto` как заданный край, и элемент терял статическую
/// позицию: `abspos-*-applies-to-*` вставали в угол содержащего блока
/// вместо своего места в потоке.
pub(crate) fn edge_set(l: Option<Len>) -> bool {
    !matches!(l, None | Some(Len::Auto))
}

/// Есть ли у инлайнового куска собственная коробка.
///
/// Прогон текста не умеет рисовать вокруг себя ничего: ни рамку, ни тень, ни
/// отступ. Раньше проверялись только верх и лево, поэтому `padding-right`,
/// боковая рамка, тень, прозрачность и три угла из четырёх у `<span>` молча
/// пропадали. У настоящего `display: inline` размеры не проверяются: CSS их
/// такому элементу и не даёт.
pub(crate) fn has_own_box(c: &Computed, font_px: f32) -> bool {
    // Нулевая величина коробки не создаёт: `padding: 0` и `border: 0` пишут
    // в стиль ноль, и по одному лишь «задано» кусок вынимался из строки —
    // а вынутый кусок рвёт соединение букв и общий перенос по словам.
    let set = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let any =
        |s: &crate::style::computed::Sides| set(&s.top) || set(&s.right) || set(&s.bottom) || set(&s.left);
    // Вертикальные поля признаком коробки НЕ служат: строку они не двигают
    // (замерено на `flexbox_inline`, где `margin-top: -20em` обязан пройти
    // впустую), и по ним коробка заводилась бы только затем, чтобы уехать за
    // экран.
    // Атомарная строчная коробка — коробка по определению: у неё свои ширина,
    // высота и вертикальные поля, а прогон текста не умеет ни одного из трёх.
    //
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: раньше коробкой считался только атом С
    // ЗАДАННЫМ размером, а безразмерный оставался прогоном — под флагом
    // `ATOM_BOX`, потому что прошлый замер давал flexbox 363→359 (атом-коробка
    // не отдавала строке базовую линию содержимого). Сейчас не
    // воспроизводится: CSS2 5039 → 5043, oldfront 2345 без изменений. Внутри
    // такой коробки живут отступ первой строки, сжатие по содержимому и свой
    // перенос — прогон их не знает.
    // Настоящий `display: inline` сюда НЕ входит: разбор держит его как
    // `InlineBlock` с пометкой `inline_display`, и без этой отсечки каждый
    // `<span>` становился атомарной коробкой — вместе с ней уезжали
    // сохранённые пробелы и перенос (`white-space-pre-005`).
    let atomic = (matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && c.inline_display != Some(true))
        || (c.display == Some(Display::GridLanes) && c.lanes_inline);
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: прежний замер (flexbox 363→359 из-за
    // непрокинутой базовой линии атома) больше не воспроизводится — CSS2
    // 5039 → 5043, oldfront 2345 без изменений. Флаг `ATOM_BOX`, под которым
    // проба жила, снят.
    // Позиционированный кусок — тем же порядком: его коробку двигают края, а
    // краёв у прогона нет.
    // ПРОБОВАЛИ И ОТКАТИЛИ: считать коробкой и `position: relative`, чтобы
    // относительный `<span>` служил содержащим блоком абсолютным потомкам
    // (по CSS это так). Выигрыш нулевой во всех разделах, css-grid 389 → 386.
    // Возвращать вместе с настоящей коробкой строчного фрагмента.
    let positioned = matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    );
    if atomic || positioned {
        return true;
    }
    // Сплошной фон коробки не требует: его несёт прогон текста, и тогда
    // подсветка переносится вместе со строкой. Раньше `<span>` с фоном
    // становился отдельным блоком, и его слова вставали столбиком.
    // ПРОЗРАЧНАЯ рамка не рисует ничего, поэтому и коробки не требует: место
    // под неё держит знак-распорка строки. Пока `border: solid transparent`
    // заводил коробку, `<span>` с такой рамкой рвал абзац на части
    // (`word-space-transform-010`, где рамка ровно для того и прозрачная,
    // чтобы проверить одни только отступы).
    // РОВНУЮ рамку рисует прогон текста по кускам строк, коробка ей не нужна
    // (`inline::uniform_border`): в коробке текст перестаёт переноситься
    // вместе с абзацем, и `<span>` с рамкой уезжал одной строкой за край
    // (`hanging-punctuation-inline-bound-001`).
    // …и только у СТРОЧНОГО уровня: у блока рамка принадлежит его коробке, и
    // без неё он теряет и фон, и поля (`flexbox_first-line`: `li` с рамкой в
    // 1px разъезжался на четверть страницы).
    // ЗАМЕРЕНО И ОТКАЧЕНО: отдавать прогону ТОЛЬКО ровную рамку, а рамку с
    // разными гранями уводить в коробку. По семьям `borders/*` и `css1/*brdr*`
    // это +19/−1, но по всему CSS2 — 4917 → 4906: семьи `bidi-*` теряют 33
    // пары, потому что кусок, разорванный переносом, коробке не даётся.
    // Порог «все ЗАДАННЫЕ грани одной толщины» тоже замерен: +13/−14
    // (`border-top-width-0NN` уходят в прогон и там ложатся мимо). Настоящая
    // развилка — геометрия полосы: 1.16 кегля вместо подъёма и спуска шрифта,
    // и рамка внутрь вместо наружу. Возвращаться вместе с ней.
    // Явный `display: inline` (`div { display: inline }`) — тот же строчный
    // уровень: разбор держит его как `InlineBlock` с `inline_display`, и
    // `div` с рамкой уходил в коробку — с вертикальными полями и рамкой
    // ВНУТРИ строки (`margin-top-applies-to-008`, §10.6.1: вертикальные
    // поля строчной коробки на строку не действуют).
    // Graphical effects of the inline box (css-masking-1 §1: `clip-path` and
    // `mask` apply to all elements; css-color-4 §opacity) act on everything
    // the box paints, its border and background included. A text run applies
    // none of them, so the box is needed even when the run could paint the
    // border itself (`clip-path-inline-006`: the red border stayed unclipped).
    if c.opacity.is_some() || c.mask_image.is_some() {
        return true;
    }
    let inline_level = c.display.is_none() || c.inline_display == Some(true);
    if inline_level
        && (crate::text::inline::uniform_border(c, font_px).is_some()
            || crate::text::inline::sided_border(c, font_px).is_some())
    {
        return false;
    }
    let visible = |col: &Option<crate::style::values::value::Color>| col.is_some_and(|x| x.a > 0.0);
    let colored = c.border_color.is_some() || c.border_colors.iter().any(Option::is_some);
    let border_paints = visible(&c.border_color)
        || c.border_colors.iter().any(visible)
        // Цвет не задан вовсе — рамка красится цветом текста, то есть видна.
        || (!colored && any(&c.borders()));
    c.gradient.is_some()
        || c.bg_image.is_some()
        || border_paints
        // Отступ и поле СТРОЧНОЙ коробки рисуют пустоту: место под них
        // держит знак-распорка внутри строки (`inline_sides`). Пока они
        // заводили коробку, строка рвалась по краю `<span>` — иероглифы
        // расходились по разным строкам, а коробки выходили разной ширины
        // (`word-space-transform-010`).
        // Скругление подсветки рисует прогон вместе с её фоном.
        || (c.background.is_none()
            && (set(&c.radius.tl)
                || set(&c.radius.tr)
                || set(&c.radius.br)
                || set(&c.radius.bl)))
        || !c.shadows.is_empty()
        || c.opacity.is_some()
        // Контур на раскладку не влияет ВООБЩЕ (css-ui §2): он рисуется за
        // краем коробки и места не занимает. Строчному куску коробку он
        // поэтому не заводит — иначе `<span>` с контуром переставал
        // переноситься вместе с абзацем и уезжал одной строкой за край
        // (`text-autospace-break-001`). Рисует его прогон строки, как и
        // ровную рамку (см. `inline::uniform_border`).
        || (!inline_level && c.outline.is_some())
}
