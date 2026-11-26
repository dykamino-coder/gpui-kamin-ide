//! Отложенные математические функции (css-values-4 §10.10): `min()`/`max()`/
//! `clamp()` и прочие функции над природами, базы которых ещё не известны
//! (`clamp(2rem, 1rem + 5vw, 4rem)`, `min(3em, 8vw)`).
//!
//! Линейная сумма (`Sum`) такую запись не держит: сравнить `5vw` с `2rem`
//! можно только при известных окне и кегле. Поэтому разбор записывает дерево
//! выражения, листья которого — суммы любых природ, и кладёт его в арену
//! документа (`Len::Math`, ручка как у `Len::Calc`). Каскад подставляет базы в
//! листья в тех же шагах, что и для `Len::Calc` (шрифт — `resolve_em`, строка —
//! `resolve_line_height`, окно и контейнер — `fold_viewport`/`fold_container`),
//! и упрощает дерево: узел, все доводы которого стали одной природы,
//! вычисляется, а дерево из одного листа становится обычной длиной. Так и
//! в Blink (`CSSMathExpressionNode` → `CalculationExpressionNode`) и Gecko
//! (`GenericCalcNode`): к вычисленному значению остаются лишь доли.
//!
//! Доли внутри дерева здесь пока не принимаются: их база известна только
//! раскладке, и такая запись по-прежнему отбрасывается.

use super::calc::{Calc, Val};
use super::calc_expression::Node;
use super::calc_function::Arguments;
use super::{Len, Sum};
use crate::values::ValueCx;
use std::hash::{Hash, Hasher};

/// Ручка дерева математической функции (`Len::Math`) в арене документа.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MathId(crate::values::ArenaKey);

/// Тело `Len::Math`: дерево выражения с листьями-суммами.
#[derive(Clone, Debug, PartialEq)]
pub struct MathTree(Node);

/// Разбор записи, которую `eval_calc` не свернул: дерево с проверкой типов.
/// `None` — запись негодна или держит долю (её база — только у раскладки).
pub(super) fn deferred(raw: &str, vcx: ValueCx<'_>) -> Option<Len> {
    let mut calc = Calc::new(raw);
    calc.recording = Some(Vec::new());
    calc.typing = true;
    if !matches!(calc.expr(vcx)?, Val::Len(_)) || !calc.rest.trim().is_empty() {
        return None;
    }
    let mut nodes = calc.recording?;
    let node = nodes.pop()?;
    if !nodes.is_empty() || node.any_leaf(&|s| s.has_percentage || s.pct != 0.0) {
        return None;
    }
    Some(finish(node.simplified(), vcx))
}

fn finish(node: Node, vcx: ValueCx<'_>) -> Len {
    match node {
        // Верхний уровень: NaN -> 0, ±∞ -> предел (§10.13).
        Node::Value(Val::Len(sum)) => sum.censored().collapse_mixed(vcx).unwrap_or(Len::Px(0.0)),
        tree => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            tree.hash_into(&mut h);
            let tree = MathTree(tree);
            Len::Math(MathId(crate::values::store_once(
                vcx,
                |d| &d.math,
                tree,
                h.finish(),
            )))
        }
    }
}

fn load(id: MathId, vcx: ValueCx<'_>) -> Option<Node> {
    crate::values::load(vcx, |d| &d.math, id.0).map(|t| t.0)
}

impl Node {
    /// Упрощение §10.10: линейные действия над листьями и функции, доводы
    /// которых сравнимы (одна природа), вычисляются; остальное остаётся узлом.
    pub(super) fn simplified(self) -> Node {
        match self {
            Node::Value(v) => Node::Value(v),
            Node::Binary(op, l, r) => {
                let (l, r) = (l.simplified(), r.simplified());
                if let (Node::Value(a), Node::Value(b)) = (&l, &r)
                    && let Some(v) = a.binary(op, *b)
                {
                    return Node::Value(v);
                }
                Node::Binary(op, Box::new(l), Box::new(r))
            }
            Node::Call(function, args) => {
                let args: Vec<Node> = args.into_iter().map(Node::simplified).collect();
                let mut arguments = Arguments::new(function);
                let folded = args.iter().try_for_each(|a| match a {
                    Node::Value(v) => arguments.push(*v),
                    _ => None,
                });
                if let Some(v) = folded.and_then(|()| arguments.finish()) {
                    return Node::Value(v);
                }
                Node::Call(function, args)
            }
        }
    }

    fn map_leaves(&mut self, f: &mut impl FnMut(&mut Sum) -> bool) -> bool {
        match self {
            Node::Value(Val::Len(s)) => f(s),
            Node::Value(_) => false,
            Node::Binary(_, l, r) => l.map_leaves(f) | r.map_leaves(f),
            Node::Call(_, args) => args.iter_mut().fold(false, |any, a| a.map_leaves(f) | any),
        }
    }

    fn any_leaf(&self, f: &impl Fn(&Sum) -> bool) -> bool {
        match self {
            Node::Value(Val::Len(s)) => f(s),
            Node::Value(_) => false,
            Node::Binary(_, l, r) => l.any_leaf(f) || r.any_leaf(f),
            Node::Call(_, args) => args.iter().any(|a| a.any_leaf(f)),
        }
    }

    fn hash_into(&self, h: &mut impl Hasher) {
        match self {
            Node::Value(v) => {
                0u8.hash(h);
                match v {
                    Val::Num(n) => (0u8, n.to_bits()).hash(h),
                    Val::Ang(a) => (1u8, a.to_bits()).hash(h),
                    Val::Len(s) => (2u8, super::calc_pool::sum_hash(s)).hash(h),
                }
            }
            Node::Binary(op, l, r) => {
                (1u8, *op).hash(h);
                l.hash_into(h);
                r.hash_into(h);
            }
            Node::Call(function, args) => {
                (2u8, function, args.len()).hash(h);
                args.iter().for_each(|a| a.hash_into(h));
            }
        }
    }
}

impl Len {
    /// Шаг каскада над деревом: `f` подставляет известные базы в лист и
    /// отвечает, менял ли его. Дерево упрощается и, если свелось к одному
    /// листу, становится длиной. Без перемен ручка остаётся прежней: арена
    /// append-only, а шаги идут на каждом наследовании.
    pub fn resolve_math(self, vcx: ValueCx<'_>, mut f: impl FnMut(&mut Sum) -> bool) -> Len {
        let Len::Math(id) = self else {
            return self;
        };
        let Some(mut node) = load(id, vcx) else {
            return self;
        };
        if !node.map_leaves(&mut f) {
            return self;
        }
        finish(node.simplified(), vcx)
    }

    /// Сумма, к которой дерево сводится после подстановки баз `f`, — без
    /// записи в арену (для потребителей на раскладке и отрисовке).
    pub fn math_sum(self, vcx: ValueCx<'_>, mut f: impl FnMut(&mut Sum) -> bool) -> Option<Sum> {
        let Len::Math(id) = self else {
            return None;
        };
        let mut node = load(id, vcx)?;
        node.map_leaves(&mut f);
        match node.simplified() {
            Node::Value(Val::Len(sum)) => Some(sum.censored()),
            _ => None,
        }
    }

    /// Есть ли в дереве лист, для которого `f` истинно.
    pub fn math_any(self, vcx: ValueCx<'_>, f: impl Fn(&Sum) -> bool) -> bool {
        matches!(self, Len::Math(id) if load(id, vcx).is_some_and(|n| n.any_leaf(&f)))
    }
}

impl Sum {
    /// Шрифтовые слагаемые в точки по кеглю `em` и метрикам глифов.
    /// `false` — шрифтовых слагаемых не было.
    pub fn fold_font(&mut self, em: f32, glyphs: impl FnOnce() -> [f32; 4]) -> bool {
        if self.em == 0.0 && self.ch == 0.0 && self.ex == 0.0 && self.ic == 0.0 && self.cap == 0.0 {
            return false;
        }
        let [cap, ch, ex, ic] =
            if self.ch != 0.0 || self.ex != 0.0 || self.ic != 0.0 || self.cap != 0.0 {
                glyphs()
            } else {
                [0.0; 4]
            };
        self.px += self.em * em + self.ch * ch + self.ex * ex + self.ic * ic + self.cap * cap;
        self.em = 0.0;
        self.ch = 0.0;
        self.ex = 0.0;
        self.ic = 0.0;
        self.cap = 0.0;
        true
    }

    /// Слагаемое `lh` в точки по высоте строки; `false` — его не было.
    pub fn fold_line(&mut self, line: f32) -> bool {
        if self.lh == 0.0 {
            return false;
        }
        self.px += self.lh * line;
        self.lh = 0.0;
        true
    }
}

#[cfg(test)]
#[path = "calc_tree_tests.rs"]
mod tests;
