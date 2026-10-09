//! Прогоны по кускам: перенос, сдвиги, высота строки, интервалы, оформление, слова, автопробелы.

use crate::text::inline::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
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

/// Относительный сдвиг ПО КУСКАМ: отрезок байт → смещение отрисовки.
///
/// Отдельно от `shift_spans`: тот растит строчную коробку, а относительный
/// сдвиг на поток не влияет вовсе (CSS 2.1 §9.4.3).
pub fn rel_spans(pieces: &[Piece]) -> Vec<(std::ops::Range<usize>, (f32, f32))> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        if let Some(d) = style.rel_shift
            && d != (0.0, 0.0)
        {
            out.push((at..end, d));
        }
        at = end;
    }
    out
}

/// Сдвиг ПО КУСКАМ: отрезок байт → смещение базовой линии в точках.
///
/// `vertical-align: super`/`sub` поднимает и опускает кусок внутри строки.
/// Доля кегля взята браузерная: треть вверх и пятая часть вниз.
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-centralbaseline-2026-09.md`,
// 4 хунка): центральная доминантная базовая линия у повёрнутой строки
// (css-writing-modes-4 §4.2: «the central baseline is used as the
// dominant baseline when text-orientation is mixed or upright»,
// величина по css-inline-3 A.2 = (ascent − descent)/2; Blink —
// `computed_style.cc:2154`). Патч включал разделение `mixed` и
// `sideways`, которые у нас хранятся одним булем.
// Срез 1476 пар, база тем же списком: **+0 / −3**. Обещанные
// `text-baseline-vrl-002` и `-vlr-003` НЕ позеленели, а ушли
// `vertical-alignment-003` 0.39 → 0.61, `-009` 0.39 → 0.55,
// `-vlr-025` 0.17 → 1.17. Возвращать только вместе с разбором того,
// почему поправка не даёт нуля там, где арифметика скаута его даёт.
pub fn shift_spans(
    pieces: &[Piece],
    base_size: f32,
    line_px: f32,
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
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        // К вне-поточной коробке выравнивание в строке не применяется
        // (CSS 2.1 §9.5, §10.8): она из строки вынута.
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
            );
        let dy = (!out_of_flow)
            .then(|| style.vertical_shift_px)
            .flatten()
            .or_else(|| {
                // Единица шрифта разрешается ЗДЕСЬ: кегль и гарнитура куска
                // уже известны.
                style.vertical_shift_len.map(|l| {
                    let family = style.font_family.clone().unwrap_or_default();
                    -crate::text::metrics::spacing_px(Some(l), &family, size)
                })
            })
            .or_else(|| style.vertical_shift.map(|k| k * size))
            // Процент считается от `line-height` САМОГО куска (§10.8.1), а не
            // от кегля: `vertical-align: 50%` при `line-height: 2` — это кегль
            // целиком, а не половина. Замерено: 0 и 0 — правка по спеке, в
            // своде такой записи почти нет.
            .or_else(|| {
                style.vertical_shift_pct.map(|k| {
                    let own = match style.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => line_px,
                    };
                    k * own
                })
            })
            .or_else(|| {
                // `text-top`/`text-bottom` равняют край куска по краю
                // ТЕКСТОВОЙ области родителя (CSS 2.1 §10.8.1). Разница
                // берётся из метрик обоих кеглей: подъём и спуск шрифта —
                // доли кегля, поэтому величина пропорциональна их разности.
                // Коробка куска — это глифы ПЛЮС полулидинг с каждой
                // стороны (CSS 2.1 §10.8.1), а у родителя берётся текстовая
                // область без лидинга. Подъём и спуск — доли кегля.
                const ASCENT: f32 = 0.8;
                const DESCENT: f32 = 0.2;
                let parent = style.vertical_align_base.unwrap_or(base_size);
                // Полулидинг БЫВАЕТ отрицательным: `L = line-height − AD`
                // (§10.8.1), и при `line-height` меньше кегля коробка куска
                // выше строки. Зажим нулём убивал ровно этот случай.
                let half = (line_px - size) / 2.0;
                style.vertical_align_text.map(|top| {
                    if top {
                        (ASCENT * size + half) - ASCENT * parent
                    } else {
                        DESCENT * parent - (DESCENT * size + half)
                    }
                })
            })
            // Запрет вне-поточной коробке — на ВСЮ цепочку, а не на её голову:
            // `(!out_of_flow).then(..)` гасил только сдвиг длиной в точках, а
            // `sub`/`super`, `em`, `ex`, процент и `text-top`/`text-bottom`
            // проходили дальше по `or_else`. Абсолютный кусок блокифицирован
            // (CSS 2.1 §9.7), `vertical-align` к нему не применяется вовсе.
            // `vertical-align-sub-001`: зелёный кусок уезжал вниз на 0.2em и
            // открывал красный. `super-001` был зелёным случайно: подъём
            // гасила верхняя надбавка строки (`line_padding`).
            .filter(|_| !out_of_flow);
        if let Some(v) = dy {
            out.push((at..end, gpui::px(v)));
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
fn zero_width_format(ch: char) -> bool {
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
fn autospace_between(left: char, right: char, style: &Computed) -> bool {
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
fn combining(ch: char) -> bool {
    matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}

/// Иероглиф ли знак — по классу переноса строк.
pub(super) fn ideographic(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Ideographic | ConditionalJapaneseStarter | NonStarter
    )
}
