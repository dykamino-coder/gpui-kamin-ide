//! Computed::apply_box_model, продолжение цепочки: padding-* по сторонам и margin*. Ветви в исходном порядке; не совпавший ключ уходит в apply_box_model_logical.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_box_model_sides(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            "padding-top" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[0] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.top = crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[0] = self.decl_seq;
            }
            "padding-right" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[1] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.right =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[1] = self.decl_seq;
            }
            "padding-bottom" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[2] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.bottom =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[2] = self.decl_seq;
            }
            "padding-left" => {
                // `inherit` разбором не выражается: слово копирует вычисленное
                // значение родителя (§6.2.1). Без ветки `Len::parse` отдавал
                // `None`, и отступ обнулялся.
                if v == "inherit" {
                    self.padding_inherit_side[3] = true;
                    return;
                }
                // Отрицательный внутренний отступ невалиден (§8.4) — слот
                // не трогается (ref-no-vert-space-between и родня).
                if matches!(
                    Len::parse(v),
                    Some(Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n)) if n < 0.0
                ) {
                    return;
                }
                self.padding.left =
                    crate::style::values::value::fold_zero_percentage(Len::parse(v));
                self.side_seq.padding[3] = self.decl_seq;
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

            _ => self.apply_box_model_logical(key, val, v, hit),
        }
    }
}
