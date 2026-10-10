//! Atom glue for runs; split out to keep the owning module within 250 lines.

use crate::text::inline::*;

/// Знак классов GL/WJ/ZWJ (UAX #14), который НЕЛЬЗЯ отделять от соседней
/// атомарной строчной коробки.
///
/// css-text-3 §5.1, пункт «atomic-compat-wrap», дословно: «with the exception
/// of U+00A0 NO-BREAK SPACE, there must be no soft wrap opportunity between
/// atomic inlines and adjacent characters belonging to the Unicode GL, WJ, or
/// ZWJ line breaking classes». Неразрывный пробел из правила ИСКЛЮЧЁН: рядом
/// с атомом он точку переноса, наоборот, ДАЁТ — `line-breaking-atomic-001`
/// и `-002` на этом и построены.
pub(super) fn atom_glue(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    ch != '\u{00A0}'
        && matches!(
            unicode_linebreak::break_property(ch as u32),
            NonBreakingGlue | WordJoiner | ZeroWidthJoiner
        )
}

/// Служебный кусок: метка границы атома (`U+200B`, ставится в `collect`) и
/// распорка полей строчной коробки (`SPACER`). Ширины у них нет, и поиску
/// настоящего соседа они мешать не должны: между текстом и атомом метка
/// стоит ВСЕГДА.
///
/// Сравнение идёт с куском ЦЕЛИКОМ: `U+FEFF` как знак документа
/// (`line-breaking-atomic-012/013`) приезжает внутри текста вместе с буквой и
/// служебным не считается.
pub(super) fn glue_marker(p: &Piece) -> bool {
    matches!(p, Piece::Text { text, .. } if text == "\u{200b}" || text == SPACER || text.is_empty())
}

/// Куски ряда, сгруппированные по правилу склейки с атомом (css-text-3 §5.1).
///
/// Клеится только ПРИЛЕГАЮЩЕЕ слово: точка переноса по пробелу перед ним
/// обязана остаться, иначе весь кусок текста стал бы неразрывным.
/// Две соседние коробки БЕЗ знака-склейки не склеиваются: между ними точка
/// переноса есть (`line-breaking-atomic-007`).
pub(super) fn glue_atoms(pieces: Vec<Piece>) -> Vec<Vec<Piece>> {
    // Шаг 1: у каких текстовых кусков сосед — атом (сквозь служебные метки).
    let mut before_atom = vec![false; pieces.len()];
    let mut after_atom = vec![false; pieces.len()];
    {
        let real = |from: usize, back: bool| -> Option<usize> {
            let mut k = from;
            loop {
                k = if back { k.checked_sub(1)? } else { k + 1 };
                if k >= pieces.len() {
                    return None;
                }
                if !glue_marker(&pieces[k]) {
                    return Some(k);
                }
            }
        };
        for i in 0..pieces.len() {
            if !matches!(pieces[i], Piece::Atom(_)) {
                continue;
            }
            if let Some(j) = real(i, true) {
                before_atom[j] = true;
            }
            if let Some(j) = real(i, false) {
                after_atom[j] = true;
            }
        }
    }
    // Шаг 2: отрезать прилегающее слово в свой кусок и пометить его.
    let mut cut: Vec<(Piece, bool)> = Vec::with_capacity(pieces.len() + 2);
    for (i, p) in pieces.into_iter().enumerate() {
        match p {
            Piece::Text { text, style }
                if before_atom[i] && text.chars().next_back().is_some_and(atom_glue) =>
            {
                let at = text.rfind(' ').map(|k| k + 1).unwrap_or(0);
                if at > 0 {
                    cut.push((
                        Piece::Text {
                            text: text[..at].to_string(),
                            style: style.clone(),
                        },
                        false,
                    ));
                }
                cut.push((
                    Piece::Text {
                        text: text[at..].to_string(),
                        style,
                    },
                    true,
                ));
            }
            Piece::Text { text, style }
                if after_atom[i] && text.chars().next().is_some_and(atom_glue) =>
            {
                let at = text.find(' ').map(|k| k + 1).unwrap_or(text.len());
                cut.push((
                    Piece::Text {
                        text: text[..at].to_string(),
                        style: style.clone(),
                    },
                    true,
                ));
                if at < text.len() {
                    cut.push((
                        Piece::Text {
                            text: text[at..].to_string(),
                            style,
                        },
                        false,
                    ));
                }
            }
            other => cut.push((other, false)),
        }
    }
    // Шаг 3: где шва между детьми ряда быть НЕ должно. 0 — обычный кусок,
    // 1 — служебная метка, 2 — атом, 3 — приклеенное слово.
    let kind: Vec<u8> = cut
        .iter()
        .map(|(p, glued)| match p {
            Piece::Atom(_) => 2,
            _ if *glued => 3,
            p if glue_marker(p) => 1,
            _ => 0,
        })
        .collect();
    let mut bind = vec![false; kind.len()];
    for i in 0..kind.len() {
        if kind[i] != 2 {
            continue;
        }
        // Влево: сквозь служебные метки — и дальше ТОЛЬКО если там стоит
        // приклеенное слово. Цепочка из одних меток связывать не должна:
        // иначе две соседние коробки слиплись бы навсегда.
        let mut k = i;
        while k > 0 && kind[k - 1] == 1 {
            k -= 1;
        }
        if k > 0 && kind[k - 1] == 3 {
            for j in k..=i {
                bind[j] = true;
            }
            // Слово, к которому приклеен атом, часто лежит в НЕСКОЛЬКИХ
            // кусках: `<a>A</a>&#x2011;<span>B</span>` даёт куски `A` и
            // `\u{2011}`, приклеивался только второй, шов между ними
            // оставался, и ряд рвал слово пополам — коробка уезжала на
            // вторую строку (`line-breaking-atomic-020/022/024/026`).
            // Слово продолжается влево, пока слева стоит ТЕКСТ, не
            // кончающийся пробелом: по UAX-14 LB12a перед классом GL
            // переносить нельзя, кроме как после пробела, а точка переноса
            // по пробелу обязана остаться.
            let mut g = k - 1;
            loop {
                let mut m = g;
                while m > 0 && kind[m - 1] == 1 {
                    m -= 1;
                }
                if m == 0 {
                    break;
                }
                let Piece::Text { text, .. } = &cut[m - 1].0 else {
                    break;
                };
                if text.ends_with(' ') {
                    break;
                }
                for j in m..=g {
                    bind[j] = true;
                }
                g = m - 1;
            }
        }
        // Вправо тем же порядком.
        let mut k = i;
        while k + 1 < kind.len() && kind[k + 1] == 1 {
            k += 1;
        }
        if k + 1 < kind.len() && kind[k + 1] == 3 {
            for j in (i + 1)..=(k + 1) {
                bind[j] = true;
            }
        }
    }
    let mut out: Vec<Vec<Piece>> = Vec::with_capacity(cut.len());
    for (i, (p, _)) in cut.into_iter().enumerate() {
        if bind[i] && !out.is_empty() {
            out.last_mut().expect("группа уже открыта").push(p);
        } else {
            out.push(vec![p]);
        }
    }
    out
}
