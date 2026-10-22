//! Рекордер главной копии ребёнка стопки колонок: у ребёнка, чьи точки разрыва даёт
//! раскладка копии, — измеренные строки (`Paragraph`, BandFlow) и блочная стопка
//! (`flow::blocks`). Это ребёнок,
//! чью высоту меряет раскладка (`StackChild::measure`), и поддерево обычных блоков
//! (`plain_block_tree`), таблица из них (`table_tree`) или поддерево с сеткой (`grid_tree`):
//! его границы классa A, поля на разрыве, запреты, монолиты, ряды таблиц и сеток берутся из
//! разложенных border box и дорожек, а не из оценки формы.

use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::blocks::{BlockBox, grid_tree, table_tree};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::probe::plain_block_tree;
use crate::layout::fragment::types::BreaksAt;
use crate::layout::options::RenderOpts;
use std::cell::RefCell;
use std::rc::Rc;

/// Рекордер копии и ширина её раскладки до укладки (`StackChild::breaks_w`).
/// `own_plan` — у ребёнка свой план фрагментов (клон украшений, целый или
/// рядами вложенный многоколоночник): его точки остаются у оценки.
pub(super) fn recorder(
    copy: &Element,
    measured_kids: &RefCell<Vec<(u64, f32)>>,
    opts: &RenderOpts,
    col_vert: bool,
    line_col_w: Option<f32>,
    own_plan: bool,
) -> (Rc<Watched>, Option<BreaksAt>) {
    if col_vert {
        return (Watched::new(&[]), None);
    }
    let measured = measured_kids
        .borrow()
        .iter()
        .any(|&(id, _)| id == copy.node_id);
    // A plain block tree is measured also when its column width is known only
    // after the stack box is laid out (`BreaksAt::Column`); a table or grid
    // tree only at a width known before (`kid_w`), its rows recorded by the
    // layout.
    let plain = !own_plan && plain_block_tree(copy, 4);
    let rows = !own_plan
        && line_col_w.is_some()
        && (table_tree(copy, 4) || grid_tree(copy, 4));
    if !(measured || plain || rows) {
        return (Watched::new(&[]), None);
    }
    let blocks = BlockBox::collect(copy, ShapeCx::COLUMNS, opts.cx.vcx());
    let breaks_w = (!measured).then(|| line_col_w.map_or(BreaksAt::Column, BreaksAt::Width));
    // A measured box without block children keeps its measured lines alone
    // (css-break-3 §4.3 class B, orphans/widows by `Paragraph`); a box whose
    // estimate is replaced keeps its own border and padding edges too.
    // A box of another context without recorded parts keeps its own
    // estimate; its line boxes are not lines of the stack flow
    // (`flow::flow_lines`).
    if blocks.is_empty() && !crate::layout::fragment::flow::flow_lines(copy) {
        return (Watched::new(&[]), None);
    }
    if blocks.kids.is_empty() && measured {
        return (Watched::for_lines(), breaks_w);
    }
    (Watched::for_blocks(blocks), breaks_w)
}

/// Строки копии срезаются `text-box-trim` у разрывов колонок (css-inline-3
/// §4.2; csswg-drafts#5335): срез многоколоночника или коробки копии.
/// Измеренные строки абзаца несут свои края без среза у разрыва, и точки
/// такой копии остаются у оценки строк (`line_run_shape`).
pub(super) fn trims_at_breaks(copy: &Element, merged: &crate::style::computed::Computed) -> bool {
    fn trims(e: &Element, depth: u8) -> bool {
        e.style.text_box_trim_start
            || e.style.text_box_trim_end
            || e.marks.host_trim_start
            || e.marks.host_trim_end
            || (depth > 0
                && e.children.iter().any(|n| match n {
                    crate::dom::Node::Element(k) => trims(k, depth - 1),
                    _ => false,
                }))
    }
    merged.text_box_trim_start || merged.text_box_trim_end || trims(copy, 5)
}

/// Таблица-корень копии, чьи ряды даёт раскладка (`breaks_w` или мера
/// раскладкой): может ли по стилю повторяться её шапка и подвал
/// (`break-inside: avoid*` секции, css-tables-3 §repeated-headers).
pub(super) fn repeatable(
    watched: &Watched,
    copy: &Element,
    breaks_w: Option<BreaksAt>,
    measured_kids: &RefCell<Vec<(u64, f32)>>,
) -> Option<(bool, bool)> {
    let laid_out = breaks_w.is_some()
        || measured_kids
            .borrow()
            .iter()
            .any(|&(id, _)| id == copy.node_id);
    let t = watched.blocks().filter(|_| laid_out)?.table.as_ref()?;
    Some((t.head.is_some_and(|s| s.2), t.foot.is_some_and(|s| s.2)))
}
