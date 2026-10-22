//! Группа свойств `Computed::apply_one`: текст — `color`, `text-justify`,
//! `text-align`/`-all`/`-last` с `match-parent`, `letter-spacing`; длинные
//! свойства текста и оформления реестра (`white-space-collapse`,
//! `text-wrap-*`, `text-emphasis-style/-color`, `text-decoration-*`) — в
//! `longhands` с разбором `values`; `text-indent`/`-transform`, `word-*`,
//! `hyphens`, `vertical-align`, `line-clamp`, `text-box*` и режимы письма — в
//! подмодулях.

use crate::document::state::ValueCx;
use crate::style::computed::*;
use crate::style::css::{Grammar, LonghandId};
use crate::style::values::value::{Len, StyleColor};

mod breaking;
mod hyphen_limits;
mod longhands;
mod overflow_case;
mod text_box_values;
pub(crate) mod values;
mod vertical_clamp;

impl Computed {
    pub(crate) fn apply_text(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
        if let Some(id) = LonghandId::from_name(key)
            .filter(|id| matches!(id.grammar(), Grammar::Text | Grammar::TextDecor))
        {
            return self.apply_text_longhand(id, v, vcx);
        }
        match key {
            // Неразборный цвет делает объявление недействительным (§4.2):
            // прежнее значение живёт, а не сменяется умолчанием. Пустой слот
            // у нас и означает «взять у родителя», поэтому `inherit` его
            // очищает (`color-174`). `currentcolor` в `color` — то же
            // `inherit` (css-color-4 §6.4), а функция с ним (`color-mix()`,
            // относительный цвет) вычисляется от цвета РОДИТЕЛЯ при слиянии
            // (`color_function`) и наследуется уже абсолютным цветом.
            "color" => {
                let parsed = if v == "inherit" {
                    Some(StyleColor::Current)
                } else {
                    record_parsed!(self, StyleColor::parse_value(v, vcx))
                };
                match parsed {
                    Some(StyleColor::Current) => {
                        self.color = None;
                        self.color_function = None;
                    }
                    Some(StyleColor::Absolute(color)) => {
                        self.color = Some(color);
                        self.color_function = None;
                    }
                    Some(function) => {
                        self.color = None;
                        self.color_function = Some(function);
                    }
                    None => {}
                }
            }
            // `text-justify: none` запрещает выключку целиком: строка с
            // `text-align: justify` прижимается к началу, как `start`
            // (css-text-3 §7.3). Прочие значения различают, ЧТО растягивать —
            // пробелы или знаки; у нас растягиваются пробелы, и это поведение
            // `auto`/`inter-word`.
            "text-justify" => {
                if matches!(
                    v,
                    "none" | "auto" | "inter-word" | "inter-character" | "distribute" | "ruby"
                ) {
                    self.accept_declaration();
                    self.ruby_justify = Some(v == "ruby");
                    self.justify_chars = match v {
                        "inter-word" => Some(0),
                        "inter-character" | "distribute" => Some(2),
                        _ => Some(1),
                    };
                }
                self.no_justify = match v {
                    "none" => Some(true),
                    "auto" | "inter-word" | "inter-character" | "distribute" | "ruby" => {
                        Some(false)
                    }
                    _ => self.no_justify,
                };
            }
            // `text-align` — сокращение (css-text-4 §text-align): `match-parent`
            // ставит его обоим лонгхендам; `text-align-all` — только всем
            // строкам, кроме последней.
            "text-align" | "text-align-all" if v == "match-parent" => {
                self.accept_declaration();
                self.text_align = None;
                self.text_align_match_parent |= if key == "text-align" { 3 } else { 1 };
            }
            "text-align" | "text-align-all" => {
                if matches!(
                    v,
                    "center" | "right" | "left" | "start" | "end" | "justify" | "justify-all"
                ) {
                    self.accept_declaration();
                    self.text_align_match_parent &= if key == "text-align" { 0 } else { 2 };
                }
                self.text_align = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" | "justify-all" => Some(TextAlign::Justify),
                    _ => self.text_align,
                };
                // `justify-all` — это выключка ВМЕСТЕ с последней строкой:
                // сокращение от `text-align: justify` + `text-align-last:
                // justify`.
                if v == "justify-all" && key == "text-align" {
                    self.text_align_last = Some(TextAlign::Justify);
                }
            }
            "text-align-last" if v == "match-parent" => {
                self.accept_declaration();
                self.text_align_last = None;
                self.text_align_match_parent |= 2;
            }
            "text-align-last" => {
                if matches!(
                    v,
                    "auto" | "center" | "right" | "left" | "start" | "end" | "justify"
                ) {
                    self.accept_declaration();
                }
                if !matches!(
                    v,
                    "auto" | "center" | "right" | "left" | "start" | "end" | "justify" | "inherit"
                ) {
                    return;
                }
                if v != "auto" && v != "inherit" {
                    self.text_align_match_parent &= 1;
                }
                self.text_align_last = match v {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" => Some(TextAlign::Left),
                    "start" => Some(TextAlign::Start),
                    "end" => Some(TextAlign::End),
                    "justify" => Some(TextAlign::Justify),
                    _ => self.text_align_last,
                }
            }
            "letter-spacing" => {
                self.letter_spacing = parsed_value!(
                    self,
                    Len::parse_spacing(v, vcx)
                        .map(Some)
                        .or_else(|| v.eq_ignore_ascii_case("inherit").then_some(None))
                )
            }
            _ => self.apply_text_overflow_case(key, v, hit, vcx),
        }
    }
}
