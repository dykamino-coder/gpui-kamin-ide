//! Термы @supports: (свойство: значение), selector() со строгим разбором, прочие функции;
//! трёхзначный результат.

use super::*;
use crate::document::state::ValueCx;

/// `selector(<complex-selector>)` (css-conditional-4 §at-supports-ext):
/// ОДИН сложный селектор — список через запятую ложен (`at-supports-selector-004`);
/// внутри `:is()/:where()/:has()/:not()` прощающего разбора при проверке нет —
/// неизвестная часть роняет всё (`…-detecting-invalid-in-logical-combinations`).
/// Capabilities come from the actual selector parser and nesting normalization.
fn supports_selector(args: &str, cx: &SelectorCx) -> bool {
    let s = args.trim();
    if split_top_level(s, ',').len() > 1 {
        return false;
    }
    let s = super::super::sheet::selector_for_supports(s, cx);
    selector_strict(&s, cx)
}

/// Селектор годен, и годна КАЖДАЯ часть списков внутри `:is()` и родни.
fn selector_strict(s: &str, cx: &SelectorCx) -> bool {
    if Selector::parse_in(s, cx).is_none() {
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
                if p.is_empty() || !selector_strict(p, cx) {
                    return false;
                }
            }
            from = close;
        }
    }
    true
}

pub(super) fn supports_eval_term(term: &str, cx: &SelectorCx, vcx: ValueCx<'_>) -> SupTri {
    let term = term.trim();
    // Функция `имя(...)`.
    if !term.starts_with('(') {
        let Some(open) = term.find('(') else {
            return SupTri::Unknown;
        };
        let name = unescape(&term[..open]).to_ascii_lowercase();
        let args = &term[open + 1..term.len().saturating_sub(1)];
        return match name.as_str() {
            "selector" => {
                if supports_selector(args, cx) {
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
            "at-rule" => {
                if at_rule::supported(args) {
                    SupTri::True
                } else {
                    SupTri::False
                }
            }
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
    if let Some(t) = supports_condition(inner, cx, vcx) {
        return t;
    }
    // Точка с запятой внутри скобок: `<declaration>` её не содержит
    // (css-syntax-3 §5.4.4 — `<declaration-value>` не берёт `;` верхнего
    // уровня), значит `(margin: 0;)` — не объявление, а `<general-enclosed>`,
    // то есть ЛОЖЬ (`at-supports-038/039`).
    if split_top_level(inner, ';').len() > 1 {
        return SupTri::False;
    }
    // Декларация: синтаксис объявления и результат разбора свойства.
    let colons = split_top_level(inner, ':');
    if colons.len() >= 2 {
        // Пользовательское свойство поддержано всегда, если объявление
        // разобралось (css-variables-1 §2: значением `--*` служит любой
        // годный `<declaration-value>`). Оракул «дельта на чистом стиле» его
        // не видит: `--foo` не пишет ни в одно поле (`at-supports-044`).
        if colons[0].trim().to_ascii_lowercase().starts_with("--") {
            return if parse_decls_in(inner, vcx).is_empty() {
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
        if !attr_prefixes_declared(colons[1], cx) {
            return SupTri::False;
        }
        // CSS Conditional 3 §6.1: substitution cannot add an unknown property.
        let property = unescape(colons[0].trim()).to_ascii_lowercase();
        if !crate::style::computed::Computed::recognizes_property(&property, vcx) {
            return SupTri::False;
        }
        // css-variables-1 §3: «If a property contains one or more var()
        // functions, and those functions are syntactically valid, the entire
        // property's grammar must be assumed to be valid at parse time».
        // Оракул «дельта на чистом стиле» такое не видит: без значения
        // переменной объявление не пишет ни в одно поле (`at-supports-044`,
        // `(color: var(--anything) invalid-value)`).
        if colons[1].to_ascii_lowercase().contains("var(") {
            return if parse_decls_in(inner, vcx).is_empty() {
                SupTri::False
            } else {
                SupTri::True
            };
        }
        let decls = parse_decls_in(inner, vcx);
        if decls.is_empty() {
            // Синтаксис объявления сломан (`!bogus`, `!important !important`,
            // `!important green`) — `<general-enclosed>`, то есть ЛОЖЬ
            // (`css-supports-043/044/045`).
            return SupTri::False;
        }
        // Пометка важности к ПОДДЕРЖКЕ отношения не имеет и обязана быть
        // допустима (css-conditional-3 §4: «Property declarations in an
        // @supports rule can have !important specified»): значение
        // объявления хранится без неё (`css-supports-004`, `at-supports-007`).
        // CSS Conditional 3 §6.1 tests whether the declaration is accepted,
        // independently of its resulting value or cascade bookkeeping.
        let accepted = decls.iter().any(|d| {
            crate::style::computed::Computed::declaration_result(&d.name, &d.value, vcx)
                == crate::style::computed::ParseResult::Accepted
        });
        return if accepted {
            SupTri::True
        } else {
            SupTri::False
        };
    }
    // Скобка без двоеточия и без условия — тоже `<general-enclosed>`: ЛОЖЬ
    // (`css-supports-032/033/034/040`).
    SupTri::False
}
