//! Физические padding и margin: значения сторон, наследование и порядок деклараций.

use super::*;
use crate::document::state::ValueCx;

impl Computed {
    pub(super) fn apply_box_model_sides(
        &mut self,
        key: &str,
        v: &str,
        hit: &mut bool,
        vcx: ValueCx<'_>,
    ) {
        match key {
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                self.apply_padding_side(key, v, vcx);
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
                self.margin = parsed_value!(self, parsed_sides(v, true, vcx));
                self.margin_inherit = [false; 4];
                self.side_seq.margin = [self.decl_seq; 4];
            }
            "margin-top" => {
                if v == "inherit" {
                    self.margin_inherit[0] = true;
                    return;
                }
                if !self.css_wide_value && side_length(v, true, vcx).is_none() {
                    return;
                }
                self.margin.top = apply_parsed!(self, Len::parse(v, vcx));
                self.side_seq.margin[0] = self.decl_seq;
            }
            "margin-right" => {
                if v == "inherit" {
                    self.margin_inherit[1] = true;
                    return;
                }
                if !self.css_wide_value && side_length(v, true, vcx).is_none() {
                    return;
                }
                // Смесь «доля ± точки» доживает индексом: раскладка складывает
                // её сама (css-values-4 §10.9), а вклад решает долю от нуля
                // (css-sizing-3 §5.2.1, `calc-margins-*`). Вертикальные поля
                // — по-прежнему `parse`: смесь там закрыла бы схлопывание.
                self.margin.right = apply_parsed!(
                    self,
                    crate::style::values::value::fold_zero_percentage(
                        Len::parse_mixed(v, vcx),
                        vcx
                    )
                );
                self.side_seq.margin[1] = self.decl_seq;
            }
            "margin-bottom" => {
                if v == "inherit" {
                    self.margin_inherit[2] = true;
                    return;
                }
                if !self.css_wide_value && side_length(v, true, vcx).is_none() {
                    return;
                }
                self.margin.bottom = apply_parsed!(self, Len::parse(v, vcx));
                self.side_seq.margin[2] = self.decl_seq;
            }
            "margin-left" => {
                if v == "inherit" {
                    self.margin_inherit[3] = true;
                    return;
                }
                if !self.css_wide_value && side_length(v, true, vcx).is_none() {
                    return;
                }
                // Смесь «доля ± точки» доживает (см. `margin-right`).
                self.margin.left = apply_parsed!(
                    self,
                    crate::style::values::value::fold_zero_percentage(
                        Len::parse_mixed(v, vcx),
                        vcx
                    )
                );
                self.side_seq.margin[3] = self.decl_seq;
            }

            _ => self.apply_box_model_logical(key, v, hit, vcx),
        }
    }

    fn apply_padding_side(&mut self, key: &str, value: &str, vcx: ValueCx<'_>) {
        let index = match key {
            "padding-top" => 0,
            "padding-right" => 1,
            "padding-bottom" => 2,
            "padding-left" => 3,
            _ => return,
        };
        // CSS 2.1 §6.2.1: inherit copies the parent's computed side.
        if value == "inherit" {
            self.padding_inherit_side[index] = true;
            return;
        }
        if !self.css_wide_value && side_length(value, false, vcx).is_none() {
            return;
        }
        let parsed = apply_parsed!(
            self,
            crate::style::values::value::fold_zero_percentage(side_length(value, false, vcx), vcx)
        );
        match index {
            0 => self.padding.top = parsed,
            1 => self.padding.right = parsed,
            2 => self.padding.bottom = parsed,
            _ => self.padding.left = parsed,
        }
        self.side_seq.padding[index] = self.decl_seq;
    }
}

#[cfg(test)]
mod tests;

/// Сокращение годно только при успехе разбора каждого компонента.
pub(in crate::style::computed::props) fn parsed_sides(
    v: &str,
    auto: bool,
    vcx: ValueCx<'_>,
) -> Option<Sides> {
    let words = split_outside_parens(v);
    if words.is_empty() || words.len() > 4 {
        return None;
    }
    let lengths = words
        .iter()
        .map(|w| side_length(w, auto, vcx))
        .collect::<Option<Vec<_>>>()?;
    let top = Some(lengths[0]);
    let right = Some(*lengths.get(1).unwrap_or(&lengths[0]));
    let bottom = Some(*lengths.get(2).unwrap_or(&lengths[0]));
    let left = Some(
        *lengths
            .get(3)
            .unwrap_or(&lengths[if lengths.len() > 1 { 1 } else { 0 }]),
    );
    Some(Sides {
        top,
        right,
        bottom,
        left,
    })
}

pub(in crate::style::computed::props) fn side_length(
    v: &str,
    auto: bool,
    vcx: ValueCx<'_>,
) -> Option<Len> {
    if stretch_keyword(v) {
        return None;
    }
    let len = if auto {
        Len::parse_mixed(v, vcx)?
    } else {
        size_range::parse_nonnegative_size(v, vcx)?
    };
    match len {
        Len::Auto if auto => Some(len),
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
        Len::Px(n) | Len::Pct(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n) if !auto && n < 0.0 => None,
        _ => Some(len),
    }
}
