//! Shared argument reduction for parsed and computed CSS math functions.

use super::calc::Val;
use super::calc_arguments::{Comparison, Homogeneous};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Function {
    Compare(u8),
    Math(&'static str, &'static str),
}

pub(super) struct Arguments {
    function: Function,
    comparison: Option<Comparison>,
    homogeneous: Homogeneous,
    squares: f32,
    args: [Val; 2],
    count: usize,
}

impl Arguments {
    pub(super) fn new(function: Function) -> Self {
        Self {
            function,
            comparison: match function {
                Function::Compare(kind) => Some(Comparison::new(kind)),
                _ => None,
            },
            homogeneous: Homogeneous::default(),
            squares: 0.0,
            args: [Val::Num(0.0); 2],
            count: 0,
        }
    }

    pub(super) fn push(&mut self, value: Val) -> Option<()> {
        match self.function {
            Function::Compare(_) => self.comparison.as_mut()?.push(value)?,
            Function::Math("hypot", _) => {
                let key = self.homogeneous.push(value)?;
                self.squares += key * key;
            }
            Function::Math(_, _) => *self.args.get_mut(self.count)? = value,
        }
        self.count += 1;
        Some(())
    }

    pub(super) fn finish(self) -> Option<Val> {
        match self.function {
            Function::Compare(_) => self.comparison?.finish(),
            Function::Math("hypot", _) => self.homogeneous.rebuild(self.squares.sqrt()),
            Function::Math(name, strategy) => {
                super::calc_math::apply(name, strategy, &self.args[..self.count])
            }
        }
    }
}
