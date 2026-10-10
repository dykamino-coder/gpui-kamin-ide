//! Computed::apply_multicol_rules, хвост цепочки: visibility/overlap/break линеек, margin-trim, сокращения линеек, columns. Ветви в исходном порядке.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_multicol_rule_flags(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
            // §visibility-items: 0 normal, 1 all, 2 around, 3 between.
            "rule-visibility-items"
            | "column-rule-visibility-items"
            | "row-rule-visibility-items" => {
                let code = match v.trim() {
                    "normal" => Some(0u8),
                    "all" => Some(1),
                    "around" => Some(2),
                    "between" => Some(3),
                    _ => None,
                };
                if let Some(code) = code {
                    if key != "row-rule-visibility-items" {
                        self.column_rule_visibility = Some(code);
                    }
                    if key != "column-rule-visibility-items" {
                        self.row_rule_visibility = Some(code);
                    }
                }
            }
            // §overlap: порядок краски пересекающихся линеек.
            "rule-overlap" => match v.trim() {
                "row-over-column" => self.rule_column_over_row = Some(false),
                "column-over-row" => self.rule_column_over_row = Some(true),
                _ => {}
            },
            // §break: `none` 0, `normal` 1, `intersection` 2.
            "column-rule-break" | "row-rule-break" | "rule-break" => {
                let code = match v.trim() {
                    "none" => Some(0u8),
                    "normal" => Some(1),
                    "intersection" => Some(2),
                    _ => None,
                };
                if let Some(code) = code {
                    if key != "row-rule-break" {
                        self.column_rule_break = Some(code);
                    }
                    if key != "column-rule-break" {
                        self.row_rule_break = Some(code);
                    }
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
                    self.margin_trim = bits;
                }
            }
            // Сокращения линеек (css-gaps-1 §rule-shorthands): список
            // `<gap-rule>` через запятую с `repeat()`; `rule` — обе оси.
            // Незнакомый токен делает недействительным ВСЁ объявление
            // (CSS 2.1 §4.1.7), а токены с пробелами внутри скобок
            // (`rgba(0, 0, 255, 0.5)`) больше не рвутся.
            "column-rule" | "row-rule" | "rule" => self.gap_rule_shorthand(key, v),
            "columns" => {
                // `columns: [<ширина> || <число>] [/ <column-height>]?`
                // (css-multicol-2 §columns): ширина и число в любом порядке;
                // `auto` оставляет сторону нерешённой (не затирать уже
                // разобранную ширину). Короткая форма сбрасывает
                // `column-height` и `column-wrap` в начальные
                // (`columns-shorthand-reset-wrap`, `columns: 2 / 0`).
                let (head, tail) = v.split_once('/').map_or((v, None), |(a, b)| (a, Some(b)));
                // Грамматика css-multicol-1 §columns: «<<'column-width'>> ||
                // <<'column-count'>>» — не больше двух слов, каждое не больше раза.
                // Иное объявление НЕВАЛИДНО и отбрасывается ЦЕЛИКОМ (css-syntax-3
                // §consume-declaration), ничего не сбрасывая. Прежде слова брались
                // поштучно: `columns: 8 auto 6em` давал `column-count: 8` и
                // `column-width: 6em`, и с пакетом B (`column_width` из `merged`,
                // в точках) колонок стало min(8, ⌊240/120⌋) = 2 вместо 4 —
                // `multicol-columns-invalid-002` 0.00 (v39) → 0.67 (v40).
                let words: Vec<&str> = head.split_whitespace().collect();
                if words.is_empty() || words.len() > 2 {
                    return;
                }
                let mut count: Option<u16> = None;
                let mut width: Option<Len> = None;
                for token in &words {
                    if token.eq_ignore_ascii_case("auto") {
                        continue;
                    }
                    match token.parse::<u16>() {
                        Ok(n) if n > 0 && count.is_none() => count = Some(n),
                        // `0` — как прежде: без действия (как длина `column-width: 0`
                        // у нас отвергается и в полной форме).
                        Ok(0) => {}
                        Ok(_) => return,
                        Err(_) => match Len::parse(token) {
                            Some(l)
                                if width.is_none()
                                    && !matches!(
                                        l,
                                        Len::Auto
                                            | Len::Pct(_)
                                            | Len::MinContent
                                            | Len::MaxContent
                                            | Len::FitContent
                                    )
                                    && !matches!(l, Len::Px(w) | Len::Em(w) if w < 0.0) =>
                            {
                                width = Some(l)
                            }
                            _ => return,
                        },
                    }
                }
                self.column_height = None;
                self.column_wrap = None;
                if let Some(t) = tail {
                    self.apply_one("column-height", t.trim());
                }
                if let Some(n) = count {
                    self.column_count = Some(n);
                }
                if let Some(l) = width {
                    self.column_width = Some(l);
                }
            }
            _ => *hit = false,
        }
    }
}
