//! Soft break opportunities separated from mandatory breaks and final filtering.

use super::{
    centered_punctuation, conditional_japanese_starter, iteration_mark, letter_unit,
    no_break_after, no_break_before, wide_postfix, wide_prefix,
};
use crate::text::paragraph::*;

impl Paragraph {
    pub(crate) fn soft_opportunities(&self, out: &mut Vec<Stop>) {
        if self.wrap.anywhere {
            for (i, _) in self.text.char_indices().skip(1) {
                out.push(Stop {
                    at: i,
                    mandatory: false,
                });
            }
        } else {
            // `line-break: anywhere` на вложенном куске: точки ставятся
            // только внутри него, остальной абзац живёт по UAX-14.
            for (range, w) in &self.spans {
                if !w.anywhere {
                    continue;
                }
                for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                    out.push(Stop {
                        at: range.start + i,
                        mandatory: false,
                    });
                }
            }
            for (at, kind) in self.linebreaks() {
                // Обязательные разрывы Юникода — это не только перевод
                // строки: подача страницы, вертикальная табуляция,
                // разделители строки и абзаца, NEL. Все они заканчивают
                // строку принудительно (`line-breaking-022`).
                match kind {
                    unicode_linebreak::BreakOpportunity::Allowed => out.push(Stop {
                        at,
                        mandatory: false,
                    }),
                    // Конец текста переносчик тоже зовёт обязательным
                    // разрывом — но переносить там нечего, а лишняя точка
                    // ломает счёт строк.
                    unicode_linebreak::BreakOpportunity::Mandatory if at < self.text.len() => out
                        .push(Stop {
                            at,
                            mandatory: true,
                        }),
                    unicode_linebreak::BreakOpportunity::Mandatory => {}
                }
            }
            {
                // Разрыв разрешён между знаками слова, но запреты
                // типографики он не отменяет: перед точкой, скобкой или
                // знаком препинания рвать всё равно нельзя (это отличает
                // `break-all` от `line-break: anywhere`). Правило берётся
                // ПО МЕСТУ: заданное на вложенном `<span>`, оно действует
                // только на его байты.
                for (i, ch) in self.text.char_indices().skip(1) {
                    if !self.wrap_at(i).break_all {
                        continue;
                    }
                    let before = self.text[..i].chars().next_back();
                    let allowed = !ch.is_whitespace()
                        && !no_break_before(ch)
                        && before.is_none_or(|c| !no_break_after(c));
                    if allowed {
                        out.push(Stop {
                            at: i,
                            mandatory: false,
                        });
                    }
                }
            }
            {
                // `line-break: normal`/`loose` (css-text-3 §5.2): перед
                // малой каной и знаком долготы (класс CJ) разрыв
                // разрешён — UAX-14 в строгом варианте держит их при
                // предыдущем знаке. В `loose` ещё перед знаками повтора и
                // между неразделимыми `‥…` (класс IN). Слева — не пробел
                // и не открывающая скобка/запрет (`line-break-loose-011`,
                // `line-break-normal-011`). Уровень берётся ПО МЕСТУ.
                for (i, ch) in self.text.char_indices().skip(1) {
                    let level = self.wrap_at(i).loose;
                    if level == 0 {
                        continue;
                    }
                    let Some(before) = self.text[..i].chars().next_back() else {
                        continue;
                    };
                    let cjk = self.wrap_at(i).cjk_lang;
                    let eased = conditional_japanese_starter(ch)
                        || (level >= 2
                            && (iteration_mark(ch)
                                || (matches!(ch, '\u{2025}' | '\u{2026}')
                                    && matches!(before, '\u{2025}' | '\u{2026}'))))
                        // Только для китайского/японского письма (§5.2):
                        // `normal`/`loose` — перед волнистым тире
                        // U+301C/U+30A0; `loose` — перед центрированной
                        // пунктуацией, перед широкими постфиксами (PO)
                        // и ПОСЛЕ широких префиксов (PR)
                        // (`line-break-loose-016a/016b/017a/017b/018`).
                        || (cjk && matches!(ch, '\u{301C}' | '\u{30A0}'))
                        || (cjk
                            && level >= 2
                            && (centered_punctuation(ch) || wide_postfix(ch)));
                    // После широкого префикса запрет UAX-14 «PR × ID»
                    // снимается целиком — проверяется только правый знак.
                    let after_prefix = cjk
                        && level >= 2
                        && wide_prefix(before)
                        && !ch.is_whitespace()
                        && !no_break_before(ch);
                    if after_prefix
                        || (eased
                            && !before.is_whitespace()
                            && !no_break_after(before)
                            && !matches!(before, '\u{200B}' | '\u{2060}' | '\u{00A0}'))
                    {
                        out.push(Stop {
                            at: i,
                            mandatory: false,
                        });
                    }
                }
            }
            {
                // Мягкий перенос — ЯВНАЯ точка переноса: она сильнее
                // запретов типографики. UAX-14 держит вместе перенос и
                // следующую за ним кавычку (`hyphens-i18n-manual-003`:
                // «tú­’àn» не рвалось вовсе).
                for (i, ch) in self.text.char_indices() {
                    if ch == SOFT_HYPHEN {
                        out.push(Stop {
                            at: i + ch.len_utf8(),
                            mandatory: false,
                        });
                    }
                }
            }
            {
                // После КАЖДОГО сохранённого пробела — своя точка разрыва.
                // Табуляция тоже пробел: `break-spaces` рвёт и после неё,
                // хотя UAX-14 держит подряд идущие табуляции вместе
                // (`break-spaces-tab-003`).
                // Пробел здесь — ЛЮБОЙ сохранённый пробельный знак:
                // `break-spaces` рвёт и после идеографического U+3000,
                // и после em-space U+2003 (`break-spaces-with-ideographic-
                // space-005/010`, `trailing-ideographic-space-break-spaces-007`
                // показывали красное). Перевод строки исключён: его разрыв
                // обязательный и ставится своим проходом.
                for (i, ch) in self.text.char_indices() {
                    if ch.is_whitespace()
                        && ch != '\n'
                        && ch != '\r'
                        && self.wrap_at(i).break_spaces
                    {
                        out.push(Stop {
                            at: i + ch.len_utf8(),
                            mandatory: false,
                        });
                    }
                }
            }
            {
                // Запрет действует только МЕЖДУ буквенными единицами:
                // иероглифы друг от друга не отрываются, а после запятой
                // или дефиса строка рвётся по-прежнему.
                out.retain(|s| {
                    if !self.wrap_at(s.at).keep_all {
                        return true;
                    }
                    let before = self.text[..s.at].chars().next_back();
                    let after = self.text[s.at..].chars().next();
                    s.mandatory
                        || !(before.is_some_and(letter_unit) && after.is_some_and(letter_unit))
                });
            }
        }
    }
}
