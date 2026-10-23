//! Computed::apply_multicol_rules, хвост цепочки: visibility/overlap/break линеек, margin-trim.
//! Ветви в исходном порядке.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_multicol_rule_flags(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        _vcx: ValueCx<'_>,
    ) {
        match key {
            // §visibility-items: 0 normal, 1 all, 2 around, 3 between.
            "column-rule-visibility-items" | "row-rule-visibility-items" => {
                let code = parsed_value!(self, super::super::values::visibility_items(v));
                if key == "column-rule-visibility-items" {
                    self.column_rule_visibility = Some(code);
                } else {
                    self.row_rule_visibility = Some(code);
                }
            }
            // §overlap: порядок краски пересекающихся линеек.
            "rule-overlap" => match v.trim() {
                "row-over-column" => self.rule_column_over_row = accept_parsed!(self, Some(false)),
                "column-over-row" => self.rule_column_over_row = accept_parsed!(self, Some(true)),
                _ => {}
            },
            // §break: `none` 0, `normal` 1, `intersection` 2.
            "column-rule-break" | "row-rule-break" => {
                let code = parsed_value!(self, super::super::values::rule_break(v));
                if key == "column-rule-break" {
                    self.column_rule_break = Some(code);
                } else {
                    self.row_rule_break = Some(code);
                }
            }
            // §margin-trim: `none | block | [ block-start || block-end ]`.
            "margin-trim" => {
                let mut bits = 0u8;
                let mut ok = true;
                for token in v.split_whitespace() {
                    match token {
                        "none" => {}
                        "block" => bits |= 3,
                        "block-start" => bits |= 1,
                        "block-end" => bits |= 2,
                        // Строчные края — текущая редакция спеки: их
                        // исполняют гибкий контейнер и сетка («Flex
                        // Containers», «Grid Containers»); блочный контейнер
                        // их не видит (`block-container-inline-001`).
                        "inline" => bits |= 12,
                        "inline-start" => bits |= 4,
                        "inline-end" => bits |= 8,
                        _ => ok = false,
                    }
                }
                if ok {
                    self.accept_declaration();
                    self.margin_trim = bits;
                }
            }
            _ => *hit = false,
        }
    }
}
