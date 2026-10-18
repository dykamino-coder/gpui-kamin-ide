//! Физические padding и margin: значения сторон, наследование и порядок деклараций.

use super::*;

impl Computed {
    pub(super) fn apply_box_model_sides(&mut self, key: &str, v: &str, hit: &mut bool) {
        match key {
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                self.apply_padding_side(key, v);
            }
            // Физическая запись ГАСИТ логический слот той же стороны: разбор
            // идёт в порядке каскада, и авторский `margin: 0` обязан бить
            // более ранний `margin-block` таблицы агента — а разрешение
            // логических идёт после каскада и иначе перекрывало бы всё.
            // Соответствие сторон берётся горизонтальное: письмо на разборе
            // ещё неизвестно, а гасят почти всегда сбросом всех сторон.
            "margin" => {
                if v == "inherit" {
                    self.margin_inherit = [true; 4];
                    return;
                }
                self.margin = Sides::shorthand(v);
                self.side_seq.margin = [self.decl_seq; 4];
            }
            "margin-top" => {
                if v == "inherit" {
                    self.margin_inherit[0] = true;
                    return;
                }
                self.margin.top = Len::parse(v);
                self.side_seq.margin[0] = self.decl_seq;
            }
            "margin-right" => {
                if v == "inherit" {
                    self.margin_inherit[1] = true;
                    return;
                }
                // Смесь «доля ± точки» доживает индексом: раскладка складывает
                // её сама (css-values-4 §10.9), а вклад решает долю от нуля
                // (css-sizing-3 §5.2.1, `calc-margins-*`). Вертикальные поля
                // — по-прежнему `parse`: смесь там закрыла бы схлопывание.
                self.margin.right =
                    crate::style::values::value::fold_zero_percentage(Len::parse_mixed(v));
                self.side_seq.margin[1] = self.decl_seq;
            }
            "margin-bottom" => {
                if v == "inherit" {
                    self.margin_inherit[2] = true;
                    return;
                }
                self.margin.bottom = Len::parse(v);
                self.side_seq.margin[2] = self.decl_seq;
            }
            "margin-left" => {
                if v == "inherit" {
                    self.margin_inherit[3] = true;
                    return;
                }
                // Смесь «доля ± точки» доживает (см. `margin-right`).
                self.margin.left =
                    crate::style::values::value::fold_zero_percentage(Len::parse_mixed(v));
                self.side_seq.margin[3] = self.decl_seq;
            }

            _ => self.apply_box_model_logical(key, v, hit),
        }
    }

    fn apply_padding_side(&mut self, key: &str, value: &str) {
        let (index, slot) = match key {
            "padding-top" => (0, &mut self.padding.top),
            "padding-right" => (1, &mut self.padding.right),
            "padding-bottom" => (2, &mut self.padding.bottom),
            "padding-left" => (3, &mut self.padding.left),
            _ => return,
        };
        // CSS 2.1 §6.2.1: inherit copies the parent's computed side.
        if value == "inherit" {
            self.padding_inherit_side[index] = true;
            return;
        }
        // CSS 2.1 §8.4: a negative padding leaves the previous slot intact.
        if matches!(
            Len::parse(value),
            Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
        ) {
            return;
        }
        *slot = crate::style::values::value::fold_zero_percentage(Len::parse(value));
        self.side_seq.padding[index] = self.decl_seq;
    }
}

#[cfg(test)]
mod tests;
