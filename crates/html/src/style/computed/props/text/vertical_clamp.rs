//! Computed::apply_text, хвост цепочки: vertical-align/baseline-shift, line-clamp, block-ellipsis, writing-mode, text-orientation, hyphens, tab-size. Ветви в исходном порядке после apply_text_breaking.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_text_vertical_clamp(
        &mut self,
        key: &str,
        val: &str,
        v: &str,
        hit: &mut bool,
    ) {
        match key {
            // `baseline-shift` (css-inline-3 §5.2.2) — ТОТ ЖЕ разбор:
            // спека сама пишет соответствие («''vertical-align/top''
            // (''baseline-shift: top'')…», css-inline-3 §5.2). Значения
            // совпадают дословно, кроме середины: у сокращения она
            // `middle`, у длинной записи — `center`.
            "vertical-align" | "baseline-shift" => {
                // Надстрочный и подстрочный кусок остаются В СТРОКЕ, только
                // сдвигаются от базовой линии, — это не выравнивание коробки,
                // поэтому у них своё поле. Доли кегля браузерные.
                // Сдвиг хранится ДОЛЕЙ кегля: у длины она считается при
                // разборе, у процента она и есть написанное (CSS 2.1 §10.8.1
                // считает процент от `line-height`, но у нас доля умножается
                // на кегль — при `line-height: normal` это то же самое с
                // точностью до полулидинга, а точная формула ждёт модели
                // строчной коробки).
                self.vertical_shift = match v {
                    "super" => Some(-1.0 / 3.0),
                    "sub" => Some(1.0 / 5.0),
                    other => match Len::parse(other) {
                        // Долей кегля пишутся процент и `em` — их и храним
                        // долей. Ось сдвига смотрит вниз, а положительное
                        // значение поднимает знак ВВЕРХ.
                        Some(Len::Pct(k)) => {
                            // Процент — доля `line-height`, и считается он
                            // позже: кладём в своё поле.
                            self.vertical_shift_pct = (k != 0.0).then_some(-k);
                            None
                        }
                        Some(Len::Em(k)) => (k != 0.0).then_some(-k),
                        _ => None,
                    },
                };
                // Точки и единицы ШРИФТА считаются сразу в точках: доля
                // кегля тут не годится — `ex` зависит от метрик гарнитуры, а
                // кегль строчного приходит наследованием уже после разбора.
                match Len::parse(v) {
                    // Точки не зависят ни от чего — сразу в поле.
                    Some(Len::Px(px)) => self.vertical_shift_px = (px != 0.0).then_some(-px),
                    // Единицы шрифта ждут набора: у строчного своих метрик
                    // обычно нет, они приходят наследованием уже после
                    // разбора, и здесь вышли бы от чужой гарнитуры.
                    Some(l @ (Len::Ex(_) | Len::Ch(_))) => self.vertical_shift_len = Some(l),
                    _ => {}
                }
                self.vertical_align_text = match v {
                    "text-top" => Some(true),
                    "text-bottom" => Some(false),
                    _ => None,
                };
                self.vertical_align = match v {
                    // `center` — написание середины в `baseline-shift`
                    // (css-inline-3 §5.2.2): «Align the center of the aligned
                    // subtree with the center of the line box».
                    "middle" | "center" => Some(Align::Center),
                    "top" => Some(Align::Start),
                    "bottom" => Some(Align::End),
                    "baseline" => Some(Align::Baseline),
                    _ => None,
                }
            }
            // css-overflow-4 §5.1: `none | [<integer> || <'block-ellipsis'>]
            // -webkit-legacy?`; `auto` — срез по высоте контейнера. Строка
            // многоточия идёт маркером абзаца (`lines::marker_str`).
            "line-clamp" => {
                self.line_clamp = None;
                self.clamp_auto = None;
                self.clamp_legacy = Some(false);
                self.clamp_mark = None;
                // Одна строка многоточия без числа — `max-lines: none` при
                // `continue: collapse` (§5.1: «Sets continue to collapse if
                // either or both values are specified»), то есть срез по
                // высоте, как у `auto` (`line-clamp-balance-009`).
                let mut ellipsis_given = false;
                let mut rest = v.trim();
                while !rest.is_empty() {
                    let quote = rest.as_bytes()[0];
                    let (word, tail) = if quote == b'"' || quote == b'\'' {
                        match rest[1..].find(quote as char) {
                            Some(end) => (&rest[..end + 2], &rest[end + 2..]),
                            None => (rest, ""),
                        }
                    } else {
                        match rest.find(char::is_whitespace) {
                            Some(end) => (&rest[..end], &rest[end..]),
                            None => (rest, ""),
                        }
                    };
                    rest = tail.trim_start();
                    if word.len() >= 2 && (word.starts_with('"') || word.starts_with('\'')) {
                        self.clamp_mark = Some(word[1..word.len() - 1].to_string());
                        ellipsis_given = true;
                    } else if word.eq_ignore_ascii_case("no-ellipsis") {
                        self.clamp_mark = Some(String::new());
                        ellipsis_given = true;
                    } else if word.eq_ignore_ascii_case("auto") {
                        self.clamp_auto = Some(true);
                    } else if let Ok(n) = word.parse::<u32>() {
                        self.line_clamp = Some(n);
                    }
                }
                if ellipsis_given && self.line_clamp.is_none() {
                    self.clamp_auto = Some(true);
                }
            }
            // Лонгхенд `block-ellipsis`: `no-ellipsis | auto | <string>`.
            "block-ellipsis" => {
                let t = v.trim();
                self.clamp_mark = if t.eq_ignore_ascii_case("no-ellipsis") {
                    Some(String::new())
                } else if t.len() >= 2 && (t.starts_with('"') || t.starts_with('\'')) {
                    Some(collapse_forced_breaks(&unescape_content(
                        &t[1..t.len() - 1],
                    )))
                } else {
                    None
                };
            }
            "-webkit-line-clamp" => {
                self.line_clamp = v.trim().parse().ok();
                self.clamp_auto = None;
                self.clamp_legacy = Some(true);
            }
            "-webkit-box-orient" => {
                self.webkit_box_vertical = Some(v.eq_ignore_ascii_case("vertical"))
            }

            // --- Письмо и цветовые фильтры -------------------------------------
            "direction" | "unicode-bidi" => bidi_properties::apply(self, key, v),

            // --- Обтекание и направление письма --------------------------------
            // `initial-letter: normal | <size> [<sink> | drop | raise]`
            // (css-inline-3 §initial-letter). Число — высота буквицы в
            // строках (не меньше 1); целое — осадка; `raise` = 1; `drop` и
            // умолчание — осадка равна размеру, округлённому вниз. Порядок
            // слов свободный (`drop 3`). Кегль и строку буквицы считает
            // раскладка (`render::initial_letter_float`).
            "initial-letter" => {
                self.initial_letter = None;
                if v != "normal" {
                    let mut size: Option<f32> = None;
                    let mut sink: Option<u32> = None;
                    for word in v.split_ascii_whitespace() {
                        match word {
                            "drop" => sink = Some(0),
                            "raise" => sink = Some(1),
                            w if size.is_none() => {
                                size = w.parse::<f32>().ok().filter(|n| *n >= 1.0);
                            }
                            w => sink = w.parse::<u32>().ok().filter(|n| *n >= 1),
                        }
                    }
                    if let Some(size) = size {
                        let sink = match sink {
                            Some(0) | None => (size.floor() as u32).max(1),
                            Some(n) => n,
                        };
                        self.initial_letter = Some((size, sink));
                    }
                }
            }
            // css-rhythm-1 §2: «A value other than `none` … causes the box to
            // establish an independent formatting context». Сам шаг ритма мы
            // не считаем, но ПРИЗНАК контекста нужен и без него: без него
            // `block-step-size-establishes-*` неотличимы от обычного блока, и
            // сосед флоата (`covered_flow_tail`, `render.rs`) накрывает их
            // вместо того, чтобы встать сбоку. Проба
            // (`target/scout-floatplace-2026-09.md` §5.3): с контекстом обе
            // пары и обе их `-list-item`-разновидности дают 0.00.
            "block-step-size" => {
                if v != "none" && v != "auto" {
                    self.flow_root = Some(true);
                }
            }
            "text-orientation" => {
                self.upright = Some(v == "upright");
                self.text_sideways = Some(v == "sideways" || v == "sideways-right");
            }
            "writing-mode" => {
                // `sideways-*` отличается от `vertical-*` только поворотом
                // глифов, а направление потока у них общее.
                let vertical = v.starts_with("vertical") || v.starts_with("sideways");
                self.vertical = Some(vertical);
                self.vertical_rl = Some(vertical && v.ends_with("rl"));
                self.sideways = Some(v.starts_with("sideways"));
            }
            "text-combine-upright" => {
                let mut it = v.split_whitespace();
                self.combine_upright = match it.next() {
                    Some("none") | None => None,
                    Some("all") => Some(0),
                    Some("digits") => Some(it.next().and_then(|n| n.parse().ok()).unwrap_or(2)),
                    _ => None,
                };
            }

            // --- Переносы и обрезка -------------------------------------------
            "hyphens" => {
                self.hyphenate = Some(v != "none");
                // `manual` только УВАЖАЕТ расставленные знаки мягкого
                // переноса, сам их не ставит. Разделять обязательно: под одним
                // флагом `auto` и `manual` вели себя одинаково.
                self.hyphens_auto = Some(v == "auto");
            }
            "tab-size" => tab_size::apply(self, v),
            _ => *hit = false,
        }
    }
}
