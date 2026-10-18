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
    pub(super) recording: Option<Vec<super::calc_expression::Node>>,
    pub(super) rest: &'a str,
}

impl<'a> Calc<'a> {
    pub(super) fn new(rest: &'a str) -> Self {
        Self {
            rest,
            recording: None,
        }
    }

    pub(super) fn sub(&mut self, rest: &'a str) -> Self {
        Self {
            rest,
            recording: self.recording.take(),
        }
    }

    pub(super) fn resume(&mut self, sub: Self) {
        self.rest = sub.rest;
        self.recording = sub.recording;
    }

    pub(super) fn arguments(&mut self, mut accept: impl FnMut(Val) -> Option<()>) -> Option<usize> {
        accept(self.expr()?)?;
        let mut count = 1;
        while self.eat(&[',']).is_some() {
            accept(self.expr()?)?;
            count += 1;
        }
        self.eat(&[')'])?;
        Some(count)
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
            acc = acc.binary(op, rhs)?;
            self.record_binary(op)?;
        }
        Some(acc)
    }

    pub(super) fn term(&mut self) -> Option<Val> {
        let mut acc = self.factor()?;
        while let Some(op) = self.eat(&['*', '/']) {
            let rhs = self.factor()?;
            acc = acc.binary(op, rhs)?;
            self.record_binary(op)?;
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
                let mut sub = self.sub(&rest[name.len()..]);
                let function = super::calc_function::Function::Compare(kind);
                let mut arguments = super::calc_function::Arguments::new(function);
                let count = sub.arguments(|value| arguments.push(value))?;
                sub.record_call(function, count)?;
                self.resume(sub);
                return arguments.finish();
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
            let mut sub = self.sub(body);
            let val = sub.expr()?;
            sub.eat(&[')'])?;
            let used = self.rest.len() - sub.rest.len();
            debug_assert!(used >= open);
            self.resume(sub);
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
            return self.record_value(Val::Num(n));
        }
        if let Some(v) = super::calc_math::constant_or_angle(head) {
            return self.record_value(v);
        }
        self.record_value(Val::Len(super::atom::Atom::parse(head)?.sum()))
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
