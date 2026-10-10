//! Определение собственного контекста форматирования элемента.

use crate::dom::Element;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::{Computed, Display};

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
