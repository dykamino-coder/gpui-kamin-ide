//! Грамматика медиазапроса (mediaqueries-4 §3): запрос, условие с and/or/not, скобки, диапазонная запись.

use super::*;

impl Media {
    /// Один `<media-query>`: `[not | only]? <media-type> [and <cond>]?`
    /// либо голое `<media-condition>`.
    pub(crate) fn mq_query(&self, raw: &str) -> SupTri {
        let s = raw.trim().to_ascii_lowercase();
        if s.is_empty() {
            return SupTri::False;
        }
        // Голое условие узнаётся по скобке: `<media-condition>` всегда
        // начинается со скобки либо с `not (`.
        if s.starts_with('(') || s.starts_with("not (") {
            return self.mq_condition(&s);
        }
        let (negate, rest) = if let Some(r) = s.strip_prefix("not ") {
            (true, r.trim())
        } else if let Some(r) = s.strip_prefix("only ") {
            (false, r.trim())
        } else {
            (false, s.as_str())
        };
        let (ty, tail) = match rest.split_once(" and ") {
            Some((t, c)) => (t.trim(), Some(c.trim())),
            None => (rest.trim(), None),
        };
        // `<media-type>` — идентификатор, и слова `only`/`not`/`and`/`or`/
        // `layer` типом БЫТЬ НЕ МОГУТ (mediaqueries-5 §3). Значит `@media and`
        // и `@media not and` — синтаксические ошибки, а ошибочный запрос
        // равен `not all`, то есть ЛОЖЬ даже под отрицанием
        // (`mq-invalid-media-type-001..004`, `-layer-001`).
        if ty.is_empty()
            || matches!(ty, "only" | "not" | "and" | "or" | "layer")
            || !ty.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return SupTri::False;
        }
        let hit = match ty {
            "all" => SupTri::True,
            "screen" if !self.print => SupTri::True,
            "print" if self.print => SupTri::True,
            // Неизвестный, но грамматически годный тип — ЛОЖЬ, а не ошибка:
            // `not unknown` истинно (`at-media-002`).
            _ => SupTri::False,
        };
        // Отрицание относится ко всему `<media-type> and <cond>`, а не к
        // одному типу, поэтому склейка идёт первой.
        let full = match tail {
            None => hit,
            Some(cond) => mq_and(hit, self.mq_condition(cond)),
        };
        if negate { sup_not(full) } else { full }
    }

    /// `<media-condition>`: `not <in-parens>` | `<in-parens> (and ..)*` |
    /// `<in-parens> (or ..)*`. Смешивать `and` и `or` без скобок нельзя
    /// (§3.1), такой запрос негоден целиком.
    pub(crate) fn mq_condition(&self, raw: &str) -> SupTri {
        let s = raw.trim();
        if let Some(rest) = s.strip_prefix("not ") {
            let Some((term, tail)) = take_parens(rest.trim()) else {
                return SupTri::False;
            };
            if !tail.trim().is_empty() {
                return SupTri::False;
            }
            return sup_not(self.mq_in_parens(term));
        }
        let Some((term, mut rest)) = take_parens(s) else {
            return SupTri::False;
        };
        let mut acc = self.mq_in_parens(term);
        let mut op: Option<&str> = None;
        loop {
            let r = rest.trim_start();
            if r.is_empty() {
                return acc;
            }
            let word = if let Some(w) = r.strip_prefix("and ") {
                rest = w;
                "and"
            } else if let Some(w) = r.strip_prefix("or ") {
                rest = w;
                "or"
            } else {
                return SupTri::False;
            };
            if *op.get_or_insert(word) != word {
                return SupTri::False;
            }
            let Some((term, tail)) = take_parens(rest.trim_start()) else {
                return SupTri::False;
            };
            let v = self.mq_in_parens(term);
            acc = if word == "and" {
                mq_and(acc, v)
            } else {
                mq_or(acc, v)
            };
            rest = tail;
        }
    }

    /// `<media-in-parens>` без внешних скобок: вложенное условие, фича или
    /// `<general-enclosed>`.
    pub(crate) fn mq_in_parens(&self, inner: &str) -> SupTri {
        let s = inner.trim();
        if s.starts_with('(') || s.starts_with("not ") {
            return self.mq_condition(s);
        }
        // Диапазонная запись §3.3: `(width > 100px)`, `(100px <= width)`,
        // `(100px < width < 200px)`. Двусторонняя форма — это И двух
        // односторонних.
        if s.contains('<') || s.contains('>') || (s.contains('=') && !s.contains(':')) {
            return self.mq_range(s);
        }
        if let Some((name, value)) = s.split_once(':') {
            return self.mq_feature(name.trim(), value.trim(), MqCmp::Eq);
        }
        // Булева форма `<mf-boolean>`: голое имя истинно, когда значение
        // фичи не «ноль/none» (§2.4.2).
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return self.mq_boolean(s);
        }
        // `<general-enclosed>` — «неизвестно» (§3.1): пустые скобки, мусор,
        // сломанный диапазон.
        SupTri::Unknown
    }

    /// Диапазонная запись. Разрез по знакам сравнения; фича может стоять
    /// слева, справа либо между двумя пределами.
    pub(crate) fn mq_range(&self, s: &str) -> SupTri {
        let mut parts: Vec<(&str, MqCmp)> = vec![];
        let mut start = 0usize;
        let b = s.as_bytes();
        let mut at = 0usize;
        while at < b.len() {
            let (cmp, len) = match (b[at], b.get(at + 1)) {
                (b'<', Some(b'=')) => (MqCmp::Le, 2),
                (b'>', Some(b'=')) => (MqCmp::Ge, 2),
                (b'<', _) => (MqCmp::Lt, 1),
                (b'>', _) => (MqCmp::Gt, 1),
                (b'=', _) => (MqCmp::Eq, 1),
                _ => {
                    at += 1;
                    continue;
                }
            };
            parts.push((&s[start..at], cmp));
            at += len;
            start = at;
        }
        // Ни одного знака — это не диапазон, а мусор (`mq-range-001`:
        // `(width 500px)` без знака НЕгоден и обязан стать
        // `<general-enclosed>`, а не ошибкой запроса).
        if parts.is_empty() {
            return SupTri::Unknown;
        }
        let tail = s[start..].trim();
        match parts.len() {
            1 => {
                let (left, cmp) = parts[0];
                let left = left.trim();
                // Фича слева — сравнение как есть; фича справа — обратное.
                if self.mq_numeric(mq_base(left)).is_some() {
                    self.mq_feature(left, tail, cmp)
                } else {
                    self.mq_feature(tail, left, cmp.flip())
                }
            }
            2 => {
                let (lo, c1) = parts[0];
                let (name, c2) = parts[1];
                let a = self.mq_feature(name.trim(), lo.trim(), c1.flip());
                let b = self.mq_feature(name.trim(), tail, c2);
                mq_and(a, b)
            }
            _ => SupTri::Unknown,
        }
    }
}
