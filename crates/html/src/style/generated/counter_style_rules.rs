//! Правила `@counter-style` документа (css-counter-styles-3 §3).
//!
//! Разбор стилевых листов кладёт сюда дескрипторы каждого действительного
//! правила (`register`), разбор следующего документа реестр чистит
//! (`reset`). Представление значения строится по алгоритму спеки
//! §counter-style-generate: диапазон → резерв, начальное представление по
//! системе, дополнение `pad`, знак `negative`. Предопределённые стили
//! описаны в `counter_style::builtin`; без правил документа путь
//! предопределённых стилей не меняется вовсе.

use crate::style::generated::counter_style::{
    Context, builtin, builtin_initial, builtin_repr_in, normalize_name,
};
use std::collections::HashMap;
use std::sync::Mutex;

mod generate;
mod math;
mod parse;
mod resolve;
mod tokens;
use generate::anonymous;
pub use generate::{custom_marker, custom_repr, valid_symbols_fn};
use math::{Math, is_math_fn};
pub use parse::register;
use resolve::{Algo, in_range, resolve_counter_style, uses_negative};
use tokens::{Tok, tokenize};

#[derive(Clone, Debug, PartialEq)]
enum System {
    Cyclic,
    Numeric,
    Alphabetic,
    Symbolic,
    Additive,
    Fixed(i64),
    Extends(String),
}

/// Дескрипторы одного правила, как они заданы (незаданное — `None`).
#[derive(Clone, Debug, Default)]
struct Raw {
    system: Option<System>,
    symbols: Option<Vec<String>>,
    additive: Option<Vec<(u64, String)>>,
    negative: Option<(String, String)>,
    prefix: Option<String>,
    suffix: Option<String>,
    /// `Some(None)` — `auto`.
    range: Option<Option<Vec<(i64, i64)>>>,
    pad: Option<(usize, String)>,
    fallback: Option<String>,
}

static RULES: Mutex<Option<HashMap<String, Raw>>> = Mutex::new(None);

pub fn reset() {
    RULES.lock().unwrap().take();
}

#[cfg(test)]
mod tests {
    //! Тесты правил @counter-style: разбор дескрипторов, системы, запасные стили.

    use super::*;

    fn with(css: &[(&str, &[(&str, &str)])], f: impl FnOnce()) {
        reset();
        for (name, descs) in css {
            let d: Vec<(String, Vec<String>)> = descs
                .iter()
                .map(|(k, v)| (k.to_string(), vec![v.to_string()]))
                .collect();
            register(name, &d);
        }
        f();
        reset();
    }

    #[test]
    fn systems_follow_spec() {
        with(
            &[
                ("a", &[("system", "cyclic"), ("symbols", "\\2020  \\2021")]),
                (
                    "b",
                    &[
                        ("system", "extends upper-roman"),
                        ("range", "infinite 5"),
                        ("pad", "3 '*'"),
                    ],
                ),
                (
                    "c",
                    &[
                        ("system", "additive"),
                        ("additive-symbols", "3 \"a\", 2 \"b\""),
                    ],
                ),
                ("d", &[("system", "numeric"), ("symbols", "'0' '1' '2'")]),
            ],
            || {
                // css-counter-styles-3 §cyclic: index `(value - 1) mod N` with a
                // non-negative modulo, so -2 with two symbols is index 1.
                assert_eq!(
                    custom_repr(-2, "a", Context::default()).unwrap(),
                    "\u{2021}"
                );
                assert_eq!(custom_repr(-1, "b", Context::default()).unwrap(), "-*I");
                assert_eq!(custom_repr(0, "b", Context::default()).unwrap(), "0");
                assert_eq!(custom_repr(6, "b", Context::default()).unwrap(), "6");
                assert_eq!(custom_repr(1, "c", Context::default()).unwrap(), "1");
                assert_eq!(custom_repr(5, "c", Context::default()).unwrap(), "ab");
                assert_eq!(custom_repr(-4, "d", Context::default()).unwrap(), "-11");
            },
        );
    }
}
