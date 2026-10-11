//! Autospace for spans; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;

/// Зазоры `text-autospace` — диапазонами трекинга по тексту абзаца.
///
/// По css-text-4 §7 между иероглифом и соседней буквой (`ideograph-alpha`) или
/// цифрой (`ideograph-numeric`) стоит зазор в 1/8 кегля. Соседство считается
/// по ЗНАКАМ, а не по кускам: пара лежит и внутри одного текстового узла
/// (`国国XX国`), и по разные стороны границы (`<b>永</b>abc`).
///
/// Зазор — трекинг на знаке ПЕРЕД границей, и ставится он диапазоном, не
/// разрезая кусок. Два тупика, из которых это единственный выход:
///
/// * знаком-распоркой зазор сделать нельзя: любая распорка нулевой ширины
///   (U+FEFF, U+2060) имеет класс переноса WJ и запрещает разрыв по обе
///   стороны, а зазор на перенос влиять не должен;
/// * резать кусок ради своего трекинга тоже нельзя: соседние прогоны кладутся
///   с независимым округлением, и между половинками слова появлялся шов
///   в точку (`text-autospace-001`, две буквы `XX` расходились).
///
/// Трекинг вдобавок схлопывается на краю строки сам — как и требует
/// спецификация: строка рвётся по границе зазора, и на новой строке зазора
/// уже нет.
pub fn autospace_spans(
    pieces: &[Piece],
    base_size: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    // Знак перед курсором: его смещение, длина, стиль и кегль куска.
    let mut prev: Option<(usize, usize, char, f32, &Computed)> = None;
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            // Картинка иероглифом не бывает и соседство разрывает.
            prev = None;
            continue;
        };
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        for (off, ch) in text.char_indices() {
            // Соединительный знак (огласовка, диакритика) письменности не
            // меняет: он прозрачен, а класс пары берётся у БАЗОВОЙ буквы.
            // Пока знак считался обычным, арабское слово с огласовкой на
            // конце теряло зазор перед иероглифом (`text-autospace-mixed-001`,
            // `text-autospace-combining-marks`).
            if combining(ch) {
                if let Some(p) = prev.as_mut() {
                    // Место зазора — после ВСЕГО сочетания, поэтому отрезок
                    // переезжает на знак, а буква для решения прежняя.
                    p.0 = at + off;
                    p.1 = ch.len_utf8();
                }
                continue;
            }
            if let Some((p_at, p_len, p_ch, p_size, p_style)) = prev
                && autospace_between(p_ch, ch, style)
            {
                let family = p_style.font_family.clone().unwrap_or_default();
                let had = crate::text::metrics::spacing_px(p_style.letter_spacing, &family, p_size);
                // ЗАМЕРЕНО И ОТКАЧЕНО: считать зазор от `ic`, а не от кегля
                // (css-text-4 §7.1 «1/8 of the ideographic advance»). Полный
                // свод CSS3: приобретено 0, потеряно 1 —
                // `text-autospace-supplementary-ideograph` 0.08 -> 0.59.
                // Пары `text-autospace-elements-005/005b` (0.56) не сдвинулись.
                out.push((p_at..p_at + p_len, gpui::px(had + p_size / 8.0)));
            }
            prev = Some((at + off, ch.len_utf8(), ch, size, style));
        }
        at += text.len();
    }
    out
}

/// Нужен ли зазор между двумя соседними знаками.
pub(super) fn autospace_between(left: char, right: char, style: &Computed) -> bool {
    let alpha = style.autospace_alpha.unwrap_or(false);
    let numeric = style.autospace_numeric.unwrap_or(false);
    let pair = |ideo: char, other: char| {
        ideographic(ideo)
            && ((alpha && other.is_alphabetic() && !ideographic(other))
                || (numeric && other.is_ascii_digit()))
    };
    pair(left, right) || pair(right, left)
}

/// Соединительный ли знак: своей ширины нет, письменность задаёт базовая
/// буква перед ним.
pub(super) fn combining(ch: char) -> bool {
    matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}

/// Иероглиф ли знак — по классу переноса строк.
pub(crate) fn ideographic(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Ideographic | ConditionalJapaneseStarter | NonStarter
    )
}
