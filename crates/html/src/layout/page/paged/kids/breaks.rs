//! Рекордер главной копии ребёнка стопки страниц: строки — всегда, блочная стопка
//! (`flow::blocks`) — у группы из одной коробки с поддеревом обычных блоков
//! (`plain_block_tree`) или таблицы из них (`table_tree`), измеренной мерой формы: её точки
//! разрыва даёт раскладка копии.
//! Коробку без меры формы страницы разбивают распорками по раскладке копии
//! (`fragment::push`); точки поверх них легли бы на уже сдвинутое содержимое.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::dom::whitespace::is_blank;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::blocks::{BlockBox, table_tree};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::probe::plain_block_tree;
use crate::style::computed::Position;
use std::rc::Rc;

pub(super) fn recorder(
    group: &[Node],
    n: &Node,
    vertical: Option<bool>,
    shaped: bool,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Rc<Watched> {
    let single = group.iter().filter(|g| !is_blank(g)).count() == 1;
    match n {
        Node::Element(e)
            if single
                && vertical.is_none()
                && !e.inline
                && !matches!(
                    e.style.cascaded().position,
                    Some(Position::Absolute) | Some(Position::Fixed)
                )
                && shaped
                && (plain_block_tree(e, 4) || table_tree(e, 4)) =>
        {
            let blocks = BlockBox::collect(e, cx, vcx);
            // Without block children the copy keeps its measured lines alone.
            if blocks.is_empty() {
                Watched::new(&[])
            } else {
                Watched::for_blocks(blocks)
            }
        }
        _ => Watched::new(&[]),
    }
}
