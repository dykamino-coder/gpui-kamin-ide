//! Структурные псевдоклассы: :first/last/only-*, :nth-*(An+B [of S]).

use super::*;

/// Псевдокласс — of-форма `:nth-child(… of S)`?
pub(super) fn nth_of_form(pseudo: &str) -> bool {
    let Some((name, arg)) = pseudo.split_once('(') else {
        return false;
    };
    matches!(name, "nth-child" | "nth-last-child")
        && arg
            .strip_suffix(')')
            .is_some_and(|a| crate::style::css::nth_of_parts(a).is_some())
}

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` (селекторы-4):
/// узел обязан сам совпасть с S, а номер считается только среди совпавших
/// братьев — с начала либо с конца. `None` — псевдокласс не of-формы.
pub(super) fn nth_of_holds(
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Option<bool> {
    let (name, arg) = pseudo.split_once('(')?;
    let backwards = match name {
        "nth-child" => false,
        "nth-last-child" => true,
        _ => return None,
    };
    let arg = arg.strip_suffix(')')?;
    let (anb, list) = crate::style::css::nth_of_parts(arg)?;
    let hit = |a: &Ancestor, s: Sibs| list.iter().any(|sel| matches(sel, a, path, s));
    if !sibs.is_elem || list.is_empty() || !hit(me, sibs) {
        return Some(false);
    }
    let (peers, base) = if backwards {
        (sibs.next(), sibs.pos + 1)
    } else {
        (sibs.prev(), 0)
    };
    let idx = 1 + peers
        .iter()
        .enumerate()
        .filter(|(i, a)| hit(a, sibs.at(base + i)))
        .count();
    Some(nth_matches(&anb, idx))
}

/// Структурные псевдоклассы: место узла среди соседей.
///
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
pub(super) fn structural(pseudo: &str, spot: Spot) -> Option<bool> {
    let (name, arg) = match pseudo.split_once('(') {
        Some((n, rest)) => (n, rest.trim_end_matches(')').trim()),
        None => (pseudo, ""),
    };
    let (index, total) = match name {
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-last-child" => {
            (spot.index, spot.total)
        }
        "first-of-type" | "last-of-type" | "only-of-type" | "nth-of-type" | "nth-last-of-type" => {
            (spot.of_type, spot.of_type_total)
        }
        _ => return None,
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return Some(false);
    }
    Some(match name {
        "first-child" | "first-of-type" => index == 1,
        "last-child" | "last-of-type" => index == total,
        "only-child" | "only-of-type" => total == 1,
        "nth-child" | "nth-of-type" => nth_matches(arg, index),
        "nth-last-child" | "nth-last-of-type" => nth_matches(arg, total + 1 - index),
        _ => false,
    })
}

/// Запись `an+b` из `:nth-child()`: подходит ли номер.
fn nth_matches(arg: &str, index: usize) -> bool {
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
