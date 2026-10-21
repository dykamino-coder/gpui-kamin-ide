//! Вычисленные математические выражения хранят ветви, зависящие от процентов, до геометрии.

use super::Sum;
use super::calc::{Calc, Val};
use super::calc_function::{Arguments, Function};
use crate::document::state::ValueCx;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LengthExpression(Node);

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Node {
    Value(Val),
    Binary(char, Box<Node>, Box<Node>),
    Call(Function, Vec<Node>),
}

impl LengthExpression {
    pub(crate) fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let mut calc = Calc::new(raw);
        calc.recording = Some(Vec::new());
        if !matches!(calc.expr(vcx)?, Val::Len(_)) || !calc.rest.trim().is_empty() {
            return None;
        }
        let mut nodes = calc.recording?;
        let node = nodes.pop()?;
        nodes.is_empty().then_some(Self(node))
    }

    pub(crate) fn evaluate(&self, basis: f32) -> Option<f32> {
        let Val::Len(sum) = self.0.evaluate(basis)? else {
            return None;
        };
        let (_, px) = sum.censored().length_percentage()?;
        px.is_finite().then_some(px)
    }

    pub(crate) fn offset_from_end(self) -> Self {
        Self(Node::Binary(
            '-',
            Box::new(Node::Value(Val::Len(Sum::of_nature(1, 1.0)))),
            Box::new(self.0),
        ))
    }
}

impl Node {
    fn evaluate(&self, basis: f32) -> Option<Val> {
        match self {
            Self::Value(value) => value.with_basis(basis),
            Self::Binary(op, left, right) => {
                left.evaluate(basis)?.binary(*op, right.evaluate(basis)?)
            }
            Self::Call(function, nodes) => {
                let mut arguments = Arguments::new(*function);
                for node in nodes {
                    arguments.push(node.evaluate(basis)?)?;
                }
                arguments.finish()
            }
        }
    }
}

impl Calc<'_> {
    pub(super) fn record_value(&mut self, value: Val) -> Option<Val> {
        if let Some(nodes) = &mut self.recording {
            nodes.push(Node::Value(value));
            value.with_basis(1.0)
        } else {
            Some(value)
        }
    }

    pub(super) fn record_binary(&mut self, op: char) -> Option<()> {
        if let Some(nodes) = &mut self.recording {
            let right = nodes.pop()?;
            let left = nodes.pop()?;
            nodes.push(Node::Binary(op, Box::new(left), Box::new(right)));
        }
        Some(())
    }

    pub(super) fn record_call(&mut self, function: Function, count: usize) -> Option<()> {
        if let Some(nodes) = &mut self.recording {
            let start = nodes.len().checked_sub(count)?;
            let arguments = nodes.drain(start..).collect();
            nodes.push(Node::Call(function, arguments));
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_use_concrete_percentage_lengths() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let expression = LengthExpression::parse("min(0%, 100%)", vcx).unwrap();
        assert_eq!(expression.evaluate(50.0), Some(0.0));
        assert_eq!(expression.evaluate(-50.0), Some(-50.0));
        let expression =
            LengthExpression::parse("calc(clamp(10px, 75%, 40px) + hypot(3px, 4px))", vcx).unwrap();
        assert_eq!(expression.evaluate(100.0), Some(45.0));
        assert_eq!(expression.evaluate(-100.0), Some(15.0));
    }

    #[test]
    fn nested_calls_and_edge_offsets_preserve_the_expression_tree() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let expression = LengthExpression::parse(
            "calc(2 * min(50%, 20px) + round(up, max(10%, 3px), 2px))",
            vcx,
        )
        .unwrap()
        .offset_from_end();
        assert_eq!(expression.evaluate(100.0), Some(50.0));
        assert_eq!(expression.evaluate(-100.0), Some(-4.0));
        assert!(LengthExpression::parse("min(100%, 1)", vcx).is_none());
        assert!(LengthExpression::parse("min(100%, 1vw)", vcx).is_none());
        assert!(LengthExpression::parse("clamp(0px, 10%)", vcx).is_none());
    }
}
