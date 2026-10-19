//! Определение собственного контекста форматирования элемента.

use crate::dom::Element;
use crate::layout::multicol::spanner::multicol_container;
pub(crate) use crate::style::computed::formatting_context::own_context_style;

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
