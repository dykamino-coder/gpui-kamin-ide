//! Прогоны по кускам: перенос, сдвиги, высота строки, интервалы, оформление, слова, автопробелы.

mod autospace;
mod position;
mod spacing;
pub use crate::text::inline::spans::autospace::autospace_spans;
pub(super) use crate::text::inline::spans::autospace::ideographic;
pub use crate::text::inline::spans::position::rel_spans;
pub use crate::text::inline::spans::position::shift_spans;
pub use crate::text::inline::spans::spacing::letter_spans;
pub use crate::text::inline::spans::spacing::word_spans;

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;
use gpui::TextStyle;

/// `word-space-transform: ideographic-space`.
///
/// Пробел нулевой ширины между двумя иероглифами становится идеографическим:
/// у него появляется ширина, и разметка без настоящих пробелов набирается так
/// же, как с ними. Только МЕЖДУ иероглифами — у края куска преобразования нет.
/// Правила переноса ПО КУСКАМ: отрезок байт готового текста → своё правило.
///
/// `word-break` и `overflow-wrap`, заданные на вложенном `<span>`, действуют
/// только на его знаки. Пока правило собиралось одно на абзац, всё заданное
/// внутри пропадало целиком.
pub fn wrap_spans(
    pieces: &[Piece],
    base: &Computed,
) -> Vec<(std::ops::Range<usize>, crate::text::paragraph::Wrap)> {
    // Правила абзаца целиком — с ними сравнивается каждый кусок: в список
    // попадает только тот, у кого они ДРУГИЕ. Без сравнения `<span>` со своим
    // `white-space` внутри `pre` неотличим от родителя, и перенос ему
    // запрещён вместе со всем абзацем.
    let whole = crate::text::paragraph::wrap_of(base);
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
        let w = crate::text::paragraph::wrap_of(style);
        if w != whole {
            out.push((at..end, w));
        }
        at = end;
    }
    out
}

/// Высота строки ПО КУСКАМ: отрезок байт → своя `line-height` в точках.
///
/// §10.8: высоту строчной коробки задают куски, а не блок — у каждого своя
/// коробка отступа высотой `line-height`, и полулидинг считается от НЕЁ.
/// Отдаётся только кусок со СВОИМ значением: унаследованное блок уже учёл.
pub fn line_height_spans(
    pieces: &[Piece],
    inherited: &Computed,
    base_size: f32,
    normal: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
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
        if style.line_height != inherited.line_height
            && let Some(lh) = style.line_height
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                _ => base_size,
            };
            let px_of = match lh {
                Len::Px(v) => Some(v),
                Len::Pct(k) => Some(k * size),
                Len::Em(k) => Some(k * size),
                _ => None,
            };
            match px_of {
                Some(v) => out.push((at..end, gpui::px(v))),
                None => out.push((at..end, gpui::px(normal * size))),
            }
        }
        at = end;
    }
    out
}

/// Куски с украшениями (css-text-decor-3 §2.1) и их украшенные прогоны:
/// смежные куски с тем же украшением на том же месте списка (Blink
/// `ContinuesDecoratedRun`); атом прогон рвёт. Скрытый кусок не украшается.
pub fn decor_spans(pieces: &[Piece], base: &TextStyle) -> Vec<crate::text::paragraph::DecorSpan> {
    use crate::text::paragraph::{DecorItem, DecorSpan};
    let mut out: Vec<DecorSpan> = Vec::new();
    let mut at = 0usize;
    let mut broken = true;
    for p in pieces {
        match p {
            Piece::Text { text, style } => {
                let end = at + text.len();
                if !style.decors.is_empty() && style.hidden != Some(true) && !text.is_empty() {
                    let mut items: Vec<DecorItem> = style
                        .decors
                        .iter()
                        .map(|d| {
                            let probe = Computed {
                                font_family: d.font.family.clone(),
                                monospace: d.font.monospace,
                                font_weight: d.font.weight,
                                italic: d.font.italic,
                                font_stretch: d.font.stretch,
                                ..Computed::default()
                            };
                            DecorItem {
                                decor: d.clone(),
                                font: run_for("x", &probe, base).font,
                                group: at..end,
                            }
                        })
                        .collect();
                    if !broken
                        && let Some(prev) = out.last()
                        && prev.range.end == at
                    {
                        for (k, item) in items.iter_mut().enumerate() {
                            if let Some(pi) = prev.items.get(k)
                                && pi.decor == item.decor
                            {
                                item.group.start = pi.group.start;
                            }
                        }
                    }
                    out.push(DecorSpan {
                        range: at..end,
                        items,
                        skip_ink: style.skip_ink != Some(0),
                        skip_spaces: style.skip_spaces.unwrap_or(3),
                    });
                }
                if !text.is_empty() {
                    broken = false;
                }
                at = end;
            }
            Piece::Atom(_) => broken = true,
            Piece::Overlay(..) => {}
        }
    }
    for i in (0..out.len().saturating_sub(1)).rev() {
        let (head, tail) = out.split_at_mut(i + 1);
        let (cur, next) = (&mut head[i], &tail[0]);
        for (k, item) in cur.items.iter_mut().enumerate() {
            if let Some(ni) = next.items.get(k)
                && ni.group.start == item.group.start
            {
                item.group.end = ni.group.end;
            }
        }
    }
    out
}
