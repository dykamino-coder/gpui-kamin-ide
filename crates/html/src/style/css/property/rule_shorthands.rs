//! Сокращения линеек промежутков, раскрытые при разборе (css-gaps-1):
//! `column-rule`/`row-rule`/`rule` (§rule-shorthands: список `<gap-rule>`
//! раскладывается по спискам ширин, стилей и цветов, `repeat()` сохраняется
//! со своими телами — Blink `ParseGapDecorationsShorthand`), `rule-width`/
//! `-style`/`-color`/`-break`/`-visibility-items` (обе оси одним значением) и
//! семья втяжек `*-rule-inset*` (§inset).

use super::{LonghandId, ShorthandId};
use crate::style::computed::split_outside_parens;
use LonghandId::*;
use gap_rule::gap_rules;

pub(super) const COLUMN_RULE: &[LonghandId] = &[ColumnRuleWidth, ColumnRuleStyle, ColumnRuleColor];
pub(super) const ROW_RULE: &[LonghandId] = &[RowRuleWidth, RowRuleStyle, RowRuleColor];
pub(super) const RULE: &[LonghandId] = &[
    ColumnRuleWidth,
    ColumnRuleStyle,
    ColumnRuleColor,
    RowRuleWidth,
    RowRuleStyle,
    RowRuleColor,
];
/// Какие грани втяжки задаёт сокращение семьи `*-rule-inset*`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Inset {
    /// `cap-start cap-end? [/ junction-start junction-end?]?`.
    Full,
    /// `<inset-value> <inset-value>?` для концов.
    Cap,
    /// То же для стыков.
    Junction,
    /// Начала концов и стыков одним значением.
    Start,
    /// Окончания концов и стыков одним значением.
    End,
}

const COLUMN_RULE_INSET: &[LonghandId] = &[
    ColumnRuleInsetCapStart,
    ColumnRuleInsetCapEnd,
    ColumnRuleInsetJunctionStart,
    ColumnRuleInsetJunctionEnd,
];
const ROW_RULE_INSET: &[LonghandId] = &[
    RowRuleInsetCapStart,
    RowRuleInsetCapEnd,
    RowRuleInsetJunctionStart,
    RowRuleInsetJunctionEnd,
];
const RULE_INSET: &[LonghandId] = &[
    ColumnRuleInsetCapStart,
    ColumnRuleInsetCapEnd,
    ColumnRuleInsetJunctionStart,
    ColumnRuleInsetJunctionEnd,
    RowRuleInsetCapStart,
    RowRuleInsetCapEnd,
    RowRuleInsetJunctionStart,
    RowRuleInsetJunctionEnd,
];
const COLUMN_RULE_INSET_CAP: &[LonghandId] = &[ColumnRuleInsetCapStart, ColumnRuleInsetCapEnd];
const ROW_RULE_INSET_CAP: &[LonghandId] = &[RowRuleInsetCapStart, RowRuleInsetCapEnd];
const RULE_INSET_CAP: &[LonghandId] = &[
    ColumnRuleInsetCapStart,
    ColumnRuleInsetCapEnd,
    RowRuleInsetCapStart,
    RowRuleInsetCapEnd,
];
const COLUMN_RULE_INSET_JUNCTION: &[LonghandId] =
    &[ColumnRuleInsetJunctionStart, ColumnRuleInsetJunctionEnd];
const ROW_RULE_INSET_JUNCTION: &[LonghandId] =
    &[RowRuleInsetJunctionStart, RowRuleInsetJunctionEnd];
const RULE_INSET_JUNCTION: &[LonghandId] = &[
    ColumnRuleInsetJunctionStart,
    ColumnRuleInsetJunctionEnd,
    RowRuleInsetJunctionStart,
    RowRuleInsetJunctionEnd,
];
const COLUMN_RULE_INSET_START: &[LonghandId] =
    &[ColumnRuleInsetCapStart, ColumnRuleInsetJunctionStart];
const ROW_RULE_INSET_START: &[LonghandId] = &[RowRuleInsetCapStart, RowRuleInsetJunctionStart];
const RULE_INSET_START: &[LonghandId] = &[
    ColumnRuleInsetCapStart,
    ColumnRuleInsetJunctionStart,
    RowRuleInsetCapStart,
    RowRuleInsetJunctionStart,
];
const COLUMN_RULE_INSET_END: &[LonghandId] = &[ColumnRuleInsetCapEnd, ColumnRuleInsetJunctionEnd];
const ROW_RULE_INSET_END: &[LonghandId] = &[RowRuleInsetCapEnd, RowRuleInsetJunctionEnd];
const RULE_INSET_END: &[LonghandId] = &[
    ColumnRuleInsetCapEnd,
    ColumnRuleInsetJunctionEnd,
    RowRuleInsetCapEnd,
    RowRuleInsetJunctionEnd,
];

impl ShorthandId {
    pub(super) fn rule_longhands(self) -> Option<&'static [LonghandId]> {
        Some(match self {
            Self::ColumnRule => COLUMN_RULE,
            Self::RowRule => ROW_RULE,
            Self::Rule => RULE,
            Self::RuleWidth => &[ColumnRuleWidth, RowRuleWidth],
            Self::RuleStyle => &[ColumnRuleStyle, RowRuleStyle],
            Self::RuleColor => &[ColumnRuleColor, RowRuleColor],
            Self::RuleBreak => &[ColumnRuleBreak, RowRuleBreak],
            Self::RuleVisibilityItems => &[ColumnRuleVisibilityItems, RowRuleVisibilityItems],
            Self::ColumnRuleInset => COLUMN_RULE_INSET,
            Self::RowRuleInset => ROW_RULE_INSET,
            Self::RuleInset => RULE_INSET,
            Self::ColumnRuleInsetCap => COLUMN_RULE_INSET_CAP,
            Self::RowRuleInsetCap => ROW_RULE_INSET_CAP,
            Self::RuleInsetCap => RULE_INSET_CAP,
            Self::ColumnRuleInsetJunction => COLUMN_RULE_INSET_JUNCTION,
            Self::RowRuleInsetJunction => ROW_RULE_INSET_JUNCTION,
            Self::RuleInsetJunction => RULE_INSET_JUNCTION,
            Self::ColumnRuleInsetStart => COLUMN_RULE_INSET_START,
            Self::RowRuleInsetStart => ROW_RULE_INSET_START,
            Self::RuleInsetStart => RULE_INSET_START,
            Self::ColumnRuleInsetEnd => COLUMN_RULE_INSET_END,
            Self::RowRuleInsetEnd => ROW_RULE_INSET_END,
            Self::RuleInsetEnd => RULE_INSET_END,
            _ => return None,
        })
    }

    fn inset_kind(self) -> Option<Inset> {
        Some(match self {
            Self::ColumnRuleInset | Self::RowRuleInset | Self::RuleInset => Inset::Full,
            Self::ColumnRuleInsetCap | Self::RowRuleInsetCap | Self::RuleInsetCap => Inset::Cap,
            Self::ColumnRuleInsetJunction
            | Self::RowRuleInsetJunction
            | Self::RuleInsetJunction => Inset::Junction,
            Self::ColumnRuleInsetStart | Self::RowRuleInsetStart | Self::RuleInsetStart => {
                Inset::Start
            }
            Self::ColumnRuleInsetEnd | Self::RowRuleInsetEnd | Self::RuleInsetEnd => Inset::End,
            _ => return None,
        })
    }

    pub(super) fn rule_parts(self, value: &str) -> Option<Vec<String>> {
        let v = value.trim();
        let n = self.longhands().len();
        if let Some(kind) = self.inset_kind() {
            let one = inset(kind, v)?;
            return Some(one.iter().cycle().take(n).cloned().collect());
        }
        let one = match self {
            Self::ColumnRule | Self::RowRule | Self::Rule => gap_rules(v)?,
            _ if v.is_empty() => return None,
            _ => vec![v.to_string()],
        };
        Some(one.iter().cycle().take(n).cloned().collect())
    }
}

/// Значения граней одной оси по виду сокращения втяжек (§inset): опущенные
/// стыки — копия концов, опущенное окончание — копия начала.
fn inset(kind: Inset, v: &str) -> Option<Vec<String>> {
    let pair = |t: &str| -> Option<[String; 2]> {
        match split_outside_parens(t.trim()).as_slice() {
            [a] => Some([a.clone(), a.clone()]),
            [a, b] => Some([a.clone(), b.clone()]),
            _ => None,
        }
    };
    let single = |t: &str| -> Option<String> {
        match split_outside_parens(t.trim()).as_slice() {
            [a] => Some(a.clone()),
            _ => None,
        }
    };
    Some(match kind {
        Inset::Full => {
            let (cap, junction) = match v.split_once('/') {
                Some((c, j)) => (pair(c)?, pair(j)?),
                None => {
                    let c = pair(v)?;
                    (c.clone(), c)
                }
            };
            vec![
                cap[0].clone(),
                cap[1].clone(),
                junction[0].clone(),
                junction[1].clone(),
            ]
        }
        Inset::Cap | Inset::Junction => pair(v)?.to_vec(),
        Inset::Start | Inset::End => {
            let a = single(v)?;
            vec![a.clone(), a]
        }
    })
}

mod gap_rule;

#[cfg(test)]
mod tests;
