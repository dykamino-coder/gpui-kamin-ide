//! Computed::apply_multicol, хвост цепочки: лонгхенды линеек промежутков (*-rule-width/style/color/inset/visibility/overlap/break), margin-trim, сокращения линеек и columns. Ветви в исходном порядке после свойств колонок и разрывов.

use super::*;

mod rule_flags;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_multicol_rules(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // Лонгхенды линеек промежутков (css-gaps-1 §color-style-width):
            // список через запятую с `repeat()`, `rule-*` ставит обе оси.
            // Первое значение уходит в скаляры — ими живёт многоколонник.
            "column-rule-width" | "row-rule-width" | "rule-width" => {
                if let Some(l) = gap_list(v, gap_width) {
                    if key != "row-rule-width" {
                        self.set_gap_widths(true, &l);
                    }
                    if key != "column-rule-width" {
                        self.set_gap_widths(false, &l);
                    }
                }
            }
            "column-rule-style" | "row-rule-style" | "rule-style" => {
                if let Some(l) = gap_list(v, gap_style) {
                    let double = v.trim().eq_ignore_ascii_case("double");
                    if key != "row-rule-style" {
                        self.set_gap_styles(true, &l);
                        self.column_rule_double = double;
                    }
                    if key != "column-rule-style" {
                        self.set_gap_styles(false, &l);
                        self.row_rule_double = double;
                    }
                }
            }
            "column-rule-color" | "row-rule-color" | "rule-color" => {
                // Ненаследуемое свойство со словом `inherit` (css-cascade-4 §7.2):
                // цвет линейки родителя целиком, а не `currentcolor` своего текста
                // (`multicol-rule-color-inherit-001/002`).
                if v == "inherit" {
                    if key != "row-rule-color" {
                        self.inherit_bits |= inh::COLUMN_RULE_C;
                    }
                    if key != "column-rule-color" {
                        self.inherit_bits |= inh::ROW_RULE_C;
                    }
                    return;
                }
                if let Some(l) = gap_list(v, gap_color) {
                    if key != "row-rule-color" {
                        self.set_gap_colors(true, &l);
                    }
                    if key != "column-rule-color" {
                        self.set_gap_colors(false, &l);
                    }
                }
            }
            // §inset: `[column-|row-]rule-inset[-cap|-junction][-start|-end]`.
            // Без стороны — обе стороны, без вида — и концы, и стыки; два
            // значения — начало и конец. Слоты: [cap-start, cap-end,
            // junction-start, junction-end].
            k if k
                .strip_prefix("column-")
                .or_else(|| k.strip_prefix("row-"))
                .unwrap_or(k)
                .starts_with("rule-inset") =>
            {
                let tail = k
                    .strip_prefix("column-")
                    .or_else(|| k.strip_prefix("row-"))
                    .unwrap_or(k);
                let tail = &tail["rule-inset".len()..];
                let plain = tail.is_empty() || tail == "-start" || tail == "-end";
                let cap = plain || tail.starts_with("-cap");
                let junction = plain || tail.starts_with("-junction");
                let start = !tail.ends_with("-end");
                let end = !tail.ends_with("-start");
                let toks = split_outside_parens(v);
                let (vs, ve) = match toks.as_slice() {
                    [a] => (gap_inset(a), gap_inset(a)),
                    [a, b] => (gap_inset(a), gap_inset(b)),
                    _ => (None, None),
                };
                if let (Some(vs), Some(ve)) = (vs, ve) {
                    let slots = [cap && start, cap && end, junction && start, junction && end];
                    for column in [true, false] {
                        if (column && k.starts_with("row-"))
                            || (!column && k.starts_with("column-"))
                        {
                            continue;
                        }
                        let arr = if column {
                            &mut self.column_rule_inset
                        } else {
                            &mut self.row_rule_inset
                        };
                        let mut cur = arr.unwrap_or([GapInset::Len(Len::Px(0.0)); 4]);
                        for (i, on) in slots.iter().enumerate() {
                            if *on {
                                cur[i] = if i % 2 == 0 { vs } else { ve };
                            }
                        }
                        *arr = Some(cur);
                    }
                }
            }
            _ => self.apply_multicol_rule_flags(key, val, v, hit),
        }
    }
}
