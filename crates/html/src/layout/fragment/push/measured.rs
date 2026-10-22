//! Распорки роста по РАЗЛОЖЕННОЙ копии ребёнка стопки (css-break-3 §box-splitting: у
//! непоследнего фрагмента коробка дотягивается до конца фрагментаинера; Blink
//! `FinishFragmentation`). Точку роста даёт план укладки по измеренным точкам разрыва
//! (`ColumnStack::grow_points`), коробку, перед которой встаёт распорка, — разложенные
//! border box блочной стопки (`BlockBox::pushed_at`), а новую высоту и точки разрыва —
//! повторная раскладка копии, собранной заново из правленого дерева (`Regrow::build`).
//! Оценка формы (`pushed_box_at`, `shape_full`) здесь не участвует.

use super::cell::grow_before;
use super::spacer::spacer_before;
use crate::dom::Element;
use crate::layout::fragment::clone::clone_dec;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid, stacked_tree, table_tree};
use crate::layout::fragment::probe::plain_block_tree;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;

/// Сборка копии ребёнка стопки из дерева: `part` 0 — главная (с рекордером),
/// дальше — запасные.
pub(crate) type BuildCopy = dyn Fn(&Element, usize) -> AnyElement;

/// Ребёнок стопки, чьи распорки роста ставит раскладка его копии.
pub(crate) struct Regrow {
    /// Дерево, из которого строятся копии; распорки правят его.
    pub(crate) dom: Element,
    pub(crate) build: Box<BuildCopy>,
    pub(crate) opts: RenderOpts,
    /// Разложенная геометрия стопки по последней раскладке главной копии.
    pub(crate) laid: Laid,
    /// Высота ребёнка в стопке — разложенный border box (а не мера формы):
    /// ложно, когда мера несёт дотяг внепоточных или коробка из одних флоатов
    /// обнулена (`multicol_stack_child`).
    pub(crate) laid_h: bool,
}

impl Regrow {
    /// Распорка `grow` перед самой внешней коробкой стопки `blocks`,
    /// начинающейся в `at`: принудительному разрыву — коробка-распорка
    /// (`spacer_before`), монолиту — поле (`grow_before`), как у оценки.
    pub(crate) fn push(&mut self, blocks: &BlockBox, at: f32, grow: f32, forced: bool) -> bool {
        let Some(id) = blocks.pushed_at(at, &self.laid) else {
            return false;
        };
        if forced {
            spacer_before(&mut self.dom, id, grow, self.opts.cx.vcx())
        } else {
            grow_before(&mut self.dom, id, grow)
        }
    }

    /// Главная и `spare` запасных копий из правленого дерева.
    pub(crate) fn rebuild(&self, spare: usize) -> (AnyElement, Vec<AnyElement>) {
        (
            (self.build)(&self.dom, 0),
            (1..=spare).map(|i| (self.build)(&self.dom, i)).collect(),
        )
    }
}

/// Ребёнок стопки колонок, чьи распорки роста ставит раскладка копии, а не
/// оценка `grow_pushed`: поддерево обычных блоков, чьи точки разрыва даёт
/// раскладка главной копии (`StackChild::breaks_w`), без своего плана фрагментов
/// (`box-decoration-break: clone`) и не спаннер; каждую его коробку видит
/// рекордер блочной стопки (`stacked_tree`, таблица — рядами и ячейками,
/// `table_tree`): ряд растёт вместе с ячейкой, чья коробка ушла распоркой.
pub(crate) fn measured_push(c: &Element, line_col_w: Option<f32>, col_vert: bool) -> bool {
    !col_vert
        && line_col_w.is_some()
        && !c.marks.paginate
        && c.style.column_span != Some(true)
        && clone_dec(c).is_none()
        && ((plain_block_tree(c, 4) && stacked_tree(c, 4)) || table_tree(c, 4))
}
