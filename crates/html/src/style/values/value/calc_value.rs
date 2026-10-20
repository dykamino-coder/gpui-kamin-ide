//! Dimensional arithmetic shared by CSS math parsing and computed expression evaluation.

use super::Sum;
use super::calc::Val;

impl Val {
    pub(super) fn with_basis(self, basis: f32) -> Option<Self> {
        Some(match self {
            Self::Len(sum) => {
                let (pct, px) = sum.length_percentage()?;
                Self::Len(Sum::of_nature(0, pct * basis + px))
            }
            other => other,
        })
    }

    pub(super) fn binary(self, op: char, rhs: Self) -> Option<Self> {
        Some(match (op, self, rhs) {
            ('+' | '-', Self::Num(a), Self::Num(b)) => {
                Self::Num(a + if op == '-' { -b } else { b })
            }
            ('+' | '-', Self::Ang(a), Self::Ang(b)) => {
                Self::Ang(a + if op == '-' { -b } else { b })
            }
            ('+' | '-', Self::Len(a), Self::Len(b)) => {
                Self::Len(a.add(b, if op == '-' { -1.0 } else { 1.0 }))
            }
            ('*', Self::Len(a), Self::Num(k)) | ('*', Self::Num(k), Self::Len(a)) => {
                Self::Len(a.scaled(k))
            }
            ('*', Self::Ang(a), Self::Num(k)) | ('*', Self::Num(k), Self::Ang(a)) => {
                Self::Ang(a * k)
            }
            ('*', Self::Num(a), Self::Num(b)) => Self::Num(a * b),
            // CSS Values 4 §10.9: division by zero produces infinity or NaN;
            // consumers censor only the top-level result, not intermediate values.
            ('/', Self::Len(a), Self::Num(k)) => Self::Len(a.scaled(1.0 / k)),
            ('/', Self::Ang(a), Self::Num(k)) => Self::Ang(a / k),
            ('/', Self::Num(a), Self::Num(k)) => Self::Num(a / k),
            _ => return None,
        })
    }
}
