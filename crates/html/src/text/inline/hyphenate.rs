//! Переносы: hyphenate_pieces и места разрыва.

use crate::text::inline::*;

/// `hyphens: auto` — расставить знаки мягкого переноса по слогоразделу.
///
/// Ставится ИМЕННО мягкий перенос (U+00AD): вся машинерия под него уже есть —
/// он даёт точку разрыва, сам ширины не имеет и на конце строки показывается
/// знаком из `hyphenate-character`. Образцы Лианга вшиты в `hypher` по языкам;
/// язык берётся из атрибута `lang`, без него — английский.
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
    if !any || whole.contains('\u{00ad}') {
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
            .find(|c: char| !c.is_alphabetic())
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
        // Без объявленного языка не переносим ВОВСЕ: образцы слогораздела
        // у каждого языка свои, и угаданный язык рвал бы слова не там.
        // Так же решает и спецификация (`hyphens-auto-001`: без `lang`
        // ни одно слово не разрывается).
        let Some(lang) = style
            .lang
            .as_deref()
            .and_then(|l| {
                let code = l.as_bytes();
                (code.len() >= 2)
                    .then(|| [code[0].to_ascii_lowercase(), code[1].to_ascii_lowercase()])
            })
            .and_then(hypher::Lang::from_iso)
        else {
            continue;
        };
        let mut pos = from;
        let mut first = true;
        for part in hypher::hyphenate(word, lang) {
            if !first {
                cuts.push(pos);
            }
            pos += part.len();
            first = false;
        }
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

/// `word-break: break-all` и родня: перенос разрешён внутри слова.
///
/// Перенос в GPUI идёт по границам слов, и другого рычага нет. Обходной путь —
/// вставить между символами нулевой пробел: он не рисуется и ширины не имеет,
/// но переносчик считает его законной точкой разрыва. Так длинный
/// нечленораздельный токен (хеш, путь, ссылка) перестаёт распирать колонку.
pub(crate) fn breakable(text: &str, style: &Computed) -> String {
    // `hyphens`: мягкий перенос (U+00AD) — законная точка разрыва, но
    // переносчик строк его таковой не считает. Меняем на нулевой пробел:
    // он и есть разрешение разорвать слово. Оговорка: дефис на месте
    // разрыва не рисуется — своего глифа у переноса в конвейере нет.
    // Умолчание CSS — `manual`: по мягкому переносу рвать МОЖНО. Убирает
    // его только явное `hyphens: none`.
    // Соединитель гроздей (U+034F) невидим и ширины не имеет: его дело —
    // запретить разрыв между соседями. До шейпера он доезжать не должен —
    // портит метрику последней строки
    // (`line-break-anywhere-overrides-uax-behavior-011`: коробка ниже на шесть
    // точек). Но СТИРАТЬ его нельзя: класс знака (GL по UAX-14) читается
    // позже — `glue_atoms` смотрит на последний знак куска и по нему решает,
    // клеить ли кусок с атомарной коробкой (css-text-3 §5.1). После стирания
    // куску `A\u{034f}` оставалось `A`, склейки не было, и ряд рвал слово
    // (`line-breaking-atomic-016/017`). Словосоединитель U+2060 такой же
    // невидимый и нулевой, класс WJ запрещает перенос с обеих сторон, а
    // метрику не портит: замерено на всех трёх зелёных парах корпуса с U+034F
    // (`-011` 0.04 → 0.04, `-012` 0.05 → 0.05, `zwnj-renders-invisible`
    // 0.00 → 0.00).
    let text = &text.replace('\u{034f}', "\u{2060}");
    // Принудительные разрывы Юникода — подача страницы, вертикальная
    // табуляция, NEL, разделители строки и абзаца. Набору их отдавать нельзя:
    // строка с ними шейпится в НУЛЕВУЮ ширину, и весь абзац пропадает
    // (`line-breaking-022`). Заменяются переводом строки — он и значит
    // «строка кончилась».
    let text = &text.replace(
        ['\u{000b}', '\u{000c}', '\u{0085}', '\u{2028}', '\u{2029}'],
        "\n",
    );
    // Мягкий перенос остаётся СВОИМ знаком, когда абзац считает раскладку сам:
    // переносчик UAX-14 знает его как точку разрыва, а на месте разрыва
    // рисуется знак переноса (`hyphenate-character`). Замена нулевым пробелом
    // нужна только ЧУЖОМУ набору — он о мягком переносе не знает.
    let owned = if style.hyphenate == Some(false) {
        text.replace('\u{00ad}', "")
    } else if crate::lines::rules(style).is_some() {
        text.to_string()
    } else {
        text.replace('\u{00ad}', "\u{200b}")
    };
    // Дальше идут подсказки переносчику GPUI. Своя строчная раскладка те же
    // правила считает сама и по НАСТОЯЩЕМУ тексту, поэтому подсказки ей
    // только мешают: невидимый знак становится лишней точкой разрыва и
    // сдвигает границы прогонов.
    if crate::lines::rules(style).is_some() {
        return owned;
    }
    // `white-space: break-spaces`: после КАЖДОГО сохранённого пробела есть
    // точка разрыва, а сам пробел остаётся в строке и занимает место. Нулевой
    // пробел сразу за ним и означает ровно это: разрыв разрешён здесь, и
    // предыдущий пробел уходит в измеряемую часть строки, а не свисает.
    let owned = if style.break_after_spaces == Some(true) {
        owned.replace(' ', " \u{200b}")
    } else {
        owned
    };
    // `word-break: keep-all`: иероглифы переносятся только по пробелам. Между
    // знаками ставится словосоединитель — он и означает «здесь не рвать», и
    // переносчик его уважает (класс WJ по UAX-14).
    if style.keep_all == Some(true) {
        let mut out = String::with_capacity(owned.len() * 2);
        let mut prev: Option<char> = None;
        for ch in owned.chars() {
            if let Some(p) = prev
                && !p.is_whitespace()
                && !ch.is_whitespace()
            {
                out.push('\u{2060}');
            }
            out.push(ch);
            prev = Some(ch);
        }
        return out;
    }
    if style.break_anywhere != Some(true) {
        return owned;
    }
    let text = owned.as_str();
    // `line-break: anywhere` рвёт где угодно, в том числе рядом с пробелом;
    // `word-break: break-all` — только внутри слова, поэтому перед пробелом
    // точку разрыва не ставит.
    let anywhere = style.break_anywhere_strict == Some(true);
    let mut out = String::with_capacity(text.len() * 2);
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && (anywhere || !ch.is_whitespace()) {
            out.push('\u{200b}');
        }
        out.push(ch);
    }
    out
}
