//! Computed::apply_text, продолжение цепочки: text-indent, text-box*, hanging-punctuation, разрывы слов и строк, text-fit, text-wrap. Ветви в исходном порядке; не совпавший ключ уходит в apply_text_vertical_clamp.

use super::*;

impl Computed {
    #[allow(unused_variables)]
    pub(super) fn apply_text_breaking(&mut self, key: &str, val: &str, v: &str, hit: &mut bool) {
        match key {
            // Отступ первой строки. Кроме длины значение несёт до двух
            // ключевых слов (css-text-3 §7.1): `each-line` повторяет отступ
            // после КАЖДОГО жёсткого разрыва, `hanging` переворачивает выбор —
            // отступ получают все строки, КРОМЕ той, что получила бы его.
            // Порядок слов свободный, поэтому значение разбирается по словам.
            "text-indent" => text_indent::apply(self, v),
            // `text-box-trim` (css-inline-3 §4.2): у блочного контейнера
            // срезается ПОЛУЛИДИНГ первой и/или последней строки, чтобы край
            // содержимого сел на метрику текста. Свойство НЕ наследуется.
            "text-box-trim" => {
                self.text_box_trim_start = matches!(v, "trim-start" | "trim-both");
                self.text_box_trim_end = matches!(v, "trim-end" | "trim-both");
            }
            // `text-box-edge` (css-inline-3 §4.3): по какой метрике срезать.
            // Первое слово — верхний край, второе — нижний; при одном слове
            // второй край берёт то же значение, а если оно ему не подходит —
            // `text` (спека: «else 'text' is assumed as the missing value»).
            // `auto` = `text` (начальное `line-fit-edge: leading` читается
            // как `text`).
            "text-box-edge" => {
                self.text_box_edge_set = true;
                let mut it = v.split_ascii_whitespace();
                let over = it.next().unwrap_or("auto");
                let under = it.next().unwrap_or(over);
                self.text_box_over = match over {
                    "cap" => TextEdge::Cap,
                    "ex" => TextEdge::Ex,
                    _ => TextEdge::Text,
                };
                self.text_box_under = match under {
                    "alphabetic" => TextEdge::Alphabetic,
                    _ => TextEdge::Text,
                };
            }
            // Сокращение `text-box` (css-inline-3 §4.1): без `text-box-trim`
            // подразумевается `trim-both` (НЕ начальное значение), без
            // `text-box-edge` — `auto`. `normal` гасит оба.
            "text-box" => {
                if v == "normal" {
                    self.text_box_trim_start = false;
                    self.text_box_trim_end = false;
                    self.text_box_over = TextEdge::Text;
                    self.text_box_under = TextEdge::Text;
                    self.text_box_edge_set = true;
                } else {
                    // Без края сокращение ставит `auto` (§4.1) — явно.
                    self.text_box_over = TextEdge::Text;
                    self.text_box_under = TextEdge::Text;
                    self.text_box_edge_set = true;
                    let trim = v
                        .split_ascii_whitespace()
                        .find(|w| w.starts_with("trim-"))
                        .unwrap_or("trim-both");
                    self.text_box_trim_start = matches!(trim, "trim-start" | "trim-both");
                    self.text_box_trim_end = matches!(trim, "trim-end" | "trim-both");
                    let edge: String = v
                        .split_ascii_whitespace()
                        .filter(|w| !w.starts_with("trim-"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !edge.is_empty() {
                        self.apply_one("text-box-edge", &edge);
                    }
                }
            }
            "hanging-punctuation" => {
                let mut h = Hanging::default();
                for word in v.split_ascii_whitespace() {
                    match word {
                        "first" => h.first = true,
                        "last" => h.last = true,
                        "force-end" => h.force_end = true,
                        "allow-end" => h.allow_end = true,
                        _ => {}
                    }
                }
                self.hanging = (h != Hanging::default()).then_some(h);
            }
            // Разрыв ВНУТРИ слова разрешают по-разному, и разница видна на
            // экране. `word-break: break-all` и `line-break: anywhere` рвут
            // слово всегда. А `overflow-wrap` — только когда слово иначе не
            // влезает: короткое сначала целиком уходит на следующую строку.
            "text-autospace" => {
                // `normal` = оба разряда, `no-autospace` = ни одного.
                // Разряды могут стоять и по отдельности, и вместе.
                let (alpha, numeric) = match v {
                    "no-autospace" => (false, false),
                    "normal" | "auto" => (true, true),
                    other => (
                        other.contains("ideograph-alpha"),
                        other.contains("ideograph-numeric"),
                    ),
                };
                self.autospace_alpha = Some(alpha);
                self.autospace_numeric = Some(numeric);
            }
            "word-space-transform" => {
                // Точка переноса показывается пробелом: обычным или
                // идеографическим (css-text-4). `none` и `auto-phrase`
                // не показывают ничего.
                // Значение из ДВУХ слов (`ideographic-space auto-phrase`,
                // css-text-4 §word-space-transform) сравнением целиком не
                // ловилось и падало в `none`. Ключевое слово ищем среди
                // разделённых пробелом кусков.
                // Явное `none` хранится нулевым знаком, а не `None`: свойство
                // наследуемое, и `wbr { word-space-transform: none }` внутри
                // `space` обязано отменить замену у своей точки переноса
                // (`word-space-transform-004`), а `None` значит «как у
                // родителя».
                self.word_space_char = Some(
                    v.split_whitespace()
                        .find_map(|w| match w {
                            "space" => Some(' '),
                            "ideographic-space" => Some('\u{3000}'),
                            _ => None,
                        })
                        .unwrap_or('\0'),
                );
            }
            "overflow-wrap" | "word-wrap" => {
                self.break_word = Some(matches!(v, "break-word" | "anywhere"));
                // `anywhere` отличается от `break-word` ровно одним: он МЕНЯЕТ
                // размер по минимальному содержимому — слово рвётся и при его
                // подсчёте. `break-word` на этот размер не влияет
                // (css-text-3 §5.5), и на этой разнице построено целое
                // семейство тестов.
                self.wrap_anywhere = Some(v == "anywhere");
            }
            "word-break" => {
                // `break-word` — устаревший псевдоним, и по спецификации он
                // равен `overflow-wrap: anywhere`, а не `break-word`: разница
                // в том, что `anywhere` УЧИТЫВАЕТСЯ в размере по минимальному
                // содержимому. Пока стоял `break_word`, ячейка с
                // `max-width: 0` не сжималась до знака (`word-break-min-content-001`).
                if v == "break-word" {
                    self.break_word = Some(true);
                    self.wrap_anywhere = Some(true);
                    self.break_anywhere = Some(false);
                    self.keep_all = Some(false);
                } else {
                    self.break_anywhere = Some(v == "break-all");
                    self.keep_all = Some(v == "keep-all");
                }
            }
            "line-break" => {
                self.break_anywhere = Some(v == "anywhere");
                self.break_anywhere_strict = Some(v == "anywhere");
                // `auto` у Blink ведёт себя строго (`LineBreakStrictness::
                // kDefault`), поэтому послабления — только у явных
                // `normal`/`loose`; `line-break-normal-011`, `-loose-*`.
                self.line_break_loose = Some(match v {
                    "normal" => 1,
                    "loose" => 2,
                    _ => 0,
                });
            }
            "hyphenate-character" => {
                // Значение — строка в кавычках; `auto` значит «сам знак
                // переноса».
                self.hyphen_char = Some(if v == "auto" {
                    "\u{2010}".to_string()
                } else {
                    v.trim_matches(['"', '\'']).to_string()
                });
            }
            "text-fit" => {
                self.text_fit = (v != "none").then(|| {
                    let mut f = TextFit {
                        grow: false,
                        shrink: false,
                        per_line: false,
                        all: false,
                        target: None,
                    };
                    for word in v.split_whitespace() {
                        match word {
                            "grow" => f.grow = true,
                            "shrink" => f.shrink = true,
                            "consistent" => f.per_line = false,
                            "per-line" => f.per_line = true,
                            "per-line-all" => {
                                f.per_line = true;
                                f.all = true;
                            }
                            other => {
                                if let Some(pct) = other.strip_suffix('%')
                                    && let Ok(n) = pct.parse::<f32>()
                                {
                                    f.target = Some(n / 100.0);
                                }
                            }
                        }
                    }
                    f
                });
            }
            "text-wrap" | "text-wrap-mode" | "text-wrap-style" => {
                if matches!(v, "nowrap" | "wrap") {
                    self.nowrap = Some(v == "nowrap");
                }
                // `balance` выравнивает длины строк абзаца: последняя строка
                // не должна оставаться коротким огрызком.
                self.balance_lines = Some(v == "balance");
            }
            _ => self.apply_text_vertical_clamp(key, val, v, hit),
        }
    }
}
