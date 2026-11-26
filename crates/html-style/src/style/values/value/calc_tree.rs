//! Deferred math functions (css-values-4 §10.10): `min()`/`max()`/`clamp()`
//! and the other functions over units whose bases are not known yet
//! (`clamp(2rem, 1rem + 5vw, 4rem)`, `min(3em, 8vw)`).
//!
//! A linear sum (`Sum`) cannot hold such a value: `5vw` compares with `2rem`
//! only once the viewport and the font size are known. The parser records
//! an expression tree whose leaves are sums of any units and stores it in
//! the document arena (`Len::Math`, a handle like `Len::Calc`). The cascade
//! substitutes bases in the same steps as for `Len::Calc` (font: `resolve_em`,
//! line: `resolve_line_height`, viewport and container: `fold_viewport`/`fold_container`)
//! and simplifies the tree: a node whose arguments now share one unit is
//! evaluated, and a tree of one leaf becomes an ordinary length. Blink
//! (`CSSMathExpressionNode` -> `CalculationExpressionNode`) and Gecko
//! (`GenericCalcNode`) do the same: only percentages survive the computed value.
//!
//! Percentages inside a tree are not accepted yet: only layout knows their
//! basis, so such a value is still dropped.

use super::calc::{Calc, Val};
use super::calc_expression::Node;
use super::calc_function::Arguments;
use super::{Len, Sum};
use crate::values::ValueCx;
use std::hash::{Hash, Hasher};

/// Handle of a math function tree (`Len::Math`) in the document arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MathId(crate::values::ArenaKey);

/// Body of `Len::Math`: an expression tree whose leaves are sums.
#[derive(Clone, Debug, PartialEq)]
pub struct MathTree(Node);

/// Parses a value `eval_calc` could not fold, as a type-checked tree.
/// `None`: the value is invalid or holds a percentage (only layout has its basis).
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
        // Top level: NaN -> 0, ±infinity -> the limit (§10.13).
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
    /// Simplification (§10.10): linear operations on leaves and functions
    /// whose arguments are comparable (one unit) are evaluated; the rest stays a node.
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
    /// A cascade step over the tree: `f` substitutes known bases into a leaf
    /// and reports whether it changed it. The tree is simplified and becomes
    /// a length when one leaf remains. Without changes the handle is kept: the
    /// arena is append-only and the steps run on every inheritance.
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

    /// The sum the tree reduces to after `f` substitutes bases, without an
    /// arena write (for layout and paint consumers).
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

    /// Whether a leaf of the tree satisfies `f`.
    pub fn math_any(self, vcx: ValueCx<'_>, f: impl Fn(&Sum) -> bool) -> bool {
        matches!(self, Len::Math(id) if load(id, vcx).is_some_and(|n| n.any_leaf(&f)))
    }
}

impl Sum {
    /// Folds font-relative terms into pixels by the `em` size and glyph metrics.
    /// `false`: there were no font-relative terms.
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

    /// Folds the `lh` term by the line height; `false`: there was none.
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
