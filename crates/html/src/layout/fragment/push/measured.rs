//! Распорки роста по РАЗЛОЖЕННОЙ копии ребёнка стопки (css-break-3 §box-splitting: у
//! непоследнего фрагмента коробка дотягивается до конца фрагментаинера; Blink
//! `FinishFragmentation`). Точку роста даёт план укладки по измеренным точкам разрыва
//! (`ColumnStack::grow_points`), коробку, перед которой встаёт распорка, — разложенные
//! border box блочной стопки (`BlockBox::pushed_at`), ряд сетки — её разложенные дорожки
//! (перенос ряда, `grid_row::push_row`), а новую высоту и точки разрыва — повторная
//! раскладка копии, собранной заново из правленого дерева (`Regrow::build`).
//! Оценка формы (`pushed_box_at`, `shape_full`) здесь не участвует.

use super::cell::grow_before;
use super::spacer::spacer_before;
use crate::dom::Element;
use crate::layout::fragment::clone::clone_dec;
use crate::layout::fragment::flow::blocks::{
    BlockBox, Laid, Pushed, flex_tree, grid_tree, stacked_tree, table_tree,
};
use crate::layout::fragment::probe::plain_block_tree;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;

/// Сборка копии ребёнка стопки из дерева: `part` 0 — главная (с рекордером),
/// дальше — запасные.
pub(crate) type BuildCopy = dyn Fn(&Element, usize) -> AnyElement;

/// Ребёнок стопки, чьи распорки роста ставит раскладка его копии.
#[derive(Clone)]
pub(crate) struct Regrow {
    pub(crate) axis: crate::layout::fragment::types::StackAxis,
    pub(crate) source: Element,
    /// Дерево, из которого строятся копии; распорки правят его.
    pub(crate) dom: Element,
    pub(crate) build: std::rc::Rc<BuildCopy>,
    pub(crate) opts: RenderOpts,
    /// Разложенная геометрия стопки по последней раскладке главной копии.
    pub(crate) laid: Laid,
    /// Высота ребёнка в стопке — разложенный border box (а не мера формы):
    /// ложно, когда мера несёт дотяг внепоточных или коробка из одних флоатов
    /// обнулена (`multicol_stack_child`).
    pub(crate) laid_h: bool,
    pub(crate) watched: std::rc::Rc<crate::layout::fragment::paginate::watch::Watched>,
}

impl Regrow {
    /// Несколько потоков могут запросить один и тот же рост общей строки.
    /// Ограничение применяется один раз с наибольшим занятым местом.
    pub(crate) fn push_many(&mut self, mut grows: Vec<(BlockBox, f32, f32, bool)>) -> bool {
        grows.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut unique: Vec<(BlockBox, f32, f32, bool)> = Vec::new();
        for (scope, at, growth, forced) in grows {
            let Some(target) = scope.pushed_at(at, &self.laid) else {
                continue;
            };
            if let Some(previous) = unique
                .iter_mut()
                .find(|p| p.0.pushed_at(p.1, &self.laid).as_ref() == Some(&target))
            {
                previous.2 = previous.2.max(growth);
                previous.3 |= forced;
            } else {
                unique.push((scope, at, growth, forced));
            }
        }
        let mut changed = false;
        for (scope, at, growth, forced) in unique {
            changed |= self.push(&scope, at, growth, forced);
        }
        changed
    }

    /// Распорка `grow` перед самой внешней коробкой стопки `blocks`,
    /// начинающейся в `at`: принудительному разрыву — коробка-распорка
    /// (`spacer_before`), монолиту — поле (`grow_before`), как у оценки.
    pub(crate) fn push(&mut self, blocks: &BlockBox, at: f32, grow: f32, forced: bool) -> bool {
        let pushed = blocks.pushed_at(at, &self.laid);
        if let Some(Pushed::Box(id)) = pushed
            && self
                .laid
                .flexes
                .values()
                .any(|f| f.column && f.lines.iter().any(|l| l.items.contains(&id)))
        {
            return self.watched.grow_flex_item(id, grow);
        }
        if self.axis.is_vertical()
            && let Some(Pushed::Box(id)) = pushed
        {
            return super::vertical::push(
                &mut self.dom,
                id,
                grow,
                forced,
                self.axis,
                self.opts.cx.vcx(),
            );
        }
        match pushed {
            Some(Pushed::Box(id)) if forced => {
                spacer_before(&mut self.dom, id, grow, self.opts.cx.vcx())
            }
            Some(Pushed::Box(id)) => grow_before(&mut self.dom, id, grow),
            Some(Pushed::Row(row)) => {
                super::grid_row::push_row(&mut self.dom, &row, grow, self.axis)
            }
            Some(Pushed::Flex(line)) => {
                self.watched.grow_flex_line(line.container, line.line, grow)
            }
            Some(Pushed::Line(line)) => line.grow(grow),
            None => false,
        }
    }

    /// Главная и `spare` запасных копий из правленого дерева.
    pub(crate) fn rebuild(&self, spare: usize) -> (AnyElement, Vec<AnyElement>) {
        self.watched.rewind_flows();
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
/// `table_tree`): ряд растёт вместе с ячейкой, чья коробка ушла распоркой; у
/// поддерева с сеткой (`grid_tree`) — и ряды разложенной сетки.
pub(crate) fn measured_push(c: &Element, line_col_w: Option<f32>, col_vert: bool) -> bool {
    line_col_w.is_some()
        && if col_vert {
            crate::layout::fragment::flow::vertical_copy(c, false)
                || crate::layout::fragment::flow::vertical_copy(c, true)
        } else {
            measured_tree(c)
        }
}

/// Поддерево, чью геометрию полностью записывает раскладка копии.
pub(crate) fn measured_tree(c: &Element) -> bool {
    !c.marks.paginate
        && c.style.column_span != Some(true)
        && clone_dec(c).is_none()
        && ((plain_block_tree(c, 4) && stacked_tree(c, 4))
            || table_tree(c, 4)
            || grid_tree(c, 4)
            || flex_tree(c, 4))
}
