//! Spacing for spans; split out to keep the owning module within 250 lines.

use crate::style::values::value::Len;
use crate::text::inline::*;

/// Трекинг ПО КУСКАМ: отрезок байт → своё значение `letter-spacing`.
pub fn letter_spans(
    pieces: &[Piece],
    base_size: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut zero = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        let extra = style.letter_spacing.map(|len| {
            crate::text::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            )
        });
        if let Some(v) = extra {
            // Знак нулевой ширины единицей письма не является, и трекинг за
            // ним не идёт (css-text-3 §8.2: интервал ставится МЕЖДУ
            // единицами). Пока шёл, строка с невидимыми знаками разъезжалась
            // на их число (`letter-spacing-control-chars-001`).
            //
            // Кусок из ОДНОГО такого знака — наша распорка (`spacer_style`):
            // в её трекинге лежит поле строчной коробки или зазор, и снимать
            // его нельзя.
            if text.chars().nth(1).is_some() {
                for (off, ch) in text.char_indices().filter(|(_, c)| zero_width_format(*c)) {
                    zero.push((at + off..at + off + ch.len_utf8(), gpui::px(0.)));
                }
            }
            out.push((at..end, gpui::px(v)));
        }
        // Зазор на ГРАНИЦЕ элементов: он принадлежит не куску, а ближайшему
        // общему предку обоих знаков (css-text-3 §8.2). Ставится на последний
        // знак куска и обязан перебить его собственный трекинг, поэтому идёт
        // впереди — поиск диапазона берёт первое попадание.
        if let Some(len) = style.letter_spacing_after
            && let Some(last) = text.char_indices().next_back()
        {
            let v = crate::text::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            );
            zero.push((at + last.0..end, gpui::px(v)));
        }
        at = end;
    }
    // Нули идут ПЕРВЫМИ: поиск диапазона берёт первое попадание.
    zero.extend(out);
    zero
}

/// Знак нулевой ширины, управляющий набором, а не письмом: единицей письма он
/// не считается, и межбуквенный интервал вокруг него не ставится.
pub(super) fn zero_width_format(ch: char) -> bool {
    matches!(
        ch as u32,
        // Нулевой пробел и соединители, знаки направления письма, встраивание
        // и отмена двунаправленности, невидимые знаки математики, устаревшее
        // управление формой арабской вязи, метка порядка байтов.
        0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x2069
            | 0x206A..=0x206F
            | 0xFEFF
    )
}

/// Межсловный интервал ПО КУСКАМ: отрезок байт → добавка к каждому пробелу.
///
/// `word-spacing` на вложенном `<span>` действует только на его пробелы. Пока
/// интервал брался один на абзац, заданный внутри пропадал целиком.
pub fn word_spans(pieces: &[Piece], base_size: f32) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        let extra = style.word_spacing.map(|len| {
            crate::text::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            )
        });
        if let Some(v) = extra {
            out.push((at..end, gpui::px(v)));
        }
        at = end;
    }
    out
}
