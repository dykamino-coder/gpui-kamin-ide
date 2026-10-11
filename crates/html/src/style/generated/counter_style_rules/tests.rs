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
            assert_eq!(custom_repr(-2, "a").unwrap(), "\u{2021}");
            assert_eq!(custom_repr(-1, "b").unwrap(), "-*I");
            assert_eq!(custom_repr(0, "b").unwrap(), "0");
            assert_eq!(custom_repr(6, "b").unwrap(), "6");
            assert_eq!(custom_repr(1, "c").unwrap(), "1");
            assert_eq!(custom_repr(5, "c").unwrap(), "ab");
            assert_eq!(custom_repr(-4, "d").unwrap(), "-11");
        },
    );
}
