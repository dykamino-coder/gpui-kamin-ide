//! Computed::apply_multicol, хвост цепочки: лонгхенды линеек промежутков
//! (*-rule-width/style/color/inset/visibility/overlap/break) и margin-trim; сокращения линеек и
//! columns раскрыты разбором блока. Ветви в исходном порядке после свойств колонок и разрывов.

use super::*;
use crate::document::state::ValueCx;

mod rule_flags;

impl Computed {
    pub(super) fn apply_multicol_rules(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            // Лонгхенды линеек промежутков (css-gaps-1 §color-style-width):
            // список через запятую с `repeat()`; `rule-*` — сокращения.
            // Первое значение уходит в скаляры — ими живёт многоколонник.
            "column-rule-width" | "row-rule-width" => {
                if let Some(l) = gap_list(v, |s| gap_width(s, vcx)) {
                    self.accept_declaration();
                    self.set_gap_widths(key == "column-rule-width", &l);
                }
            }
            "column-rule-style" | "row-rule-style" => {
                if let Some(l) = gap_list(v, gap_style) {
                    self.accept_declaration();
                    let double = v.trim().eq_ignore_ascii_case("double");
                    let kinds = gap_list(v, super::gap_rules::gap_kind);
                    if key == "column-rule-style" {
                        self.set_gap_styles(true, &l);
                        self.column_rule_double = double;
                        self.column_rule_kinds = kinds;
                    } else {
                        self.set_gap_styles(false, &l);
                        self.row_rule_double = double;
                        self.row_rule_kinds = kinds;
                    }
                }
            }
            "column-rule-color" | "row-rule-color" => {
                // Ненаследуемое свойство со словом `inherit` (css-cascade-4 §7.2):
                // цвет линейки родителя целиком, а не `currentcolor` своего текста
                // (`multicol-rule-color-inherit-001/002`).
                if v == "inherit" {
                    self.inherit_bits |= if key == "column-rule-color" {
                        inh::COLUMN_RULE_C
                    } else {
                        inh::ROW_RULE_C
                    };
                    return;
                }
                if let Some(l) = gap_list(v, |s| gap_color(s, vcx)) {
                    self.accept_declaration();
                    self.set_gap_colors(key == "column-rule-color", &l);
                }
            }
            // §inset: `{column,row}-rule-inset-{cap,junction}-{start,end}`,
            // по одной грани; слоты: [cap-start, cap-end, junction-start,
            // junction-end]. Сокращения раскрыты разбором блока.
            k if k
                .strip_prefix("column-")
                .or_else(|| k.strip_prefix("row-"))
                .is_some_and(|t| t.starts_with("rule-inset-")) =>
            {
                let column = k.starts_with("column-");
                let slot = match k.split_once("rule-inset-").map_or("", |(_, t)| t) {
                    "cap-start" => 0,
                    "cap-end" => 1,
                    "junction-start" => 2,
                    "junction-end" => 3,
                    _ => {
                        *hit = false;
                        return;
                    }
                };
                let Some(inset) = gap_inset(v, vcx) else {
                    return;
                };
                self.accept_declaration();
                let arr = if column {
                    &mut self.column_rule_inset
                } else {
                    &mut self.row_rule_inset
                };
                arr.get_or_insert([GapInset::Len(Len::Px(0.0)); 4])[slot] = inset;
            }
            _ => self.apply_multicol_rule_flags(key, v, hit, vcx),
        }
    }
}
