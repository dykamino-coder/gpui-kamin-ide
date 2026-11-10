//! Явные уровни вложения UAX #9 (X1–X8) по байтам текста абзаца: где точка
//! разреза набора стоит вне всех вложений и изоляций (`chunks`).

/// Явные уровни вложения UAX #9 (X1–X8) в начале каждого байта текста: стек
/// битами, 1 — изоляция. Глубже 64 уровней кусок не режется.
#[derive(Default)]
pub(super) struct Levels {
    stack: u64,
    depth: u32,
    isolates: u32,
}

impl Levels {
    /// Учесть знак, начатый байтом `i`; `false` — вложение глубже стека.
    pub(super) fn step(&mut self, bytes: &[u8], i: usize) -> bool {
        let control = match bytes.get(i..i + 3) {
            Some([0xE2, 0x80, b @ 0xAA..=0xAE]) => *b,
            Some([0xE2, 0x81, b @ 0xA6..=0xA9]) => *b,
            _ => return true,
        };
        match control {
            // PDF закрывает вложение, но не изоляцию.
            0xAC => {
                if self.depth > 0 && self.stack & 1 == 0 {
                    self.pop();
                }
            }
            // PDI закрывает всё до ближайшей изоляции, если она открыта.
            0xA9 => {
                while self.isolates > 0 {
                    if self.pop() {
                        break;
                    }
                }
            }
            _ => {
                if self.depth == 64 {
                    return false;
                }
                let isolate = matches!(control, 0xA6..=0xA8);
                self.stack = (self.stack << 1) | u64::from(isolate);
                self.depth += 1;
                self.isolates += u32::from(isolate);
            }
        }
        true
    }

    /// Снять верхний уровень; `true` — это была изоляция.
    fn pop(&mut self) -> bool {
        let isolate = self.stack & 1 == 1;
        self.stack >>= 1;
        self.depth -= 1;
        self.isolates -= u32::from(isolate);
        isolate
    }

    pub(super) fn zero(&self) -> bool {
        self.depth == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_wait_until_every_explicit_level_is_closed() {
        let levels_after = |text: &str| {
            let mut levels = Levels::default();
            (0..text.len()).all(|i| levels.step(text.as_bytes(), i));
            levels.zero()
        };
        assert!(
            !levels_after("a \u{2067}b \u{202b}c"),
            "isolate with an embedding"
        );
        assert!(
            levels_after("a \u{2067}b \u{202b}c\u{2069} d"),
            "PDI closes both"
        );
        assert!(
            !levels_after("a \u{2067}b\u{202c} c"),
            "PDF does not close an isolate"
        );
        assert!(
            levels_after("a \u{202b}b\u{202c} c"),
            "PDF closes an embedding"
        );
        assert!(
            levels_after("a\u{2069}\u{202c} b"),
            "unmatched closers are ignored"
        );
    }
}
