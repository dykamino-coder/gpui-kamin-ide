//! Computed::apply_one: color, line-height, text-align/indent/transform/wrap, white-space, word-*, hyphens, vertical-align, line-clamp, text-box*, writing modes.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

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
                    "auto" | "inter-word" | "inter-character" | "distribute" | "ruby" => Some(false),
                    _ => self.no_justify,
                };
            }
            "text-align" => {
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
                if v == "justify-all" {
                    self.text_align_last = Some(TextAlign::Justify);
                }
            }
            "text-align-last" => {
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
            "text-overflow" => {
                self.text_overflow_inherit = v.trim().eq_ignore_ascii_case("inherit");
                if self.text_overflow_inherit {
                    return;
                }
                // css-overflow-4 §5: clip | ellipsis | <строка>, до двух
                // сторон. Наша обрезка — конец строки: берётся последнее
                // не-clip значение.
                let mut on = false;
                let mut marker = None;
                // Резка по пробелам ВНЕ кавычек: маркер-строка может
                // нести пробел; метка '\u{0}' отличает строку от ключевого слова.
                let mut tokens: Vec<String> = vec![];
                let mut cur = String::new();
                let mut quote: Option<char> = None;
                for ch in v.chars() {
                    match quote {
                        Some(q) if ch == q => quote = None,
                        Some(_) => cur.push(ch),
                        None if ch == '"' || ch == '\'' => {
                            quote = Some(ch);
                            if cur.is_empty() {
                                cur.push('\u{0}');
                            }
                        }
                        None if ch.is_whitespace() => {
                            if !cur.is_empty() {
                                tokens.push(std::mem::take(&mut cur));
                            }
                        }
                        None => cur.push(ch),
                    }
                }
                if !cur.is_empty() {
                    tokens.push(cur);
                }
                for t in tokens {
                    if let Some(text) = t.strip_prefix('\u{0}') {
                        on = true;
                        // Строка объявления несёт экранирование (css-syntax
                        // §4.3.7: `\0A` — перевод строки, `\2026` — многоточие),
                        // а разрывы сегмента в ней преобразуются, как в тексте
                        // (css-text-3 §4.1.2; Blink `line_truncator.cc`
                        // `SuppressLineBreaks`): ряд принудительных разрывов —
                        // один пробел (`text-overflow-string-009…016`).
                        marker = Some(collapse_forced_breaks(&unescape_content(text)));
                    } else if t == "ellipsis" {
                        on = true;
                        marker = None;
                    }
                }
                self.ellipsis = Some(on);
                self.overflow_marker = marker;
            }
            // Долгая запись `white-space-collapse` (css-text-4 §3.1) трогает
            // ТОЛЬКО схлопывание; перенос (`nowrap`) остаётся за
            // `text-wrap-mode`. У `preserve-spaces` и `discard` своего пути в
            // сборке текста нет — не трогаем.
            "white-space-collapse" => match v {
                "collapse" => {
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(false);
                    self.break_after_spaces = Some(false);
                }
                "preserve" => {
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "preserve-breaks" => {
                    self.keep_spaces = Some(false);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(false);
                }
                "break-spaces" => {
                    self.keep_spaces = Some(true);
                    self.preserve_newlines = Some(true);
                    self.break_after_spaces = Some(true);
                }
                _ => {}
            },
            "white-space" => white_space::apply(self, v),
            "word-spacing" => self.word_spacing = Len::parse_spacing(v),
            "text-transform" => {
                // Свойство наследуемое: `inherit` очищает свой слот, иначе
                // прежнее объявление того же правила его переживало.
                if v == "inherit" {
                    self.text_transform = None;
                    self.text_transform_flags = 0;
                    return;
                }
                // Значений бывает несколько сразу (`uppercase full-width`):
                // регистр и добавки складываются, а не вытесняют друг друга —
                // прежде действовало только последнее слово
                // (`text-transform-multiple-001`).
                let mut case: Option<TextTransform> = None;
                let mut flags = 0u8;
                let mut any = false;
                for word in v.split_ascii_whitespace() {
                    match word.to_ascii_lowercase().as_str() {
                        "uppercase" => case = Some(TextTransform::Upper),
                        "lowercase" => case = Some(TextTransform::Lower),
                        "capitalize" => case = Some(TextTransform::Capitalize),
                        "full-width" | "fullwidth" => flags |= TT_FULL_WIDTH,
                        "full-size-kana" => flags |= TT_KANA,
                        "math-auto" => flags |= TT_MATH,
                        "none" => {}
                        _ => continue,
                    }
                    any = true;
                }
                if any {
                    self.text_transform = Some(case.unwrap_or(TextTransform::None));
                    self.text_transform_flags = flags;
                }
            }
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
                                if let Some(pct) = other.strip_suffix('%') {
                                    if let Ok(n) = pct.parse::<f32>() {
                                        f.target = Some(n / 100.0);
                                    }
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
                    Some(collapse_forced_breaks(&unescape_content(&t[1..t.len() - 1])))
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
