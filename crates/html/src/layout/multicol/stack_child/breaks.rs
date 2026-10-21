//! Рекордер главной копии ребёнка стопки колонок: строки — всегда, блочная стопка
//! (`flow::blocks`) — у ребёнка, чьи точки разрыва даёт раскладка копии. Это ребёнок,
//! чью высоту меряет раскладка (`StackChild::measure`), и поддерево обычных блоков
//! (`plain_block_tree`): его границы классa A, поля на разрыве, запреты и монолиты
//! берутся из разложенных border box, а не из оценки формы.

use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::blocks::BlockBox;
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::probe::plain_block_tree;
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
) -> (Rc<Watched>, Option<f32>) {
    if col_vert {
        return (Watched::new(&[]), None);
    }
    let measured = measured_kids
        .borrow()
        .iter()
        .any(|&(id, _)| id == copy.node_id);
    let plain = !own_plan && plain_block_tree(copy, 4);
    if !(measured || (plain && line_col_w.is_some())) {
        return (Watched::new(&[]), None);
    }
    let blocks = BlockBox::collect(copy, ShapeCx::COLUMNS, opts.cx.vcx());
    // Without block children the copy keeps its measured lines alone.
    if blocks.kids.is_empty() {
        return (Watched::new(&[]), None);
    }
    (
        Watched::for_blocks(blocks),
        line_col_w.filter(|_| !measured),
    )
}
