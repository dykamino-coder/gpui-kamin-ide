//! Text nodes for collect; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::text::inline::*;

/// Начинается ли текст узла пробельным рядом с переводом строки.
pub(super) fn leading_segment_break(raw: &str) -> bool {
    raw.chars()
        .take_while(|c| is_collapsible(*c))
        .any(|c| matches!(c, '\n' | '\r'))
}

/// Последний ЗНАЧАЩИЙ знак собранных кусков — нулевой пробел. Распорки полей
/// и рамок, метки направления и куски вне потока — это границы коробок, а
/// для преобразования перевода строки их нет.
pub(super) fn ends_with_zwsp(out: &[Piece]) -> bool {
    for p in out.iter().rev() {
        match p {
            Piece::Overlay(..) => continue,
            Piece::Atom(_) => return false,
            Piece::Text { text, .. } => {
                if text.is_empty() || text == SPACER || text.chars().all(bidi_format) {
                    continue;
                }
                return text.ends_with('\u{200b}');
            }
        }
    }
    false
}

#[allow(clippy::needless_borrow)]
pub(super) fn collect_text(
    t: &str,
    inherited: &Computed,
    case: &mut text_case::Context,
    out: &mut Vec<Piece>,
) {
    // Возврат каретки (U+000D) — ПРОБЕЛ при любом `white-space`
    // (css-text-3 §4.1: «carriage returns … are treated
    // identically to spaces»): разбор HTML сводит CR к LF только в
    // разметке, а `&#x0D;` доезжает знаком и при `pre*` рвал
    // строку (`control-chars-00D`).
    let cr_free;
    let t: &str = if t.contains('\r') {
        cr_free = t.replace('\r', " ");
        &cr_free
    } else {
        t
    };
    // `white-space: pre*` сохраняет пробелы как есть: отступы кода
    // иначе схлопывались в один пробел и текст терял форму.
    let raw = if inherited.preserve_newlines == Some(true) && inherited.keep_spaces != Some(false) {
        // Табуляция остаётся СВОИМ знаком: её ширину считает
        // раскладка строк по позициям табуляции. Разворот в
        // пробелы менял и число точек переноса, и вид хвоста
        // строки (`break-spaces-tab`).
        t.to_string()
    } else if inherited.preserve_newlines == Some(true) {
        // `pre-line`: пробелы схлопываются, переводы строк живут.
        t.split('\n')
            .map(normalize_spaces)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        normalize_spaces(t)
    };
    // Замена нулевого пробеля идеографическим идёт ПО КУСКАМ
    // абзаца (`space_transform_pieces`): соседи точки переноса
    // сплошь и рядом лежат в других кусках, и проход по одному
    // узлу их не видит.
    let mut text = breakable(&text_case::transform(&raw, inherited, case), inherited);
    // То же правило нулевого пробела СКВОЗЬ границу строчной
    // коробки (css-text-4 §4.1.3; границ коробок для него нет —
    // `seg-break-transformation-018`): перевод строки в начале
    // узла, а нулевой пробел — последний знак предыдущего куска.
    if inherited.preserve_newlines != Some(true)
        && leading_segment_break(t)
        && ends_with_zwsp(&out)
        && text.starts_with(' ')
    {
        text.remove(0);
    }
    if !text.is_empty() {
        out.push(Piece::Text {
            text,
            style: inherited.clone(),
        });
    }
}
