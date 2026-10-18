//! Вычисление calc(): дерево выражения, сравнение min/max/clamp, разбор.

use super::*;

/// Операнд выражения: голое число участвует только в умножении и делении.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Val {
    Num(f32),
    /// Угол в радианах: довод и ответ тригонометрии (§10.4).
    Ang(f32),
    Len(Sum),
}

/// `expr := term (('+'|'-') term)*`, `term := factor (('*'|'/') factor)*`.
pub(super) struct Calc<'a> {
    pub(super) rest: &'a str,
}

impl<'a> Calc<'a> {
    pub(super) fn new(rest: &'a str) -> Self {
        Self { rest }
    }

    pub(super) fn arguments(&mut self, mut accept: impl FnMut(Val) -> Option<()>) -> Option<()> {
        accept(self.expr()?)?;
        while self.eat(&[',']).is_some() {
            accept(self.expr()?)?;
        }
        self.eat(&[')'])?;
        Some(())
    }

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
                (Val::Ang(a), Val::Ang(b)) => Val::Ang(a + sign * b),
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
                ('*', Val::Ang(a), Val::Num(k)) | ('*', Val::Num(k), Val::Ang(a)) => {
                    Val::Ang(a * k)
                }
                ('*', Val::Num(a), Val::Num(b)) => Val::Num(a * b),
                // Деление на ноль даёт ±∞ или NaN (css-values-4 §10.9:
                // «division by zero … produces +∞, −∞ or NaN»), а не отказ:
                // `calc(1px / 0)` — бесконечная длина, зажатая потребителем.
                ('/', Val::Len(a), Val::Num(k)) => Val::Len(a.scaled(1.0 / k)),
                ('/', Val::Ang(a), Val::Num(k)) => Val::Ang(a / k),
                ('/', Val::Num(a), Val::Num(k)) => Val::Num(a / k),
                // Делить на длину и умножать длину на длину нечем.
                _ => return None,
            };
        }
        Some(acc)
    }

    pub(super) fn factor(&mut self) -> Option<Val> {
        self.rest = self.rest.trim_start();
        // round()/mod()/sin()/… (§10.3–§10.8) — своя ветвь в `calc_math`.
        if super::calc_math::is_math_fn(self.rest) {
            return self.math_fn();
        }
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
                let mut compare = super::calc_arguments::Comparison::new(kind);
                sub.arguments(|value| compare.push(value))?;
                self.rest = sub.rest;
                return compare.finish();
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
        // Numeric atoms share the canonical unit parser without arena storage.
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
        // Число по грамматике CSS: `head.parse` брал бы и `inf`/`NaN` Rust.
        if let Some(n) = css_number(head) {
            return Some(Val::Num(n));
        }
        if let Some(v) = super::calc_math::constant_or_angle(head) {
            return Some(v);
        }
        super::atom::Atom::parse(head).map(|atom| Val::Len(atom.sum()))
    }
}

pub(super) fn eval_calc(inner: &str) -> Option<Sum> {
    Calc::new(inner).sum()
}

impl Calc<'_> {
    pub(super) fn sum(mut self) -> Option<Sum> {
        let val = self.expr()?;
        if !self.rest.trim().is_empty() {
            return None;
        }
        match val {
            // Верхний уровень: NaN -> 0, ±∞ -> предел (css-values-4 §10.13).
            Val::Len(sum) => Some(sum.censored()),
            // Голое число длиной не станет: `calc(2 * 3)` — не длина.
            Val::Num(_) | Val::Ang(_) => None,
        }
    }
}

pub(super) fn parse_calc(inner: &str) -> Option<Len> {
    eval_calc(inner)?.collapse()
}
