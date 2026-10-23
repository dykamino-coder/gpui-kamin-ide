//! Распорки роста по РАЗЛОЖЕННОЙ копии ребёнка стопки (css-break-3 §box-splitting: у
//! непоследнего фрагмента коробка дотягивается до конца фрагментаинера; Blink
//! `FinishFragmentation`). Точку роста даёт план укладки по измеренным точкам разрыва
//! (`ColumnStack::grow_points`), коробку, перед которой встаёт распорка, — разложенные
//! border box блочной стопки (`BlockBox::pushed_at`), ряд сетки — её разложенные дорожки
//! (перенос ряда, `grid_row::push_row`), а новую высоту и точки разрыва — повторная
//! раскладка копии, собранной заново из правленого дерева (`Regrow::build`).
//! Оценка формы (`pushed_box_at`, `shape_full`) здесь не участвует.

use super::cell::grow_before;
use super::spacer::spacer_before_laid;
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

/// Получатель роста сохраняет идентичность даже у совпадающих границ строк.
#[derive(Clone, Debug)]
pub(crate) struct Growth {
    pub(crate) target: Pushed,
    pub(crate) at: f32,
    pub(crate) amount: f32,
    pub(crate) forced: bool,
}

impl Growth {
    pub(crate) fn at(
        scope: &BlockBox,
        laid: &Laid,
        at: f32,
        amount: f32,
        forced: bool,
    ) -> Option<Self> {
        Some(Self {
            target: scope.pushed_at(at, laid)?,
            at,
            amount,
            forced,
        })
    }
}

/// Ребёнок стопки, чьи распорки роста ставит раскладка его копии.
#[derive(Clone)]
pub(crate) struct Regrow {
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
    pub(crate) fn push_many(&mut self, mut grows: Vec<Growth>) -> bool {
        if !grows.is_empty() {
            self.watched.pin_flex_lines(&self.laid);
        }
        grows.sort_by(|a, b| b.at.total_cmp(&a.at));
        let mut unique: Vec<Growth> = Vec::new();
        for growth in grows {
            if let Some(previous) = unique.iter_mut().find(|p| p.target == growth.target) {
                previous.amount = previous.amount.max(growth.amount);
                previous.forced |= growth.forced;
            } else {
                unique.push(growth);
            }
        }
        let mut changed = false;
        for growth in unique {
            changed |= self.push(growth.target, growth.amount, growth.forced);
        }
        changed
    }

    /// Идентичность цели берётся до правок дерева: координаты могут совпадать.
    fn push(&mut self, pushed: Pushed, grow: f32, forced: bool) -> bool {
        // Элемент flex сдвигается своим смещением (колонка — по главной оси,
        // ряд — по блочной после выравнивания), а не распоркой.
        if let Pushed::Box(id) = &pushed
            && self
                .laid
                .flexes
                .values()
                .any(|f| f.lines.iter().any(|l| l.items.contains(id)))
        {
            return self.watched.grow_flex_item(*id, grow);
        }
        match pushed {
            Pushed::Box(id) if forced => {
                spacer_before_laid(&mut self.dom, id, grow, self.opts.cx.vcx())
            }
            Pushed::Box(id) => grow_before(&mut self.dom, id, grow),
            Pushed::Row(row) => super::grid_row::push_row(&mut self.dom, &row, grow),
            Pushed::Flex(line) => self.watched.grow_flex_line(line.container, line.line, grow),
            Pushed::Line(line) => line.grow(grow),
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
    !col_vert
        && line_col_w.is_some()
        && !c.marks.paginate
        && c.style.column_span != Some(true)
        && clone_dec(c).is_none()
        && ((plain_block_tree(c, 4) && stacked_tree(c, 4))
            || table_tree(c, 4)
            || grid_tree(c, 4)
            || flex_tree(c, 4))
}
