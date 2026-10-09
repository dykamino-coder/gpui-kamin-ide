//! @supports: условия, селекторы, трёхзначная логика.

use crate::style::css::*;

/// Выполнено ли условие `@supports` (css-conditional-3 §4).
///
/// Трёхзначная логика: неизвестная конструкция (`general-enclosed`) — не
/// ложь и не истина, а «неизвестно»; на верхнем уровне неизвестное и
/// невалидное равнозначны лжи. Поддержка декларации проверяется ОРАКУЛОМ:
/// разобранное объявление применяется к чистому стилю — изменился, значит
/// свойство и значение наши (той же механикой живёт реестр покрытия).
pub(super) fn supports(condition: &str) -> bool {
    matches!(supports_condition(condition.trim()), Some(SupTri::True))
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SupTri {
    True,
    False,
    Unknown,
}

pub(super) fn sup_not(t: SupTri) -> SupTri {
    match t {
        SupTri::True => SupTri::False,
        SupTri::False => SupTri::True,
        SupTri::Unknown => SupTri::Unknown,
    }
}

/// `not <терм>` | `<терм> (and <терм>)*` | `<терм> (or <терм>)*` — уровни
/// не смешиваются: `a and b or c` недействительно целиком.
fn supports_condition(s: &str) -> Option<SupTri> {
    let s = s.trim();
    // `not` — слово: слитное `not(` лексится функцией и уходит в терм.
    if let Some(rest) = s.strip_prefix("not")
        && rest.starts_with(char::is_whitespace)
    {
        let rest = rest.trim_start();
        let (term, tail) = supports_take_term(rest)?;
        if !tail.trim().is_empty() {
            return None;
        }
        return Some(sup_not(supports_eval_term(term)));
    }
    let (term, mut rest) = supports_take_term(s)?;
    let mut acc = supports_eval_term(term);
    let mut op: Option<&str> = None;
    loop {
        let r = rest.trim_start();
        if r.is_empty() {
            return Some(acc);
        }
        let word = if let Some(w) = r.strip_prefix("and") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "and"
        } else if let Some(w) = r.strip_prefix("or") {
            if !w.starts_with(char::is_whitespace) {
                return None;
            }
            rest = w;
            "or"
        } else {
            return None;
        };
        if let Some(prev) = op {
            if prev != word {
                return None;
            }
        } else {
            op = Some(word);
        }
        let (term, tail) = supports_take_term(rest.trim_start())?;
        let v = supports_eval_term(term);
        acc = match (word, acc, v) {
            ("and", SupTri::True, SupTri::True) => SupTri::True,
            ("and", SupTri::False, _) | ("and", _, SupTri::False) => SupTri::False,
            ("and", ..) => SupTri::Unknown,
            ("or", SupTri::True, _) | ("or", _, SupTri::True) => SupTri::True,
            ("or", SupTri::False, SupTri::False) => SupTri::False,
            _ => SupTri::Unknown,
        };
        rest = tail;
    }
}

/// Один терм: скобочная группа либо функция `имя(...)`; возврат — тело
/// терма (со скобками функции внутри среза) и хвост после него.
fn supports_take_term(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    // Функция: идентификатор вплотную к скобке.
    let mut name_end = 0;
    while name_end < bytes.len()
        && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'-')
    {
        name_end += 1;
    }
    let open = if name_end < bytes.len() && bytes[name_end] == b'(' {
        name_end
    } else if bytes.first() == Some(&b'(') {
        0
    } else {
        return None;
    };
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..i + 1], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// `selector(<complex-selector>)` (css-conditional-4 §at-supports-ext):
/// ОДИН сложный селектор — список через запятую ложен (`at-supports-selector-004`);
/// внутри `:is()/:where()/:has()/:not()` прощающего разбора при проверке нет —
/// неизвестная часть роняет всё (`…-detecting-invalid-in-logical-combinations`).
/// Псевдоэлементы, которые Blink знает, а наш каскад не исполняет
/// (`::details-content`, `::picker(select)`, `::picker-icon`,
/// `::-webkit-slider-thumb`), для ПРОВЕРКИ снимаются; в `known_pseudo` их не
/// вносим — иначе правила с ними начали бы применяться к самой коробке.
fn supports_selector(args: &str) -> bool {
    let s = args.trim();
    if split_top_level(s, ',').len() > 1 {
        return false;
    }
    let mut s = s.to_string();
    for known in [
        "::details-content",
        "::picker(select)",
        "::picker-icon",
        "::-webkit-slider-thumb",
        "::-webkit-slider-runnable-track",
    ] {
        s = s.replace(known, "");
    }
    // Неизвестные вендорные псевдо — не поддержаны (`::-webkit-asdf`).
    if s.contains("::-webkit-") || s.contains(":-webkit-") {
        return false;
    }
    // Снятый псевдоэлемент мог стоять один: `::picker-icon` → пусто.
    if s.trim().is_empty() || s.ends_with(|ch: char| ch.is_whitespace() || "+>~".contains(ch)) {
        s.push('*');
    }
    selector_strict(&s)
}

/// Селектор годен, и годна КАЖДАЯ часть списков внутри `:is()` и родни.
fn selector_strict(s: &str) -> bool {
    if Selector::parse(s).is_none() {
        return false;
    }
    for f in [":is(", ":where(", ":has(", ":not(", ":matches(", ":any("] {
        let mut from = 0usize;
        while let Some(at) = s[from..].find(f) {
            let open = from + at + f.len();
            let mut depth = 1i32;
            let mut close = None;
            for (i, ch) in s[open..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { return false };
            for part in split_top_level(&s[open..close], ',') {
                let mut p = part.trim();
                // Относительный селектор `:has(> .a)`: ведущий комбинатор.
                if f == ":has(" {
                    p = p.trim_start_matches(['>', '+', '~']).trim_start();
                }
                if p.is_empty() || !selector_strict(p) {
                    return false;
                }
            }
            from = close;
        }
    }
    true
}

fn supports_eval_term(term: &str) -> SupTri {
    let term = term.trim();
    // Функция `имя(...)`.
    if !term.starts_with('(') {
        let Some(open) = term.find('(') else {
            return SupTri::Unknown;
        };
        let name = term[..open].to_ascii_lowercase();
        let args = &term[open + 1..term.len().saturating_sub(1)];
        return match name.as_str() {
            "selector" => {
                if supports_selector(args) {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            "font-format" => {
                let f = args.trim().to_ascii_lowercase();
                if matches!(f.as_str(), "woff" | "woff2" | "truetype" | "opentype") {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            // `font-tech(<font-tech>)` — ровно ОДНО слово (css-conditional-5
            // §font-tech): `features-opentype color-COLRv1` и список через
            // запятую — ложь (`at-supports-font-tech-001`). Технологии —
            // то, что открывает DirectWrite; `incremental` — нет.
            "font-tech" => {
                let t = args.trim().to_ascii_lowercase();
                if matches!(
                    t.as_str(),
                    "features-opentype"
                        | "features-aat"
                        | "color-colrv0"
                        | "color-colrv1"
                        | "color-sbix"
                        | "color-cbdt"
                        | "variations"
                        | "palettes"
                ) {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
            "at-rule" => SupTri::False,
            // `not(...)`/`or(...)` и прочие неизвестные функции — это
            // `<general-enclosed>`, а css-conditional-3 §4 говорит о нём
            // дословно: «The result is false». Не «неизвестно»: иначе
            // `not unknown()` остаётся неизвестным и на верхнем уровне
            // ложным, тогда как обязан быть ИСТИНОЙ (`at-supports-046`).
            _ => SupTri::False,
        };
    }
    let inner = &term[1..term.len() - 1];
    // Скобки вокруг условия.
    if let Some(t) = supports_condition(inner) {
        return t;
    }
    // Точка с запятой внутри скобок: `<declaration>` её не содержит
    // (css-syntax-3 §5.4.4 — `<declaration-value>` не берёт `;` верхнего
    // уровня), значит `(margin: 0;)` — не объявление, а `<general-enclosed>`,
    // то есть ЛОЖЬ (`at-supports-038/039`).
    if split_top_level(inner, ';').len() > 1 {
        return SupTri::False;
    }
    // Декларация: непустой разбор + дельта на чистом стиле.
    let colons = split_top_level(inner, ':');
    if colons.len() >= 2 {
        // Пользовательское свойство поддержано всегда, если объявление
        // разобралось (css-variables-1 §2: значением `--*` служит любой
        // годный `<declaration-value>`). Оракул «дельта на чистом стиле» его
        // не видит: `--foo` не пишет ни в одно поле (`at-supports-044`).
        if colons[0].trim().to_ascii_lowercase().starts_with("--") {
            return if parse_decls(inner).is_empty() {
                SupTri::False
            } else {
                SupTri::True
            };
        }
        // Второе двоеточие ВЕРХНЕГО уровня у обычного свойства значит, что в
        // значение затесалось чужое объявление: `(margin: 0 or padding: 0)`
        // и `(margin: 0 and padding: 0)` — мусор, а не «margin с довеском»
        // (`at-supports-034..037`: каждое условие берётся в СВОИ скобки).
        if colons.len() > 2 {
            return SupTri::False;
        }
        if !attr_prefixes_declared(colons[1]) {
            return SupTri::False;
        }
        // css-variables-1 §3: «If a property contains one or more var()
        // functions, and those functions are syntactically valid, the entire
        // property's grammar must be assumed to be valid at parse time».
        // Оракул «дельта на чистом стиле» такое не видит: без значения
        // переменной объявление не пишет ни в одно поле (`at-supports-044`,
        // `(color: var(--anything) invalid-value)`).
        if colons[1].to_ascii_lowercase().contains("var(") {
            return if parse_decls(inner).is_empty() {
                SupTri::False
            } else {
                SupTri::True
            };
        }
        let decls = parse_decls(inner);
        if decls.is_empty() {
            // Синтаксис объявления сломан (`!bogus`, `!important !important`,
            // `!important green`) — `<general-enclosed>`, то есть ЛОЖЬ
            // (`css-supports-043/044/045`).
            return SupTri::False;
        }
        // Пометка важности к ПОДДЕРЖКЕ отношения не имеет и обязана быть
        // допустима (css-conditional-3 §4: «Property declarations in an
        // @supports rule can have !important specified»). `parse_decls`
        // оставляет её в значении для каскада — здесь она мешает разобрать
        // само значение (`css-supports-004`, `at-supports-007`).
        let clean: crate::style::css::Decls = decls
            .iter()
            .map(|(k, v)| {
                if k == ORDER_KEY {
                    return (k.clone(), v.clone());
                }
                let parts: Vec<&str> = v
                    .split(DECL_SEP)
                    .map(|part| match top_level_bang(part) {
                        Some(at) => part[..at].trim(),
                        None => part,
                    })
                    .collect();
                (k.clone(), parts.join(&DECL_SEP.to_string()))
            })
            .collect();
        let mut c = crate::style::computed::Computed::default();
        c.apply_decls(&clean);
        // Счётчик порядка объявлений и номера сторон — БУХГАЛТЕРИЯ каскада, а
        // не значения свойств: они меняются у любого объявления, и без
        // обнуления «поддержанным» выходило всё подряд, включая
        // `(color: rainbow)` (`css-supports-005`, `at-supports-009`).
        c.decl_seq = 0;
        c.side_seq = Default::default();
        return if format!("{c:?}") != format!("{:?}", crate::style::computed::Computed::default()) {
            SupTri::True
        } else {
            SupTri::False
        };
    }
    // Скобка без двоеточия и без условия — тоже `<general-enclosed>`: ЛОЖЬ
    // (`css-supports-032/033/034/040`).
    SupTri::False
}
