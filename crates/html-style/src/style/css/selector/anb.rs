//! Формула An+B в AST селектора: разбирается один раз, проверяется целочисленно.
//! css-syntax-3 §6: учитываются только положительные индексы при n >= 0.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnB {
    a: i64,
    b: i64,
}

impl AnB {
    pub fn parse(arg: &str) -> Option<Self> {
        let arg = arg.trim().to_ascii_lowercase();
        let (a, b) = match arg.as_str() {
            "odd" => (2i64, 1i64),
            "even" => (2, 0),
            _ => match arg.split_once('n') {
                None => match arg.parse::<i64>() {
                    Ok(b) => (0, b),
                    Err(_) => return None,
                },
                Some((head, tail)) => {
                    // Пробел между множителем и `n` запрещён (css-syntax
                    // §the-anb-type): `1 n` — не An+B.
                    if head.ends_with(char::is_whitespace) {
                        return None;
                    }
                    let a = match head {
                        "" | "+" => 1,
                        "-" => -1,
                        other => match other.parse::<i64>() {
                            Ok(v) => v,
                            Err(_) => return None,
                        },
                    };
                    // Сдвиг после `n` обязан нести явный знак: `2n 1` — не An+B,
                    // пробелы допустимы только вокруг самого знака.
                    let tail = tail.trim();
                    let b = if tail.is_empty() {
                        0
                    } else {
                        let (sign, num) = match tail.strip_prefix('+') {
                            Some(rest) => (1i64, rest),
                            None => match tail.strip_prefix('-') {
                                Some(rest) => (-1, rest),
                                None => return None,
                            },
                        };
                        let num = num.trim_start();
                        // Второй знак у сдвига (`2n--1`) — не число.
                        if !num.bytes().all(|c| c.is_ascii_digit()) {
                            return None;
                        }
                        match num.parse::<i64>() {
                            Ok(v) => sign * v,
                            Err(_) => return None,
                        }
                    };
                    (a, b)
                }
            },
        };
        Some(Self { a, b })
    }

    pub fn matches(self, index: usize) -> bool {
        if index == 0 {
            return false;
        }
        // Разность и деление не переполняются даже при i64::MIN и usize::MAX.
        let (a, b, index) = (i128::from(self.a), i128::from(self.b), index as i128);
        if a == 0 {
            return index == b;
        }
        let diff = index - b;
        diff % a == 0 && diff / a >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_progressions_and_css_whitespace() {
        for (text, expected) in [
            ("ODD", vec![1, 3, 5]),
            ("even", vec![2, 4, 6]),
            ("-2n + 5", vec![1, 3, 5]),
            ("0n+3", vec![3]),
            ("n - 2", vec![1, 2, 3, 4, 5, 6]),
            ("+4", vec![4]),
        ] {
            let anb = AnB::parse(text).unwrap();
            assert!(!anb.matches(0));
            assert_eq!(
                (1..=6).filter(|&i| anb.matches(i)).collect::<Vec<_>>(),
                expected
            );
        }
        for text in ["1 n", "+ 2n", "2n 1", "2n--1", "n + -1", "2n+", "1.5n"] {
            assert_eq!(AnB::parse(text), None, "{text}");
        }
    }

    #[test]
    fn extreme_coefficients_do_not_overflow_matching() {
        assert!(AnB::parse("n-9223372036854775807").unwrap().matches(1));
        assert!(!AnB::parse("-9223372036854775808n+1").unwrap().matches(2));
        assert!(AnB::parse("n").unwrap().matches(usize::MAX));
    }

    #[test]
    fn prepared_formulas_match_previous_matcher() {
        use crate::style::css::Selector;
        let mut formulas = [
            "ODD", "even", " n ", "-n", "+n", "+4", "1 n", "+ 2n", "2n 1", "2n--1", "n + -1",
            "2n+", "1.5n", "", "0", "-7",
        ]
        .map(str::to_owned)
        .to_vec();
        for a in -12..=12 {
            for b in -15i32..=15 {
                formulas.push(format!("{a}n{b:+}"));
                formulas.push(format!(
                    " {a}n {} {} ",
                    if b < 0 { '-' } else { '+' },
                    b.abs()
                ));
            }
        }
        for formula in formulas {
            for name in [
                "nth-child",
                "nth-last-child",
                "nth-of-type",
                "nth-last-of-type",
            ] {
                let selector = Selector::parse(&format!(":{name}({formula})")).unwrap();
                let prepared = selector
                    .prepared(selector.pseudo.as_deref().unwrap())
                    .unwrap();
                for index in 1..=128 {
                    assert_eq!(
                        prepared.anb.is_some_and(|anb| anb.matches(index)),
                        old_matcher(&formula, index),
                        "{name}({formula}), {index}"
                    );
                }
            }
        }
    }

    // Прежний production matcher до подготовки An+B: oracle для дифференциальной проверки.
    fn old_matcher(arg: &str, index: usize) -> bool {
        let arg = arg.trim().to_ascii_lowercase();
        let (a, b) = match arg.as_str() {
            "odd" => (2i64, 1i64),
            "even" => (2, 0),
            _ => match arg.split_once('n') {
                None => match arg.parse::<i64>() {
                    Ok(b) => (0, b),
                    Err(_) => return false,
                },
                Some((head, tail)) => {
                    // Пробел между множителем и `n` запрещён (css-syntax
                    // §the-anb-type): `1 n` — не An+B.
                    if head.ends_with(char::is_whitespace) {
                        return false;
                    }
                    let a = match head {
                        "" | "+" => 1,
                        "-" => -1,
                        other => match other.parse::<i64>() {
                            Ok(v) => v,
                            Err(_) => return false,
                        },
                    };
                    // Сдвиг после `n` обязан нести явный знак: `2n 1` — не An+B,
                    // пробелы допустимы только вокруг самого знака.
                    let tail = tail.trim();
                    let b = if tail.is_empty() {
                        0
                    } else {
                        let (sign, num) = match tail.strip_prefix('+') {
                            Some(rest) => (1i64, rest),
                            None => match tail.strip_prefix('-') {
                                Some(rest) => (-1, rest),
                                None => return false,
                            },
                        };
                        let num = num.trim_start();
                        // Второй знак у сдвига (`2n--1`) — не число.
                        if !num.bytes().all(|c| c.is_ascii_digit()) {
                            return false;
                        }
                        match num.parse::<i64>() {
                            Ok(v) => sign * v,
                            Err(_) => return false,
                        }
                    };
                    (a, b)
                }
            },
        };
        let index = index as i64;
        if a == 0 {
            return index == b;
        }
        let diff = index - b;
        diff % a == 0 && diff / a >= 0
    }
}
