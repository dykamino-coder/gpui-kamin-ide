//! Вычисление математических функций в дескрипторах @counter-style (range/pad и т.п.).

pub(super) fn is_math_fn(w: &str) -> bool {
    matches!(
        w.to_ascii_lowercase().as_str(),
        "calc" | "sign" | "abs" | "min" | "max" | "clamp"
    )
}

/// Значение внутри `calc()`: (число, px, длина ли).
type MathVal = (f64, f64, bool);

/// Вычислитель `calc()` для целых дескрипторов. У дескрипторов правила нет
/// элемента, и `em`/`rem` берутся от начального кегля 16px (css-values-4
/// §6.1: где элемента нет, относительные единицы — от начальных значений).
pub(super) struct Math<'a> {
    pub(super) s: &'a [u8],
    pub(super) i: usize,
}

impl Math<'_> {
    pub(super) fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    pub(super) fn done(&mut self) -> bool {
        self.ws();
        self.i == self.s.len()
    }
    pub(super) fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    pub(super) fn expr(&mut self) -> Option<MathVal> {
        let mut a = self.term()?;
        loop {
            self.ws();
            let Some(&op) = self.s.get(self.i) else {
                return Some(a);
            };
            if op != b'+' && op != b'-' {
                return Some(a);
            }
            // Сложению нужны пробелы вокруг знака (§10.1).
            if !self.s.get(self.i + 1).is_some_and(u8::is_ascii_whitespace) {
                return None;
            }
            self.i += 1;
            let b = self.term()?;
            if a.2 != b.2 {
                return None;
            }
            let k = if op == b'+' { 1.0 } else { -1.0 };
            a = (a.0 + k * b.0, a.1 + k * b.1, a.2);
        }
    }
    pub(super) fn term(&mut self) -> Option<MathVal> {
        let mut a = self.unit()?;
        loop {
            if self.eat(b'*') {
                let b = self.unit()?;
                a = match (a.2, b.2) {
                    (false, false) => (a.0 * b.0, 0.0, false),
                    (true, false) => (0.0, a.1 * b.0, true),
                    (false, true) => (0.0, b.1 * a.0, true),
                    _ => return None,
                };
            } else if self.eat(b'/') {
                let b = self.unit()?;
                if b.2 || b.0 == 0.0 {
                    return None;
                }
                a = (a.0 / b.0, a.1 / b.0, a.2);
            } else {
                return Some(a);
            }
        }
    }
    pub(super) fn unit(&mut self) -> Option<MathVal> {
        self.ws();
        if self.eat(b'(') {
            let v = self.expr()?;
            return self.eat(b')').then_some(v);
        }
        if self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            return self.func();
        }
        let start = self.i;
        if matches!(self.s.get(self.i), Some(b'+' | b'-')) {
            self.i += 1;
        }
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.i += 1;
        }
        let n: f64 = std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()?;
        let us = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            self.i += 1;
        }
        let unit = std::str::from_utf8(&self.s[us..self.i])
            .ok()?
            .to_ascii_lowercase();
        let px = match unit.as_str() {
            "" => return Some((n, 0.0, false)),
            "px" => 1.0,
            "em" | "rem" => 16.0,
            "in" => 96.0,
            "cm" => 96.0 / 2.54,
            "mm" => 96.0 / 25.4,
            "pt" => 96.0 / 72.0,
            "pc" => 16.0,
            _ => return None,
        };
        Some((0.0, n * px, true))
    }
    /// Математическая функция по имени (css-values-4 §10).
    pub(super) fn func(&mut self) -> Option<MathVal> {
        self.ws();
        let start = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .to_ascii_lowercase();
        if !self.eat(b'(') {
            return None;
        }
        let mut args = vec![self.expr()?];
        while self.eat(b',') {
            args.push(self.expr()?);
        }
        if !self.eat(b')') {
            return None;
        }
        let val = |a: &MathVal| if a.2 { a.1 } else { a.0 };
        let mk = |x: f64, len: bool| if len { (0.0, x, true) } else { (x, 0.0, false) };
        let same = args.iter().all(|a| a.2 == args[0].2);
        match (name.as_str(), args.as_slice()) {
            ("calc", [a]) => Some(*a),
            ("sign", [a]) => {
                let x = val(a);
                Some((
                    if x > 0.0 {
                        1.0
                    } else if x < 0.0 {
                        -1.0
                    } else {
                        0.0
                    },
                    0.0,
                    false,
                ))
            }
            ("abs", [a]) => Some(mk(val(a).abs(), a.2)),
            ("min", _) if same => Some(mk(
                args.iter().map(val).fold(f64::INFINITY, f64::min),
                args[0].2,
            )),
            ("max", _) if same => Some(mk(
                args.iter().map(val).fold(f64::NEG_INFINITY, f64::max),
                args[0].2,
            )),
            ("clamp", [lo, v, hi]) if same => Some(mk(val(v).min(val(hi)).max(val(lo)), lo.2)),
            _ => None,
        }
    }
}
