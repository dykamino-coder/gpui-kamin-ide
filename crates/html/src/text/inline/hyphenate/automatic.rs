//! Словарный перенос `hyphens: auto` по целым словам абзаца: слово собирается из текста всех
//! кусков (разметка режет слова где угодно, кусок вне потока слова не рвёт, атом рвёт), точки
//! слогораздела по образцам Лианга (`hypher`, язык из `lang`) с ограничениями
//! `hyphenate-limit-chars` раскладываются обратно в куски мягкими переносами U+00AD; слово с
//! ручными переносами не трогается.

use super::{Computed, Piece};
use unicode_properties::{GeneralCategory, GeneralCategoryGroup, UnicodeGeneralCategory};

/// `hyphens: auto` — расставить знаки мягкого переноса по слогоразделу.
///
/// Ставится ИМЕННО мягкий перенос (U+00AD): вся машинерия под него уже есть —
/// он даёт точку разрыва, сам ширины не имеет и на конце строки показывается
/// знаком из `hyphenate-character`. Образцы Лианга вшиты в `hypher` по языкам;
/// язык берётся из унаследованного атрибута `lang`.
///
/// Слово ищется по ТЕКСТУ ВСЕГО АБЗАЦА, а не по одному куску: разметка режет
/// слова где угодно (`<span>high</span>way`), и по кускам порознь слогораздел
/// давал бы другие точки, чем у целого слова — а тест требует ровно тех же
/// (`hyphens-span-002`). Найденные точки раскладываются обратно в тот кусок,
/// которому принадлежат.
///
/// Слово с уже расставленными вручную знаками не трогаем: разметка знает
/// лучше.
pub fn hyphenate_pieces(pieces: &mut [Piece]) {
    // Текст абзаца и карта «глобальное смещение → кусок». Не-текстовый кусок
    // слово РАЗРЫВАЕТ: картинка посреди букв словом их не делает.
    let mut whole = String::new();
    let mut map: Vec<(usize, usize)> = Vec::new(); // (начало в тексте, кусок)
    let mut any = false;
    for (i, p) in pieces.iter().enumerate() {
        match p {
            Piece::Text { text, style } => {
                if style.hyphens_auto == Some(true) {
                    any = true;
                }
                map.push((whole.len(), i));
                whole.push_str(text);
            }
            // Кусок ВНЕ потока места в строке не занимает и слова не рвёт:
            // текста абзаца он не составляет вовсе
            // (`hyphens-out-of-flow-002`: `high<span abspos>…</span>way` —
            // это по-прежнему одно слово `highway`).
            Piece::Overlay(..) => {}
            // А вот атомарная коробка (картинка, `inline-block`) в строке
            // стоит и соседство букв разрывает.
            Piece::Atom(_) => {
                map.push((whole.len(), usize::MAX));
                whole.push('\u{0}');
            }
        }
    }
    if !any {
        return;
    }
    let owner = |at: usize| -> usize {
        map.iter()
            .rev()
            .find(|(start, _)| *start <= at)
            .map_or(usize::MAX, |(_, i)| *i)
    };
    // Точки переноса: глобальное смещение. Собираем по всему тексту, вставляем
    // с конца — иначе ранние вставки сдвигают поздние смещения.
    let mut cuts: Vec<usize> = Vec::new();
    let mut at = 0usize;
    while at < whole.len() {
        let rest = &whole[at..];
        let Some(off) = rest.find(|c: char| c.is_alphabetic()) else {
            break;
        };
        let from = at + off;
        let len = whole[from..]
            .find(|c: char| !word_character(c))
            .unwrap_or(whole.len() - from);
        let word = &whole[from..from + len];
        at = from + len;
        // Правила переноса берём у куска, где слово началось: своего стиля у
        // слова нет, а разметка могла разрезать его посередине.
        let piece = owner(from);
        let Some(Piece::Text { style, .. }) = pieces.get(piece) else {
            continue;
        };
        if style.hyphens_auto != Some(true) {
            continue;
        }
        cuts.extend(points(word, style).into_iter().map(|offset| from + offset));
    }
    for cut in cuts.into_iter().rev() {
        let piece = owner(cut);
        let local = cut - map.iter().find(|(_, i)| *i == piece).map_or(0, |(s, _)| *s);
        if let Some(Piece::Text { text, .. }) = pieces.get_mut(piece)
            && local <= text.len()
            && text.is_char_boundary(local)
        {
            text.insert(local, '\u{00ad}');
        }
    }
}

fn word_character(c: char) -> bool {
    c.is_alphabetic()
        || c == '\u{00ad}'
        || matches!(c, '\'' | '\u{2019}')
        || c.general_category_group() == GeneralCategoryGroup::Mark
}

/// Author-supplied soft hyphens suppress dictionary points in this word only.
fn points(word: &str, style: &Computed) -> Vec<usize> {
    if word.contains('\u{00ad}') {
        return Vec::new();
    }
    let Some(lang) = style.lang.as_deref().and_then(language) else {
        return Vec::new();
    };
    let limits = style.hyphen_limit_chars.unwrap_or([None; 3]);
    let (before, after) = lang.bounds();
    let before = limits[1].unwrap_or(before).max(1);
    let after = limits[2].unwrap_or(after).max(1);
    if character_count(word) < limits[0].unwrap_or(5).max(before.saturating_add(after)) {
        return Vec::new();
    }
    let mut offset = 0;
    hypher::hyphenate_bounded(word, lang, 1, 1)
        .filter_map(|part| {
            let cut = offset;
            offset += part.len();
            (cut > 0
                && character_count(&word[..cut]) >= before
                && character_count(&word[cut..]) >= after)
                .then_some(cut)
        })
        .collect()
}

fn character_count(word: &str) -> usize {
    word.chars()
        .filter(|c| {
            c.general_category() != GeneralCategory::NonspacingMark
                && c.general_category_group() != GeneralCategoryGroup::Punctuation
        })
        .count()
}

fn language(tag: &str) -> Option<hypher::Lang> {
    let primary = tag.split('-').next()?.as_bytes();
    let [a, b] = primary else { return None };
    hypher::Lang::from_iso([a.to_ascii_lowercase(), b.to_ascii_lowercase()])
}

#[cfg(test)]
mod tests {
    //! Dictionary and conditional-hyphenation regression tests.

    use super::*;

    fn style(lang: &str) -> Computed {
        {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.lang = Some(lang.into());
            with_inherited
        }
    }

    #[test]
    fn dictionary_points_and_language_tags() {
        for (lang, word, expected) in [
            ("en-US", "example", vec![2, 4]),
            ("nl", "cafeetje", vec![2]),
            ("hu", "ablakok", vec![2, 4]),
        ] {
            assert_eq!(points(word, &style(lang)), expected, "{lang}: {word}");
        }
        assert!(points("example", &style("english")).is_empty());
        assert!(points("exam\u{00ad}ple", &style("en")).is_empty());
    }

    #[test]
    fn character_limits_filter_dictionary_points() {
        let mut style = style("en");
        style.hyphen_limit_chars = Some([None, Some(3), Some(2)]);
        assert_eq!(points("example", &style), [4]);
        style.hyphen_limit_chars = Some([Some(8), None, None]);
        assert!(points("example", &style).is_empty());
    }

    #[test]
    fn manual_priority_is_per_word_and_words_span_markup() {
        let make = |text: &str| Piece::Text {
            text: text.into(),
            style: {
                let mut with_inherited = Computed { ..style("en") };
                with_inherited.hyphens_auto = Some(true);
                crate::style::computed::BoxStyle::from(with_inherited)
            },
        };
        let mut pieces = [make("exam\u{ad}ple example"), make(" exam"), make("ple")];
        hyphenate_pieces(&mut pieces);
        let text: Vec<&str> = pieces
            .iter()
            .filter_map(|piece| {
                if let Piece::Text { text, .. } = piece {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            text,
            [
                "exam\u{ad}ple ex\u{ad}am\u{ad}ple",
                " ex\u{ad}am",
                "\u{ad}ple"
            ]
        );
    }

    #[test]
    fn minimum_lengths_exclude_nonspacing_marks_and_punctuation() {
        assert_eq!(character_count("tu\u{301}’a\u{300}n"), 4);
        assert_eq!(character_count("का"), 2);
    }
}
