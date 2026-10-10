//! Вычисление calc(): дерево выражения, сравнение min/max/clamp, разбор.

use super::*;

/// Операнд выражения: голое число участвует только в умножении и делении.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Val {
    Num(f32),
    Len(Sum),
}

/// `expr := term (('+'|'-') term)*`, `term := factor (('*'|'/') factor)*`.
pub(super) struct Calc<'a> {
    pub(super) rest: &'a str,
}

impl<'a> Calc<'a> {
    pub(super) fn eat(&mut self, want: &[char]) -> Option<char> {
        self.rest = self.rest.trim_start();
        let c = self.rest.chars().next()?;
        if !want.contains(&c) {
            return None;
        }
        self.rest = &self.rest[c.len_utf8()..];
        Some(c)
    }

    pub(super) fn expr(&mut self) -> Option<Val> {
        let mut acc = self.term()?;
        while let Some(op) = self.eat(&['+', '-']) {
            let rhs = self.term()?;
            let sign = if op == '-' { -1.0 } else { 1.0 };
            acc = match (acc, rhs) {
                (Val::Num(a), Val::Num(b)) => Val::Num(a + sign * b),
                (Val::Len(a), Val::Len(b)) => Val::Len(a.add(b, sign)),
                // Число плюс длина — запись без смысла, значение теряется.
                _ => return None,
            };
        }
        Some(acc)
    }

    pub(super) fn term(&mut self) -> Option<Val> {
        let mut acc = self.factor()?;
        while let Some(op) = self.eat(&['*', '/']) {
            let rhs = self.factor()?;
            acc = match (op, acc, rhs) {
                ('*', Val::Len(a), Val::Num(k)) | ('*', Val::Num(k), Val::Len(a)) => {
                    Val::Len(a.scaled(k))
                }
                ('*', Val::Num(a), Val::Num(b)) => Val::Num(a * b),
                (_, _, Val::Num(k)) if k == 0.0 => return None,
                ('/', Val::Len(a), Val::Num(k)) => Val::Len(a.scaled(1.0 / k)),
                ('/', Val::Num(a), Val::Num(k)) => Val::Num(a / k),
                // Делить на длину и умножать длину на длину нечем.
                _ => return None,
            };
        }
        Some(acc)
    }

    pub(super) fn factor(&mut self) -> Option<Val> {
        self.rest = self.rest.trim_start();
        // `min()`/`max()`/`clamp()` (css-values-4 §10.2): доводы — полные
        // выражения через запятую, сворачивает их `compare`.
        let rest = self.rest;
        for (name, kind) in [("min(", 0u8), ("max(", 1u8), ("clamp(", 2u8)] {
            if rest
                .get(..name.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(name))
            {
                let mut sub = Calc {
                    rest: &rest[name.len()..],
                };
                let mut args = vec![sub.expr()?];
                while sub.eat(&[',']).is_some() {
                    args.push(sub.expr()?);
                }
                sub.eat(&[')'])?;
                self.rest = sub.rest;
                return compare(kind, &args);
            }
        }
        let inner = self
            .rest
            .strip_prefix('(')
            .map(|r| (r, 1usize))
            .or_else(|| {
                let low = self.rest.get(..5)?;
                low.eq_ignore_ascii_case("calc(")
                    .then(|| (&self.rest[5..], 5usize))
            });
        if let Some((body, open)) = inner {
            let mut sub = Calc { rest: body };
            let val = sub.expr()?;
            sub.eat(&[')'])?;
            let used = self.rest.len() - sub.rest.len();
            debug_assert!(used >= open);
            self.rest = sub.rest;
            return Some(val);
        }
        // Одиночный операнд: знак, число, единица. Разбор единицы отдан
        // `Len::parse`, чтобы правила были ровно одни и те же.
        let end = self
            .rest
            .char_indices()
            .find(|(i, c)| {
                let signed = matches!(c, '+' | '-') && *i == 0;
                !(c.is_ascii_digit() || *c == '.' || c.is_ascii_alphabetic() || *c == '%' || signed)
            })
            .map(|(i, _)| i)
            .unwrap_or(self.rest.len());
        let (head, tail) = self.rest.split_at(end);
        if head.is_empty() {
            return None;
        }
        self.rest = tail;
        if let Ok(n) = head.parse::<f32>() {
            return Some(Val::Num(n));
        }
        Sum::from_len(Len::parse(head)?).map(Val::Len)
    }
}

/// `min()` (0), `max()` (1), `clamp()` (2) над разобранными доводами.
///
/// Сворачивается только однородное (css-values-4 §10.10, упрощение
/// функций сравнения): все доводы — числа, либо все — длины ОДНОЙ природы.
/// Тогда ответ — сам один из доводов, и природа доживает как у `calc()`.
/// Смесь природ (`min(50%, 100px)`) решается только раскладкой — запись, как
/// и смешанный `calc()`, отбрасывается. Число вместе с длиной — несовместимые
/// типы: `min(0, 100%)` недействительно (`max-unitless-zero-invalid`).
fn compare(kind: u8, args: &[Val]) -> Option<Val> {
    if args.is_empty() || (kind == 2 && args.len() != 3) {
        return None;
    }
    let keys: Vec<(u8, f32)> = args
        .iter()
        .map(|a| match a {
            Val::Num(n) => Some((u8::MAX, *n)),
            Val::Len(s) => s.nature(),
        })
        .collect::<Option<_>>()?;
    let nature = keys[0].0;
    if keys.iter().any(|k| k.0 != nature) {
        return None;
    }
    let want = match kind {
        0 => keys.iter().map(|k| k.1).fold(f32::INFINITY, f32::min),
        1 => keys.iter().map(|k| k.1).fold(f32::NEG_INFINITY, f32::max),
        // `clamp(MIN, VAL, MAX)` = `max(MIN, min(VAL, MAX))`: при MIN > MAX
        // побеждает MIN (§10.2).
        _ => keys[1].1.min(keys[2].1).max(keys[0].1),
    };
    let at = keys.iter().position(|k| k.1 == want)?;
    Some(args[at])
}

pub(super) fn eval_calc(inner: &str) -> Option<Sum> {
    let mut calc = Calc { rest: inner };
    let val = calc.expr()?;
    if !calc.rest.trim().is_empty() {
        return None;
    }
    match val {
        Val::Len(sum) => Some(sum),
        // Голое число длиной не станет: `calc(2 * 3)` — не длина.
        Val::Num(_) => None,
    }
}

pub(super) fn parse_calc(inner: &str) -> Option<Len> {
    eval_calc(inner)?.collapse()
}
