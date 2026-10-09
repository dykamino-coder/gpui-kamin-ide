//! Разборщики значений: выравнивание, шрифт, url, пробелы, тени, border-shape, слои фона, calc-size.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

/// Функция картинки в начале слоя и хвост за её закрывающей скобкой.
pub(crate) fn split_image_func(v: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (&v[..=i], &v[i + 1..]);
                }
            }
            _ => {}
        }
    }
    (v, "")
}

/// Есть ли в записи длины в единицах шрифта (`em`, `rem`, `ex`, `ch`).
pub(crate) fn has_font_units(v: &str) -> bool {
    v.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .any(|t| {
            let unit = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-');
            unit.len() < t.len()
                && matches!(unit.to_ascii_lowercase().as_str(), "em" | "rem" | "ex" | "ch")
        })
}

/// Заменить длины в единицах шрифта на пиксели: `1em` → `16px`.
pub(crate) fn font_lengths_to_px(v: &str, em: f32, rem: f32, ex: f32, ch: f32) -> String {
    let mut out = String::with_capacity(v.len() + 8);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.is_empty() {
            return;
        }
        let unit_at = token
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .unwrap_or(token.len());
        let (num, unit) = token.split_at(unit_at);
        let k = match unit.to_ascii_lowercase().as_str() {
            "em" => Some(em),
            "rem" => Some(rem),
            "ex" => Some(ex),
            "ch" => Some(ch),
            _ => None,
        };
        match (k, num.parse::<f32>()) {
            (Some(k), Ok(n)) if unit_at > 0 => out.push_str(&format!("{}px", n * k)),
            _ => out.push_str(token),
        }
        token.clear();
    };
    for c in v.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            token.push(c);
        } else {
            flush(&mut token, &mut out);
            out.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

pub(crate) fn parse_align(v: &str) -> Option<Align> {
    align_keyword(v).unwrap_or(None)
}

/// Несёт ли значение выравнивания модификатор `safe` (css-align §5.3).
pub(crate) fn is_safe(v: &str) -> bool {
    v.split_whitespace().next() == Some("safe")
}

/// Значение выравнивания по css-align-3.
///
/// Исходов три, а не два. `Ok(Some)` — выравнивание задано; `Ok(None)` —
/// значение верное, но своего выравнивания не несёт (`normal`, `auto`), и поле
/// надо ОЧИСТИТЬ, чтобы решала раскладка; `Err(())` — объявление негодное,
/// и прежнее значение остаётся. Раньше эти три случая делились на два
/// по-разному у разных свойств: у `align-items` негодное значение сохраняло
/// прежнее, у `justify-items` — стирало его.
pub(crate) fn align_keyword(v: &str) -> Result<Option<Align>, ()> {
    let mut it = v.split_whitespace();
    let mut word = it.next().ok_or(())?;
    // `safe`/`unsafe` — что делать при переполнении области; сама позиция от
    // этого не меняется.
    if matches!(word, "safe" | "unsafe") {
        word = it.next().ok_or(())?;
    }
    Ok(match word {
        "center" => Some(Align::Center),
        // §anchor-center (только `align-self`/`justify-self`; у `*-items`
        // значение отброшено спекой — `apply` его там игнорирует).
        "anchor-center" => Some(Align::AnchorCenter),
        // `self-start`/`self-end` считаются по письму САМОГО элемента,
        // `start`/`end` — по письму контейнера (css-align-3 §6.2). Разница
        // видна, как только элемент несёт своё `direction`/`writing-mode`:
        // `flexbox-align-self-vert-002` ждёт `self-start` СПРАВА у элемента
        // с `direction: rtl`. Само значение остаётся физическим, а «мерить
        // по себе» помнится отдельным флагом `align_self_own_axis` — его
        // зеркалит `inline::inherit`, где известны письмо элемента И письмо
        // родителя.
        "start" | "flex-start" | "left" => Some(Align::Start),
        "end" | "flex-end" | "right" => Some(Align::End),
        "self-start" => Some(Align::Start),
        "self-end" => Some(Align::End),
        "stretch" => Some(Align::Stretch),
        // `first baseline` — обычное выравнивание по базовой линии. `last`
        // честно раскладке неизвестен, но для ОДНОСТРОЧНЫХ участников первая
        // и последняя базовые совпадают — суррогат первой ближе очистки
        // (flex-order-last-baseline; прежний None ронял участника в stretch).
        "baseline" | "first" | "last" => Some(Align::Baseline),
        // `normal` у растяжимого элемента даёт растяжение, а у замещаемого —
        // прижим к началу. Выбирает это сама раскладка, когда поле пусто.
        "normal" | "auto" => None,
        _ => return Err(()),
    })
}

pub(crate) fn parse_justify(v: &str) -> Option<Justify> {
    // `safe`/`unsafe` говорят, что делать при переполнении области; позиция
    // от этого не меняется, поэтому приставка снимается.
    let v = v
        .strip_prefix("safe ")
        .or_else(|| v.strip_prefix("unsafe "))
        .unwrap_or(v)
        .trim();
    match v {
        "center" => Some(Justify::Center),
        "flex-start" => Some(Justify::Start),
        "flex-end" => Some(Justify::End),
        // `left`/`right` физические; при письме слева направо они совпадают
        // с началом и концом строки.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: развести их в отдельные значения, чтобы при
        // rtl они НЕ переставлялись вместе с `start`/`end` (css-align-3 §4).
        // Правка верна по спеке, но полный свод обоих: 0 и 0 —
        // `flexbox_justifycontent-right-002` (8.53) держит не это.
        "start" => Some(Justify::WmStart),
        "end" => Some(Justify::WmEnd),
        "left" => Some(Justify::Left),
        "right" => Some(Justify::Right),
        "space-between" => Some(Justify::Between),
        "space-around" => Some(Justify::Around),
        "space-evenly" => Some(Justify::Evenly),
        "stretch" => Some(Justify::Stretch),
        _ => None,
    }
}

/// Одна грань размещения: `3`, `span 2`, `auto`.
/// Годно ли значение сокращения `background` целиком.
///
/// Негодное объявление ОТБРАСЫВАЕТСЯ, а не сбрасывает свои длинные свойства
/// (CSS 2.1 §4.1.7): `background: green` в одном правиле и `background: red\;`
/// (значение с экранированной точкой с запятой, то есть цвет `red;`) в
/// другом обязаны оставить фон зелёным. Разбор ниже намеренно снисходителен —
/// он берёт из записи всё, что узнал, — поэтому годность проверяется
/// отдельно, и только ею решается сброс (`escapes-002/014`, `keywords-000`).
pub(crate) fn background_shorthand_valid(v: &str) -> bool {
    if v.contains("gradient(") || v.contains("url(") {
        return true;
    }
    let mut any = false;
    for token in split_outside_parens(v) {
        // Запятая слоя (`none, none`) — не часть слова.
        let t = token.trim().trim_end_matches(',').trim();
        if t.is_empty() || t == "/" {
            continue;
        }
        any = true;
        let known = matches!(
            t,
            "none"
                | "transparent"
                | "initial"
                | "unset"
                | "revert"
                | "no-repeat"
                | "repeat"
                | "repeat-x"
                | "repeat-y"
                | "space"
                | "round"
                | "cover"
                | "contain"
                | "scroll"
                | "fixed"
                | "local"
                | "border-box"
                | "padding-box"
                | "content-box"
                | "text"
                | "left"
                | "right"
                | "top"
                | "bottom"
                | "center"
        ) || Len::parse(t).is_some()
            || Color::parse(t).is_some()
            || parse_image_color(t).is_some();
        if !known {
            return false;
        }
    }
    any
}

/// Голова сокращения `font` и семейство: семейство — хвост после размера.
/// Убрать пробелы вокруг косой черты: `50px / 1` → `50px/1`.
pub(crate) fn join_slash(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for part in v.split('/') {
        if !out.is_empty() {
            out.push('/');
            out.push_str(part.trim_start());
        } else {
            out.push_str(part.trim_end());
        }
    }
    out
}

pub(crate) fn split_font(v: &str) -> (&str, &str) {
    let mut end = 0;
    for token in split_outside_parens(v) {
        let at = v[end..].find(token.as_str()).map(|i| end + i).unwrap_or(end);
        end = at + token.len();
        if font_size_token(&token) {
            return (&v[..end], v[end..].trim());
        }
    }
    (v, "")
}

/// Кусок головы сокращения `font`, который задаёт КЕГЛЬ (быть может, с
/// `/высотой строки`). Голое число — ВЕС, а не кегль (§15.6: 100…900):
/// `Len::parse("900")` даёт точки, и `font: 900 2em Ahem` отдавал кегль
/// весу, а `2em Ahem` — семейству, после чего гибло всё
/// (`font-family-011`). Ключевые кегли и математические функции — тоже
/// кегль (§15.8; `font: calc(10 * 10px) sans-serif`, `font-148`).
pub(crate) fn font_size_token(token: &str) -> bool {
    let size = font_slash(token).map_or(token, |(s, _)| s);
    let lower = size.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large" | "xx-large"
            | "xxx-large" | "larger" | "smaller"
    ) || ["calc(", "min(", "max(", "clamp("].iter().any(|f| lower.starts_with(f))
    {
        return true;
    }
    size.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && (token.contains('/')
            || size == "0"
            || (Len::parse(size).is_some() && !size.chars().all(|c| c.is_ascii_digit())))
}

/// Косая черта ВНЕ скобок: `20px/1.5` делится, `calc(100px/2)` — нет.
pub(crate) fn font_slash(t: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, ch) in t.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '/' if depth == 0 => return Some((&t[..i], &t[i + 1..])),
            _ => {}
        }
    }
    None
}

/// Разбить значение по пробелам ВНЕ скобок: `rgba(0, 128, 0, .5)` —
/// один токен, а `split_whitespace` рассыпал его, и цвет пропадал
/// (`outline` с функциональным цветом).
pub(crate) fn split_ws_top(v: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, ch) in v.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ => {}
        }
        if ch.is_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&v[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(&v[st..]);
    }
    out
}

/// Разрывы сегмента в строке-маркере: ряд принудительных разрывов — один
/// пробел (css-text-3 §4.1.2, «Segment Break Transformation Rules»).
/// Строка знака обрыва (`text-overflow: <string>`, `block-ellipsis`): ряд
/// ПРИНУДИТЕЛЬНЫХ разрывов — один пробел. Принудительный разрыв по
/// css-text-3 §forced-line-break — сохранённый перевод строки и любой знак
/// классов UAX#14 BK/NL: VT, FF, NEL, LS, PS (Blink `line_truncator.cc`
/// `IsForcedLineBreak`/`SuppressLineBreaks`; `text-overflow-string-018…022`).
pub(crate) fn collapse_forced_breaks(text: &str) -> String {
    let forced = |c: char| {
        matches!(
            c,
            '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    };
    if !text.contains(forced) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if forced(ch) {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

pub(crate) fn collapse_segment_breaks(text: &str) -> String {
    if !text.contains(['\n', '\r']) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_break = false;
    for ch in text.chars() {
        if matches!(ch, '\n' | '\r') {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else {
            in_break = false;
            out.push(ch);
        }
    }
    out
}

pub(crate) fn unescape_content(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    // Экранированный перевод строки внутри строки CSS — ПРОДОЛЖЕНИЕ, а не
    // знак: CSS 2.1 §4.1.3 «the newline itself has to be escaped with a
    // backslash (\). The newline is subsequently removed from the string»
    // (css-syntax-3 §4.3.5: newline после `\` потребляется). `css::unescape`
    // общий с именами и оставлял `\n`; в `white-space: pre` он становился
    // жёстким разрывом, и кошка `content-173` рвалась лишними строками.
    // Остальные экранирования уходят в `unescape` как есть; `\` в конце
    // строки (EOF) пропадает.
    let mut joined = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            joined.push(ch);
            continue;
        }
        match it.next() {
            Some('\n') | Some('\u{c}') => {}
            Some('\r') => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
            }
            Some(c) => {
                joined.push('\\');
                joined.push(c);
            }
            None => {}
        }
    }
    crate::style::css::unescape(&joined)
}

/// `url(...)` из значения фона; кавычки внутри необязательны.
///
/// Имя записи регистронезависимо (§3.3), поэтому `URL(` и `Url(` — та же
/// запись; экранирование в нём разбор объявления уже снял. Конец ищется
/// с учётом кавычек и экранирования: закрывающая скобка внутри строки записи
/// не закрывает (§4.3.6). И искать `url(` внутри строки нельзя вовсе —
/// `content: "url(x)"` записью не является.
pub(crate) fn parse_url(v: &str) -> Option<String> {
    let mut at = 0usize;
    while at < v.len() {
        let ch = v[at..].chars().next().unwrap_or('\u{0}');
        match ch {
            '\\' => {
                at += ch.len_utf8();
                at += v[at..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            '"' | '\'' => {
                at += ch.len_utf8();
                at += crate::style::css::skip_string(&v[at..], ch);
                continue;
            }
            _ if crate::style::css::at_url(&v[at..]) => {
                let end = at + crate::style::css::skip_url(&v[at..]);
                // Обрыв на конце файла закрывает запись сам (§4.2): скобки
                // может не быть, и тогда резать последний знак нельзя, а
                // кавычка остаётся только открывающая (`uri-017`).
                let inner_end = if v[..end].ends_with(')') {
                    end - 1
                } else {
                    end
                };
                let inner = v[at + 4..inner_end.max(at + 4)].trim();
                let inner = match inner.chars().next() {
                    Some(q @ ('"' | '\'')) if inner.len() > 1 => {
                        let body = &inner[1..];
                        body.strip_suffix(q).unwrap_or(body)
                    }
                    _ => inner,
                };
                return (!inner.is_empty()).then(|| inner.to_string());
            }
            _ => at += ch.len_utf8(),
        }
    }
    None
}

/// Разбить значение по пробелам, НЕ заходя внутрь скобок.
///
/// `rgba(0, 0, 0, .5)` — это один токен, а не четыре: обычное деление по
/// пробелам разрывало функции с пробелами после запятых, и значение молча
/// пропадало.

pub(crate) fn split_outside_parens(v: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0usize;
    let mut cur = String::new();
    for ch in v.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Процентная смесь `calc(A% ± Bpx)` парой (доля, точки) — для свойств,
/// которые доли решают САМИ при отрисовке, зная размер коробки
/// (css-values-4 §10.9). Любая другая природа в сумме (`ch`, `vw`, `em`…) —
/// `None`: её здесь сложить не с чем, и запись, как прежде, не применяется.
pub(crate) fn pct_px_pair(t: &str) -> Option<(f32, f32)> {
    match Len::parse_mixed(t)? {
        Len::Calc(i) => crate::style::values::value::calc_get(i).pct_px(),
        _ => None,
    }
}

/// Кавычка вокруг имени шрифта: `font-family: "Segoe UI", sans-serif`.
pub(crate) fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

/// Семейство, которое подставляется за родовое имя.
///
/// Родовое имя — не шрифт, а разряд: в системе шрифтов его нет, и поиск по
/// нему кончается ничем (а в отладочной сборке — паникой). Подставляются те
/// же семейства, что берёт Chrome на Windows, иначе разметка набирается
/// умолчанием движка и шире эталона.
///
/// `monospace` в таблицу не входит: за него отвечает признак `monospace`, и
/// семейство под него выбирается позже — из тех, что на машине есть
/// (см. `metrics::mono_family`). Здесь он только не считается именем шрифта.
pub fn generic_family(lower: &str) -> Option<&'static str> {
    match lower {
        "system-ui" | "sans-serif" | "ui-sans-serif" | "ui-rounded" => Some(GENERIC_SANS),
        "serif" | "ui-serif" => Some("Times New Roman"),
        "cursive" => Some("Comic Sans MS"),
        "fantasy" => Some("Impact"),
        _ => None,
    }
}

/// Родовое имя семейства — разряд шрифта, а не шрифт.
pub(crate) fn is_generic(lower: &str) -> bool {
    generic_family(lower).is_some() || matches!(lower, "monospace" | "ui-monospace")
}

/// Семейство за родовое `sans-serif`; оно же — умолчание документа.
pub const GENERIC_SANS: &str = "Segoe UI";

pub(crate) fn parse_overflow(v: &str) -> Option<Overflow> {
    match v {
        "hidden" => Some(Overflow::Hidden),
        "clip" => Some(Overflow::Clip),
        // `overlay` — устаревший синоним `auto` (css-overflow-3 §overflow:
        // «legacy value alias of auto»; `overflow-overlay`).
        "scroll" | "auto" | "overlay" => Some(Overflow::Scroll),
        "visible" => Some(Overflow::Visible),
        _ => None,
    }
}

/// Опорная коробка `<geometry-box>` (css-masking-1 §1.3.1.1 плюс
/// `half-border-box` css-borders-4): 0 border, 1 margin, 2 padding,
/// 3 content, 4 half-border. У элемента с CSS-коробкой `fill-box` =
/// content-box, `stroke-box`/`view-box` = border-box.
pub(crate) fn geometry_box_kind(word: &str) -> Option<u8> {
    Some(match word.trim().to_ascii_lowercase().as_str() {
        "border-box" | "stroke-box" | "view-box" => 0,
        "margin-box" => 1,
        "padding-box" => 2,
        "content-box" | "fill-box" => 3,
        "half-border-box" => 4,
        _ => return None,
    })
}

/// `border-shape: [ <basic-shape> <geometry-box>? ]{1,2}`. Фигура — функция
/// со скобками, режется по ПАРНОЙ закрывающей (внутри `polygon(...)`
/// запятые, внутри `path('...')` — что угодно); слово коробки — следом за
/// ней. Хвост, не разобранный в две фигуры, делает значение недействительным.
pub(crate) fn parse_border_shape(v: &str) -> Option<BorderShape> {
    let mut items: Vec<(String, Option<u8>)> = Vec::new();
    let mut rest = v.trim();
    while !rest.is_empty() && items.len() < 2 {
        let open = rest.find('(')?;
        let mut depth = 0usize;
        let mut close = None;
        for (i, ch) in rest[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close?;
        let shape = rest[..=close].trim().to_string();
        // Прямоугольные фигуры пишутся через пробел (css-shapes-1 §basic-shape:
        // `rect( [ <length-percentage> | auto ]{4} … )`, так же `inset()` и
        // `xywh()`); запятая делает всё объявление недействительным, и
        // `border-shape` остаётся `none` — Blink `ConsumeBasicShapeRect`
        // (`css_parsing_utils.cc:651-668`) берёт четыре длины подряд без
        // запятой. Прежде `rect(0, 0, 100%, 100%)` разбирался в пустой
        // прямоугольник, и маска фигуры прятала коробку целиком
        // (border-shape-inset-shadow-blur, -negative-spread: пустая страница).
        let head = shape[..open].trim_start().to_ascii_lowercase();
        if matches!(head.as_str(), "rect" | "inset" | "xywh") && shape.contains(',') {
            return None;
        }
        rest = rest[close + 1..].trim_start();
        let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let bx = geometry_box_kind(&rest[..word_end]);
        if bx.is_some() {
            rest = rest[word_end..].trim_start();
        }
        items.push((shape, bx));
    }
    if !rest.is_empty() {
        return None;
    }
    let mut it = items.into_iter();
    let (outer, outer_box) = it.next()?;
    Some(match it.next() {
        Some((inner, inner_box)) => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(0),
            inner: Some((inner, inner_box.unwrap_or(2))),
        },
        None => BorderShape {
            outer,
            outer_box: outer_box.unwrap_or(4),
            inner: None,
        },
    })
}

/// Параметр суперэллипса из одного значения `<corner-shape-value>`
/// (css-borders-4 §corner-shaping): ключевые слова — их числовые
/// эквиваленты по спеке, `superellipse(<number> | infinity | -infinity)` —
/// само число. `None` — не форма угла (запись отбрасывается).
pub(crate) fn corner_shape_param(tok: &str) -> Option<f32> {
    let t = tok.trim().to_ascii_lowercase();
    Some(match t.as_str() {
        "round" => 1.0,
        "squircle" => 2.0,
        "square" => f32::INFINITY,
        "bevel" => 0.0,
        "scoop" => -1.0,
        "notch" => f32::NEG_INFINITY,
        _ => {
            let inner = t.strip_prefix("superellipse(")?.strip_suffix(')')?.trim();
            match inner {
                "infinity" => f32::INFINITY,
                "-infinity" => f32::NEG_INFINITY,
                n => n.parse::<f32>().ok()?,
            }
        }
    })
}

/// `corner-shape: a [b [c [d]]]` → K по углам tl/tr/br/bl — раскладка та же,
/// что у `border-radius` (§corner-shaping-shorthand). Функции со скобками
/// внутри пробелов не содержат, поэтому режем по пробелам.
pub(crate) fn corner_shape_shorthand(raw: &str) -> Option<[f32; 4]> {
    let v: Vec<f32> = raw
        .split_whitespace()
        .map(corner_shape_param)
        .collect::<Option<Vec<_>>>()?;
    Some(match v.len() {
        1 => [v[0]; 4],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        4 => [v[0], v[1], v[2], v[3]],
        _ => return None,
    })
}

/// Смещение первой запятой ВНЕ вложенных скобок.
/// Раскрытие записи в четыре стороны: 1 значение — все, 2 — верт/гориз,
/// 3 — верх/гориз/низ, 4 — по часовой. `None`, если разобрать не удалось.
pub(crate) fn four<T: Copy>(words: &[&str], one: impl Fn(&str) -> Option<T>) -> Option<[T; 4]> {
    let v: Vec<T> = words.iter().filter_map(|w| one(w)).collect();
    match v.len() {
        1 => Some([v[0]; 4]),
        2 => Some([v[0], v[1], v[0], v[1]]),
        3 => Some([v[0], v[1], v[2], v[1]]),
        4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    }
}

/// Фоновые свойства со списком слоёв (css-backgrounds-3 §2.1).
pub(crate) const BG_LIST_KEYS: [&str; 8] = [
    "background",
    "background-image",
    "background-size",
    "background-position",
    "background-repeat",
    "background-origin",
    "background-clip",
    "background-attachment",
];

/// Слои фона: значение режется по запятым ВНЕ скобок.
///
/// Запятая внутри `rgba(…)` или `linear-gradient(…)` слой не кончает, поэтому
/// делить строку простым `split(',')` нельзя.
pub(crate) fn background_layers(v: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = v;
    while let Some(at) = top_level_comma(rest) {
        out.push(rest[..at].trim());
        rest = &rest[at + 1..];
    }
    out.push(rest.trim());
    out
}

pub(crate) fn top_level_comma(inner: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Ширина обводки: ключевые слова и любые шрифтовые/абсолютные длины.
pub(crate) fn outline_width_of(v: &str) -> Option<Len> {
    match v {
        "thin" => Some(Len::Px(1.0)),
        "medium" => Some(Len::Px(3.0)),
        "thick" => Some(Len::Px(5.0)),
        _ => Len::parse(v).filter(|l| !matches!(l, Len::Pct(_))),
    }
}

/// Список `font-feature-settings` (css-fonts-4 §7.1): `normal` — пустой,
/// иначе `<opentype-tag> [ <integer [0,∞]> | on | off ]?` через запятую.
/// Тег — СТРОКА ровно из четырёх печатных ASCII-знаков в ЛЮБЫХ кавычках:
/// `'liga' off` прежде терялся целиком (`trim_matches('"')` оставлял шесть
/// знаков — `font-feature-resolution-001/002`). Тот же разбор нужен
/// дескриптору в `@font-face`.
pub(crate) fn feature_list(v: &str) -> Option<Vec<(String, u32)>> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("normal") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for token in v.split(',') {
        let token = token.trim();
        let q = token.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let rest = &token[1..];
        let end = rest.find(q)?;
        let tag = &rest[..end];
        if tag.len() != 4 || !tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return None;
        }
        let on = match rest[end + 1..].trim() {
            "" | "on" => 1,
            "off" => 0,
            n => n.parse::<u32>().ok()?,
        };
        out.push((tag.to_string(), on));
    }
    Some(out)
}

/// Годное имя семейства: строка в кавычках либо ряд идентификаторов.
///
/// Идентификатор по §4.1.3 начинается с буквы, подчёркивания, не-ASCII знака
/// или экранирования; за ними идут буквы, цифры, дефисы, подчёркивания и
/// экранирования. Цифра первой запрещена, дефис с цифрой следом — тоже.
pub(crate) fn family_name_ok(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    if (part.starts_with('"') && part.ends_with('"') && part.len() >= 2)
        || (part.starts_with('\'') && part.ends_with('\'') && part.len() >= 2)
    {
        return true;
    }
    part.split_whitespace().all(|word| {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let head_ok = first.is_alphabetic()
            || first == '_'
            || first == '\\'
            || first as u32 >= 0xa0
            || (first == '-'
                && word
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_alphabetic() || c == '_' || c as u32 >= 0xa0));
        head_ok
            && word.chars().all(|c| {
                c.is_alphanumeric() || c == '-' || c == '_' || c == '\\' || c as u32 >= 0xa0
            })
    })
}

/// Довод `fit-content(<length-percentage>)` (css-sizing-3 §4.1): только
/// точки или доля, неотрицательные. Прочее (`em`, `calc`) — `None`, и
/// значение ведёт себя, как прежде, голым `fit-content`.
pub(crate) fn fit_content_arg(v: &str) -> Option<Len> {
    let lower = v.trim().to_ascii_lowercase();
    let arg = lower.strip_prefix("fit-content(")?.strip_suffix(')')?;
    match Len::parse(arg) {
        Some(l @ (Len::Px(n) | Len::Pct(n))) if n >= 0.0 => Some(l),
        _ => None,
    }
}

/// Разобранный `calc-size()`.
pub(crate) enum CalcSize {
    /// Основа — длина: значение известно сразу.
    Fixed(f32),
    /// Основа — ключевое слово размера: `(mul, add, max, min)` над ним.
    Over((f32, f32, f32, f32)),
}

/// `calc-size(<basis>, <calc-sum>)` (css-values-5 §calc-size,
/// `csswg-drafts/css-values-5/Overview.bs`). Понимаются линейные выражения
/// над `size` (`size`, `size ± L`, `size * k`, `k * size`, `size / k`, их
/// суммы) и `min(size, L)` / `max(size, L)`; длины — в точках. Основа —
/// `auto`, `fit-content`, `min-content`, `max-content`, `content` или длина;
/// вложенный `calc-size()` и проценты не понимаются — объявление роняется.
pub(crate) fn calc_size_arg(v: &str) -> Option<CalcSize> {
    let inner = v.trim().strip_prefix("calc-size(")?.strip_suffix(')')?;
    let (basis, expr) = inner.split_once(',')?;
    let basis = basis.trim();
    let mut expr: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(e) = expr.strip_prefix("calc(").and_then(|e| e.strip_suffix(')')) {
        expr = e.to_string();
    }
    let px = |t: &str| t.strip_suffix("px").and_then(|n| n.parse::<f32>().ok());
    let f = if let Some(a) = expr.strip_prefix("min(size,").and_then(|e| e.strip_suffix(')')) {
        (1.0, 0.0, px(a)?, f32::MIN)
    } else if let Some(a) = expr.strip_prefix("max(size,").and_then(|e| e.strip_suffix(')')) {
        (1.0, 0.0, f32::MAX, px(a)?)
    } else {
        // Сумма членов: `size`, `size*k`, `k*size`, `size/k`, `L`.
        let (mut mul, mut add) = (0.0f32, 0.0f32);
        let mut rest = expr.as_str();
        let mut sign = 1.0f32;
        if let Some(r) = rest.strip_prefix('-') {
            sign = -1.0;
            rest = r;
        }
        loop {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let term = &rest[..end];
            if term == "size" {
                mul += sign;
            } else if let Some(k) = term.strip_prefix("size*").or_else(|| term.strip_suffix("*size")) {
                mul += sign * k.parse::<f32>().ok()?;
            } else if let Some(k) = term.strip_prefix("size/") {
                mul += sign / k.parse::<f32>().ok()?;
            } else {
                add += sign * px(term)?;
            }
            if end == rest.len() {
                break;
            }
            sign = if rest.as_bytes()[end] == b'-' { -1.0 } else { 1.0 };
            rest = &rest[end + 1..];
        }
        (mul, add, f32::MAX, f32::MIN)
    };
    if let Some(b) = px(basis) {
        let (mul, add, max, min) = f;
        return Some(CalcSize::Fixed((b * mul + add).min(max).max(min).max(0.0)));
    }
    matches!(basis, "auto" | "fit-content" | "min-content" | "max-content" | "content")
        .then_some(CalcSize::Over(f))
}

/// `object-view-box: none | <basic-shape-rect>` — `inset()`, `rect()`,
/// `round <'border-radius'>` of `inset()`/`rect()`/`xywh()` (css-shapes-1
/// §basic-shape-rect): one radius for all corners and both axes — every
/// listed value, before and after `/`, equal. `20px / 20px` is that radius;
/// unequal corners are not representable here and stay unrounded.
pub(crate) fn uniform_round(r: &str) -> Option<Len> {
    let mut it = r.split(|c: char| c == '/' || c.is_whitespace()).filter(|t| !t.is_empty());
    let first = Len::parse(it.next()?)?;
    it.all(|t| Len::parse(t) == Some(first)).then_some(first)
}

/// `xywh()` (css-images-4 §object-view-box; css-shapes-1 §basic-shape-rect).
/// Длины — точки или доли; `inset` с 1-3 значениями раскрывается как поля.
pub(crate) fn parse_view_box(v: &str) -> Option<(u8, [Len; 4])> {
    let v = v.trim();
    let (kind, inner) = if let Some(r) = v.strip_prefix("inset(") {
        (0u8, r)
    } else if let Some(r) = v.strip_prefix("rect(") {
        (1u8, r)
    } else if let Some(r) = v.strip_prefix("xywh(") {
        (2u8, r)
    } else {
        return None;
    };
    let inner = inner.strip_suffix(')')?;
    let parts: Vec<Len> = inner
        .split_whitespace()
        .map(|t| match Len::parse(t) {
            Some(l @ (Len::Px(_) | Len::Pct(_))) => Some(l),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let four = match (kind, parts.as_slice()) {
        (_, [a, b, c, d]) => [*a, *b, *c, *d],
        (0, [a]) => [*a, *a, *a, *a],
        (0, [a, b]) => [*a, *b, *a, *b],
        (0, [a, b, c]) => [*a, *b, *c, *b],
        _ => return None,
    };
    Some((kind, four))
}

pub(crate) fn assign_size(slot: &mut Option<Len>, v: &str) {
    // Смесь «доля ± точки» доживает индексом (`parse_mixed`): раскладка
    // складывает её сама (`DefiniteLength::Calc`, css-values-4 §10.9).
    // Прежде `calc(50% - 3px)` роняло объявление (`calc-width-block-1`).
    let parsed = size_range::parse(v);
    // `none` снимает предел (§10.4) — слот гаснет по праву. Прочая
    // неразборная запись объявление роняет: слот сохраняет прежнее значение,
    // а не гаснет (§4.2).
    if v.trim().eq_ignore_ascii_case("none") {
        *slot = None;
        return;
    }
    if let Some(l) = parsed {
        *slot = Some(l);
    }
}

/// Годна ли запись `box-shadow` ЦЕЛИКОМ (css-backgrounds-3 §7.1:
/// `none | <shadow>#`; тень — 2-4 длины, не больше одного цвета и одного
/// `inset`). `none` внутри списка делает декларацию негодной.
pub(crate) fn box_shadow_valid(v: &str) -> bool {
    if v.trim().eq_ignore_ascii_case("none") {
        return true;
    }
    crate::style::css::split_args(v).iter().all(|s| {
        let (mut lens, mut colours, mut insets) = (0, 0, 0);
        for token in tokenize_shadow(s) {
            match Len::parse(&token) {
                Some(Len::Pct(_)) => return false,
                Some(_) => lens += 1,
                None if token.eq_ignore_ascii_case("inset") => insets += 1,
                None if token.eq_ignore_ascii_case("currentcolor")
                    || Color::parse(&token).is_some() =>
                {
                    colours += 1
                }
                None => return false,
            }
        }
        (2..=4).contains(&lens) && colours <= 1 && insets <= 1
    })
}

pub(crate) fn parse_shadows(v: &str) -> Vec<Shadow> {
    let mut out = vec![];
    for s in crate::style::css::split_args(v) {
        // Внутренние тени не рисуются — но синтаксис их проверяется: одна
        // невалидная тень роняет ВСЮ декларацию (css-backgrounds-3 §7.2).
        let inner = s.contains("inset");
        let mut lens = vec![];
        let mut color = None;
        for token in tokenize_shadow(&s) {
            match Len::parse(&token) {
                Some(Len::Px(px)) => lens.push(Some(px)),
                // calc() из абсолютных единиц уже свёрнут в px; примесь
                // процентов невалидна для тени.
                Some(Len::Calc(id)) => {
                    let sum = crate::style::values::value::calc_get(id);
                    if sum.pct != 0.0 {
                        return vec![];
                    }
                    // Шрифтовые/оконные слагаемые здесь не резолвятся —
                    // тень пропускается, но декларация остаётся валидной.
                    let bare = crate::style::values::value::Sum {
                        px: 0.0,
                        pct: 0.0,
                        ..sum
                    };
                    lens.push((bare == crate::style::values::value::Sum::default()).then_some(sum.px));
                }
                Some(Len::Pct(_)) => return vec![],
                // em/vh и прочее — валидно, но контекста тут нет.
                Some(_) => lens.push(None),
                None => {
                    if let Some(c) = Color::parse(&token) {
                        color = Some(c);
                    } else if token == "currentcolor" {
                        // Явный `currentColor` = как отсутствие цвета:
                        // метка a = -1 дорешается при слиянии стилей.
                    } else if token != "inset" {
                        return vec![];
                    }
                }
            }
        }
        // Длин бывает от двух до четырёх (§7.2).
        if lens.len() < 2 || lens.len() > 4 {
            return vec![];
        }
        if inner || lens.iter().any(Option::is_none) {
            continue;
        }
        let lens: Vec<f32> = lens.into_iter().flatten().collect();
        out.push(Shadow {
            x: lens[0],
            y: lens[1],
            blur: lens.get(2).copied().unwrap_or(0.0),
            spread: lens.get(3).copied().unwrap_or(0.0),
            // Тень без цвета берёт currentColor (css-backgrounds-3
            // §7): цвет текста известен только после слияния стилей,
            // отрицательная альфа — метка «дорешать там».
            color: color.unwrap_or(Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: -1.0,
            }),
        });
    }
    out
}

/// Разбиение тени на токены: `rgba(0, 0, 0, .4)` — один токен, а не четыре.
pub(crate) fn tokenize_shadow(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch)
            }
            ')' => {
                depth -= 1;
                cur.push(ch)
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Множитель роста/сжатия гибкого элемента: `<number>` или `calc()` из чисел
/// (css-values-4 §10.1), неотрицательный. `calc(infinity)` (§10.7.1) —
/// наибольшее представимое: здесь — конечное большое, чтобы сумма
/// множителей и доли свободного места не уходили в бесконечность и `NaN`
/// (`flex-grow-009`: `flex: calc(infinity) 0 0px` забирает всё место).
pub(crate) fn flex_factor(v: &str) -> Option<f32> {
    let g = crate::style::values::value::number(v)?;
    if g.is_nan() || g < 0.0 {
        return None;
    }
    Some(g.min(1.0e18))
}

/// `stretch` and its prefixed spellings (css-sizing-4 §4.1).
pub(crate) fn stretch_keyword(v: &str) -> bool {
    let v = v.trim();
    v.eq_ignore_ascii_case("stretch")
        || v.eq_ignore_ascii_case("-webkit-fill-available")
        || v.eq_ignore_ascii_case("-moz-available")
}
