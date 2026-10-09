//! Предикаты коробок: строчные, блочные, в потоке, свой контекст, замещаемые.

use crate::dom::{Element, Node};
use crate::layout::block::struts::inline_axis_edges;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display};
use crate::text::text_box::blank_text;

/// Обтекание: плавающий блок и следующие за ним встают в один ряд.
///
/// Своего обтекания в раскладке нет и быть не может — оно определено через
/// строчный контекст, которого taffy не знает. Но ровно то, ради чего его
/// пишут — «картинка слева, текст справа» — выражается рядом из двух колонок
/// точно. Отличие от браузера одно: текст не заворачивается ПОД плавающий
/// блок, когда тот кончился. `clear` закрывает ряд и начинает новый.
/// Уходит ли элемент из потока: плавающие и внепоточные строчного не рвут
/// (Blink `layout_inline.cc`: разрыв вызывают только блоки В ПОТОКЕ).
pub(crate) fn out_of_flow(c: &Computed) -> bool {
    c.float.is_some_and(|f| f != 0)
        || matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
        )
}

/// Настоящий ли это строчный элемент.
///
/// `display: inline` хранится как строчная коробка с пометкой — по одному
/// лишь тегу судить нельзя: `<div style="display:inline">` строчный, а
/// `<span style="display:block">` блочный.
pub(crate) fn real_inline(e: &Element) -> bool {
    // Атомарные строчные — кнопка, поле, список выбора и замещаемые — стоят
    // в строке целиком, и содержимое их не разрывает: рвутся только
    // НЕзамещаемые строчные коробки (CSS 2.1 §9.2.1.1).
    const ATOMIC: &[&str] = &[
        "button", "select", "textarea", "input", "img", "svg", "canvas", "video", "audio",
        "object", "embed", "iframe", "meter", "progress",
    ];
    if ATOMIC.contains(&e.tag.as_str()) {
        return false;
    }
    if e.style.inline_display == Some(true) {
        return true;
    }
    match e.style.display {
        Some(_) => false,
        None => e.inline || crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Блочный ли это узел с точки зрения разрыва строчного.
pub(crate) fn breaks_inline(n: &Node) -> bool {
    let Node::Element(e) = n else { return false };
    block_level_in_flow(e)
}

/// Внутрипоточная коробка БЛОЧНОГО уровня (CSS 2.1 §9.2.1): не флоат и не
/// абсолют, не настоящая строчная, и вид — блочный (`display`, а при пустом
/// `display` — блочный тег). Ею же решается, кто спаннер (`spanner_box`:
/// css-multicol-1 §column-span «Applies to: in-flow block-level elements»).
pub(crate) fn block_level_in_flow(e: &Element) -> bool {
    if out_of_flow(&e.style) || real_inline(e) {
        return false;
    }
    // Рвут строку только НАСТОЯЩИЕ блочные виды. Внутренние части таблицы
    // (ряд, ячейка, группа) сами по себе разрыва не вызывают: вокруг них
    // сборщик строит анонимную таблицу, и её судьба решается отдельно.
    match e.style.display {
        // Строчные лунки строку НЕ рвут — внешний вид у них `inline`.
        Some(Display::GridLanes) => !e.style.lanes_inline,
        Some(Display::Block)
        | Some(Display::Flex)
        | Some(Display::Grid)
        | Some(Display::Table)
        | Some(Display::ListItem) => true,
        Some(_) => false,
        None => !e.inline && !crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Есть ли в поддереве строчного блочный потомок в потоке.
///
/// `display: contents` своей коробки не даёт — блок из-под него виден
/// строчному хозяину как свой (css-display-3 §box-generation).
pub(crate) fn contains_block(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Element(e) if real_inline(e) || e.style.display == Some(Display::Contents) => {
            !out_of_flow(&e.style) && contains_block(&e.children)
        }
        other => breaks_inline(other),
    })
}

pub(crate) fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}

/// Схлопываются ли отступы этого элемента с соседями и родителем.
///
/// Схлопывание — свойство БЛОЧНОГО потока. Не схлопываются: плавающий блок,
/// абсолютный и всё строчного уровня (`inline-block` и родня) — у них поля
/// стоят как написаны. Без этой проверки поле плавающего ребёнка «протекало»
/// наружу и поднимало родителя, а ряд строчных коробок терял поля у всех,
/// кроме первой.
pub(crate) fn in_flow(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
        )
        && !matches!(
            c.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v168, `scout-clamp-2026-09g.md` CLAMP-BFC,
/// 1 хунк): контейнер `line-clamp` заводит свой контекст форматирования
/// (css-overflow-4 §5.3). Обещание +7. Замер срезом 113 пар вместе с
/// MC-OOF-COPIES: снятие обоих убрало −5 (`flex-container-fragmentation-010/011`,
/// `single-line-column-flex-fragmentation-029`, `grid-item-oof-009/010` →
/// «красное видно») при −2 плюсах. Обособление контекста у клэмпа рушит
/// фрагментацию гибкого контейнера: у копии фрагмента появляется свой
/// контекст, и внепоточные теряют содержащий блок. Возвращать вместе с
/// FRAG-OOF (внепоточные при фрагментации).
/// Заводит ли коробка СВОЙ блочный контекст форматирования: через её край
/// поля не схлопываются ни с детьми, ни насквозь (CSS 2.1 §8.3.1).
pub(crate) fn own_context(e: &Element) -> bool {
    // A table caption is a block container that is not a block box: it
    // establishes a new block formatting context (CSS 2.2 section 9.4.1), so
    // its children's margins stay inside it
    // (`margin-collapsing-in-table-caption-002`).
    own_context_style(&e.style)
        || (e.tag == "caption" && e.style.display.is_none())
        || e.style.is_caption == Some(true)
        // A table (UA `display: table`, not written into `display`) never
        // collapses through (CSS 2.2 section 17.4: the table wrapper box
        // establishes a block formatting context); as a body's last child it
        // let the preceding paragraph's end margin escape
        // (`visibility-collapse-border-spacing-002`).
        || (e.tag == "table" && e.style.display.is_none())
        // `continue: collapse` (`line-clamp: <N>`/`auto`, у легаси — пара
        // `-webkit-box` по вертикали) делает блочный контейнер line-clamp
        // контейнером — НЕЗАВИСИМЫМ блочным контекстом (css-overflow-4
        // §continue «must establish an independent formatting context»,
        // §line-clamp-containers): поле первого ребёнка через его верх не
        // схлопывается (`line-clamp-auto-027`). Только собственный стиль:
        // слитый несёт `line_clamp` потомкам для текста.
        || ((e.style.clamp_lines().is_some() || e.style.clamp_auto == Some(true))
            && !multicol_container(&e.style))
}

/// То же по ОДНОМУ СТИЛЮ, без узла: содержащий блок приходит в `blocks()`
/// только своим `Computed`, а знать про его край надо и там.
pub(crate) fn own_context_style(c: &Computed) -> bool {
    !matches!(
        c.overflow_y,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || !matches!(
        c.overflow_x,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || matches!(
        c.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::InlineBlock)
            | Some(Display::Table)
            | Some(Display::InlineTable)
    ) || matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) || c.float.is_some()
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
        || c.contain_size == Some(true)
        || c.flow_root == Some(true)
        // css-align-3 §align-block: не-`normal` `align-content` на блочном
        // контейнере — тот же `display: flow-root`, что пишет эталон
        // `align-content-block-001-ref`. Через край такой коробки поля не
        // схлопываются ни с детьми, ни насквозь.
        || c.align_content_block
        || matches!(c.display, Some(Display::TableCell))
        || c.column_count.is_some()
        || c.column_width.is_some()
        // css-multicol-1 §column-span: «The element establishes an independent
        // formatting context … When 'column-span' is 'all', it always does» —
        // и вне многоколоночника тоже. Поля детей спаннера с его полями не
        // схлопываются (`multicol-span-all-margin-nested-firstchild-001`).
        || c.column_span == Some(true)
}

/// Замещаемый строчный атом: своих детей не имеет, но КОРОБКУ рождает —
/// значит, рождает и строчную коробку. Пустой `<span>` — не рождает.
pub(crate) fn replaced_inline(tag: &str) -> bool {
    matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input" | "br"
    )
}

/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную
/// коробку, в отличие от плавающего и абсолютного, которых в потоке нет.
/// Разбор держит `display: inline` как `InlineBlock` с пометкой
/// `inline_display`, поэтому одного взгляда на `display` мало.
pub(crate) fn atomic_inline(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
        )
        && c.inline_display != Some(true)
        && matches!(
            c.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
}

/// Блочная коробка со строчной ПОМЕТКОЙ: псевдоэлемент (`::before`/`::after`
/// помечается строчным независимо от `display`, `dom.rs`) или строчный тег с
/// блочным `display`. Раскладка (`breaks_inline`) кладёт такую коробку
/// блоком, а цепочки схлопывания полей пропускали её как строчную — и
/// `::after { display: flow-root; margin-top: 200px }` оставлял поле внутри
/// родителя вместо примыкания к его верху (`phantom-line-boxes-001…006`).
pub(crate) fn inline_marked_block(e: &Element) -> bool {
    e.inline
        && e.style.inline_display != Some(true)
        && matches!(
            e.style.display,
            Some(Display::Block)
                | Some(Display::ListItem)
                | Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::Table)
        )
}

/// Содержит ли коробка строчную коробку (§8.3.1, «does not contain a line
/// box»; нулевые строчные коробки §9.4.2 не в счёт).
///
/// Правила те же, что у `first_in_flow`: пробельный текст прозрачен,
/// непробельный рождает строку; ПУСТОЙ строчный элемент прозрачен, а
/// замещаемый атом (`img` и родня) — нет; вне потока строки не рождает никто;
/// `display: contents` своей коробки не даёт — смотреть надо в его детей.
pub(crate) fn holds_line_box(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(ch) => {
            if ch.style.display == Some(Display::None) {
                return false;
            }
            if ch.style.display == Some(Display::Contents) {
                return holds_line_box(&ch.children);
            }
            // Плавающий и абсолютный строчной коробки не рождают.
            if ch.style.float.is_some_and(|f| f != 0)
                || matches!(
                    ch.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
            {
                return false;
            }
            // Атомарный строчный в потоке — своя строчная коробка. Проверять
            // ДО `in_flow`: он их не различает и валит в одну корзину с
            // плавающим.
            if atomic_inline(&ch.style) {
                return true;
            }
            // `display: inline` делает строчным ЛЮБОЙ тег: своей коробки у
            // него нет, а строчную рождает его содержимое. Без этой ветки
            // `<div style="display:inline">` считался блочным ребёнком и
            // строки «не рождал», хотя текст внутри него её рождает.
            if ch.inline || ch.style.inline_display == Some(true) {
                // Блочный псевдоэлемент — блочный ребёнок: строки родителю
                // не рождает, его содержимое разбирает `through_strut`.
                if inline_marked_block(ch) {
                    return false;
                }
                // Пустой строчный с ненулевым полем/отступом/рамкой по
                // строчной оси — не фантом (css-inline-3
                // §invisible-line-boxes): строка есть, схлопывание насквозь
                // закрыто (`phantom-line-boxes-001…006`).
                return replaced_inline(&ch.tag)
                    || inline_axis_edges(&ch.style)
                    || holds_line_box(&ch.children);
            }
            // Блочный ребёнок строки не рождает: его содержимое разбирает
            // рекурсия `through_strut`.
            false
        }
    })
}

/// Ведущая цепочка примыкания к ВЕРХНЕМУ краю коробки (§8.3.1).
///
/// Верхнее поле коробки примыкает к верхнему полю её первого ребёнка в потоке.
/// Если тот схлопывается насквозь, примыкание тянется ВБОК, к следующему
/// брату; если не схлопывается — ВГЛУБЬ, к его собственному первому ребёнку.
///
/// Меряет ИММУТАБЕЛЬНО и копит пути до съеденных полей: обнулять на ходу
/// нельзя, потому что доля или `calc` на середине цепи заставят вернуть
/// `None`, а записанные нули уже не откатить.
/// Пустой строчный элемент без краёв по строчной оси — фантом
/// (css-inline-3 §invisible-line-boxes): строки не рождает, примыкания не
/// рвёт. Тот же признак, что у `leading_chain`.
pub(crate) fn phantom_inline(n: &Node) -> bool {
    matches!(n, Node::Element(s) if s.inline
        && !inline_marked_block(s)
        && s.children.is_empty()
        && !replaced_inline(&s.tag)
        && !inline_axis_edges(&s.style)
        && !atomic_inline(&s.style))
}

/// Строчный ли элемент по своему `display`.
pub(crate) fn inline_level(e: &Element) -> bool {
    match e.style.display {
        Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid) => true,
        // Строчный контейнер лунок — атом в строке, как inline-grid
        // (grid-lanes-align-content-001: четыре сетки стоят В РЯД).
        Some(Display::GridLanes) => e.style.lanes_inline,
        Some(_) => false,
        None => e.inline,
    }
}

/// Размер по содержимому (`width: min-content`/`max-content`).
///
/// У коробки такого размера раскладка под нами не знает — зато знает такую
/// ДОРОЖКУ СЕТКИ. Элемент заворачивается в сетку из одной дорожки нужного
/// вида: ширину она посчитает по содержимому и отдаст элементу. Обёртка
/// прижата к началу строки, иначе сетка растянула бы её саму на всю ширину
/// родителя и смысл потерялся.
/// Завернёт ли `content_sized` этот элемент в свою обёртку.
///
/// Отдельный предикат нужен вызывающей стороне: она обязана снять с элемента
/// боковые поля ДО сборки — обёртка их не пропускает.
/// Замещаемый элемент (css-display-3 §2.4): размер даёт содержимое, а не
/// раскладка детей.
pub(crate) fn replaced_tag(e: &Element) -> bool {
    matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input"
    )
}

/// Текст ДО первого жёсткого разрыва: дальше первая строка не идёт никогда.
///
/// Замер первой строки ищет, сколько знаков влезет по ширине, и про `<br>` он
/// не знает — с широкой коробкой в первую строку попадал весь абзац, и её
/// начертание доставалось второй строке тоже
/// (`text-autospace-first-line-001`).
/// Уровень коробки для схлопывания полей: тег — только УМОЛЧАНИЕ, вид из
/// каскада сильнее. `e.inline` ставится по имени тега (`dom.rs`), поэтому
/// `<span style="display:block">` доезжал сюда «строчным», и поля соседей
/// через него не примыкали — эталоны `flex-direction-column*` разводило на
/// лишние 16 точек (в них разметка именно такая).
pub(crate) fn inline_level_box(e: &Element) -> bool {
    match e.style.display {
        // Строчными считаются только НАСТОЯЩИЕ строчные виды. Первый заход
        // писал `Some(_) => true`, и в строчные попадали лунки сетки: CSS2
        // +6, а css-grid −46 (`column-align-items-*`, `*-dense-packing-*`
        // уходили 0.00 → 1.5-3.2). `display: inline` после каскада — это
        // `InlineBlock` с пометкой `inline_display` (см. `computed.rs`).
        Some(Display::InlineBlock)
        | Some(Display::InlineFlex)
        | Some(Display::InlineGrid)
        | Some(Display::InlineTable) => true,
        Some(_) => false,
        None => e.inline,
    }
}
