//! Математические функции в значении признака медиазапроса: `calc()`, `sign()`, `abs()`,
//! `min()`/`max()`/`clamp()` над длинами, разрешениями и числами.
//!
//! mediaqueries-4 §1.3 / css-values-4 §10: значение признака — любой
//! `<calc-sum>` нужного типа; относительные единицы шрифта берутся от
//! НАЧАЛЬНОГО шрифта (16px), а не от корня (`mq-calc-sign-function-*`,
//! `mq-calc-resolution`). Blink: `media_query_exp.cc` разрешает выражение
//! через `CSSMathExpressionNode` с контекстом начального шрифта.

/// Тип значения: число, длина (px) или разрешение (dppx).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Num,
    Px,
    Res,
}

/// Вычислить `raw`, если оно — математическая функция. `None` — не функция
/// или негодное выражение.
pub(super) fn eval(raw: &str, vcx: crate::document::state::ValueCx<'_>) -> Option<(f32, Kind)> {
    let s = raw.trim();
    let low = s.to_ascii_lowercase();
    let is_fn = ["calc(", "sign(", "abs(", "min(", "max(", "clamp("]
        .iter()
        .any(|f| low.starts_with(f));
    if !is_fn {
        return None;
    }
    let mut p = Parser {
        s: low.as_bytes(),
        at: 0,
        vcx,
    };
    let v = p.sum()?;
    p.ws();
    (p.at == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    vcx: crate::document::state::ValueCx<'a>,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.ws();
        if self.s.get(self.at) == Some(&b) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn sum(&mut self) -> Option<(f32, Kind)> {
        let mut acc = self.product()?;
        loop {
            self.ws();
            let sign = match self.s.get(self.at) {
                Some(b'+') => 1.0,
                Some(b'-') => -1.0,
                _ => return Some(acc),
            };
            self.at += 1;
            let rhs = self.product()?;
            if rhs.1 != acc.1 {
                return None;
            }
            acc.0 += sign * rhs.0;
        }
    }

    fn product(&mut self) -> Option<(f32, Kind)> {
        let mut acc = self.value()?;
        loop {
            self.ws();
            let op = match self.s.get(self.at) {
                Some(&c @ (b'*' | b'/')) => c,
                _ => return Some(acc),
            };
            self.at += 1;
            let rhs = self.value()?;
            acc = match (op, acc.1, rhs.1) {
                (b'*', k, Kind::Num) => (acc.0 * rhs.0, k),
                (b'*', Kind::Num, k) => (acc.0 * rhs.0, k),
                (b'/', k, Kind::Num) => (acc.0 / rhs.0, k),
                _ => return None,
            };
        }
    }

    fn args(&mut self) -> Option<Vec<(f32, Kind)>> {
        let mut out = vec![self.sum()?];
        while self.eat(b',') {
            out.push(self.sum()?);
        }
        self.eat(b')').then_some(out)
    }

    fn value(&mut self) -> Option<(f32, Kind)> {
        self.ws();
        if self.eat(b'(') {
            let v = self.sum()?;
            return self.eat(b')').then_some(v);
        }
        let start = self.at;
        while self
            .s
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'-' && self.at == start)
        {
            self.at += 1;
        }
        if self.s.get(self.at) == Some(&b'(') && self.at > start {
            let name = std::str::from_utf8(&self.s[start..self.at])
                .ok()?
                .to_string();
            self.at += 1;
            let a = self.args()?;
            let same = a.iter().all(|v| v.1 == a[0].1);
            return match (name.as_str(), a.as_slice()) {
                ("calc", [v]) => Some(*v),
                ("sign", [v]) => Some((
                    if v.0 > 0.0 {
                        1.0
                    } else if v.0 < 0.0 {
                        -1.0
                    } else {
                        0.0
                    },
                    Kind::Num,
                )),
                ("abs", [v]) => Some((v.0.abs(), v.1)),
                ("min", _) if same => {
                    Some((a.iter().map(|v| v.0).fold(f32::INFINITY, f32::min), a[0].1))
                }
                ("max", _) if same => Some((
                    a.iter().map(|v| v.0).fold(f32::NEG_INFINITY, f32::max),
                    a[0].1,
                )),
                ("clamp", [lo, v, hi]) if same => Some((v.0.min(hi.0).max(lo.0), v.1)),
                _ => None,
            };
        }
        self.at = start;
        self.number()
    }

    fn number(&mut self) -> Option<(f32, Kind)> {
        let start = self.at;
        if matches!(self.s.get(self.at), Some(b'+' | b'-')) {
            self.at += 1;
        }
        while self
            .s
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.at += 1;
        }
        let n: f32 = std::str::from_utf8(&self.s[start..self.at])
            .ok()?
            .parse()
            .ok()?;
        let u0 = self.at;
        while self
            .s
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'%')
        {
            self.at += 1;
        }
        let unit = std::str::from_utf8(&self.s[u0..self.at]).ok()?;
        let (ch, ex) = crate::text::metrics::ch_ex_px("", 16.0, self.vcx);
        Some(match unit {
            "" => (n, Kind::Num),
            "px" => (n, Kind::Px),
            "em" | "rem" => (n * 16.0, Kind::Px),
            "ex" | "rex" => (n * ex, Kind::Px),
            "ch" | "rch" => (n * ch, Kind::Px),
            "in" => (n * 96.0, Kind::Px),
            "cm" => (n * 96.0 / 2.54, Kind::Px),
            "mm" => (n * 96.0 / 25.4, Kind::Px),
            "q" => (n * 96.0 / 101.6, Kind::Px),
            "pt" => (n * 96.0 / 72.0, Kind::Px),
            "pc" => (n * 16.0, Kind::Px),
            "x" | "dppx" => (n, Kind::Res),
            "dpi" => (n / 96.0, Kind::Res),
            "dpcm" => (n * 2.54 / 96.0, Kind::Res),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_resolution_math() {
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
        assert_eq!(
            eval("calc(1px * (1 + sign(16px - 1rem)))", vcx),
            Some((1.0, Kind::Px))
        );
        assert_eq!(
            eval("calc(2 * sign(16px - 1rem))", vcx),
            Some((0.0, Kind::Num))
        );
        assert_eq!(eval("calc(2x - 96dpi)", vcx), Some((1.0, Kind::Res)));
        assert_eq!(eval("calc(1px + 2)", vcx), None);
        assert_eq!(eval("10px", vcx), None);
    }

    #[test]
    fn media_features_take_math_and_negative_ranges() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let m = crate::style::css::Media::default();
        for q in [
            "(width > calc(1px * (1 + sign(16px - 1rem))))",
            "(grid: calc(2 * sign(16px - 1rem)))",
            "(resolution >= calc(2x - 96dpi))",
            "(min-width: -100px)",
            "(min-color: -10)",
            "(min-color-index: -10)",
            "(min-monochrome: -10)",
            "(min-device-height: -100px)",
            "(not (max-width: -100px))",
            "(scripting)",
            "(min-width: -100px)
    and
    (not (max-color: -10))",
        ] {
            assert!(m.matches(q, vcx), "{q}");
        }
    }
}
