//! Computed::apply_one: color, line-height, text-align/indent/transform/wrap, white-space, word-*, hyphens, vertical-align, line-clamp, text-box*, writing modes.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod breaking;
mod overflow_case;
mod vertical_clamp;

impl Computed {
    #[allow(unused_variables)]
    pub(crate) fn apply_text(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // Неразборный цвет делает объявление недействительным (§4.2):
            // прежнее значение живёт, а не сменяется умолчанием. Пустой слот
            // у нас и означает «взять у родителя», поэтому `inherit` его
            // очищает (`color-174`).
            "color" => {
                self.color = if v == "inherit" {
                    None
                } else {
                    Color::parse(v).or(self.color)
                }
            }
            "line-height" => {
                // Голое число в line-height — множитель, а не пиксели, и
                // наследуется оно множителем: у потомка своя высота строки.
                //
                // Доля — наоборот: §10.8.1 «Computed value: for <length> and
                // <percentage> the absolute value», то есть `200%` считается
                // от СВОЕГО кегля и наследуется уже точками. У нас обе записи
                // давали `Len::Pct`, доля доживала до потомка и множилась на
                // его кегль (`c548-ln-ht-003` против зелёной `-004` — та же
                // разметка, разная запись). `Len::Em` сводится к точкам до
                // наследования, поэтому доля тегируется им.
                // `normal` — ЗАДАННОЕ значение, а не «не задано»: незаданное
                // поле у нас берётся от родителя, и `p { line-height: normal }`
                // молча наследовал `:root { line-height: 50px }` — документ
                // уезжал вниз на полулидинг (`rlh-unit-001`: зелёный квадрат
                // ниже эталона на 30 точек). Меткой служит `Len::Auto`: у всех
                // потребителей высоты строки уже есть для неё запасная ветка
                // «по метрикам шрифта» (`inline.rs:719`, `:1529`, `:2445`), а
                // `apply.rs:1618` на `Len::Auto` явно ничего не задаёт.
                if v.eq_ignore_ascii_case("normal") {
                    self.line_height = Some(Len::Auto);
                    return;
                }
                let parsed = match v.parse::<f32>() {
                    Ok(mult) if !v.ends_with("px") => Some(Len::Pct(mult)),
                    _ => match Len::parse(v) {
                        Some(Len::Pct(k)) if v.trim_end().ends_with('%') => Some(Len::Em(k)),
                        other => other,
                    },
                };
                // Отрицательная высота строки недействительна (§10.8.1):
                // объявление отбрасывается целиком, прежнее значение живёт.
                let neg = |l: &Len| {
                    matches!(
                        l,
                        Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                            if *v < 0.0
                    )
                };
                self.line_height = match parsed {
                    Some(l) if neg(&l) => self.line_height,
                    other => other,
                };
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
                self.text_align = None;
                self.text_align_match_parent |= if key == "text-align" { 3 } else { 1 };
            }
            "text-align" | "text-align-all" => {
                if matches!(
                    v,
                    "center" | "right" | "left" | "start" | "end" | "justify" | "justify-all"
                ) {
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
                self.text_align_last = None;
                self.text_align_match_parent |= 2;
            }
            "text-align-last" => {
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
            "letter-spacing" => self.letter_spacing = Len::parse_spacing(v),
            _ => self.apply_text_overflow_case(key, val, v, hit),
        }
    }
}
