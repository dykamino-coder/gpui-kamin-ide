//! Apply the font shorthand while retaining its variant and reset behavior.

use super::{Computed, font_size_token, font_slash, join_slash, split_font, split_outside_parens};
use crate::value::Len;

pub(super) fn apply(style: &mut Computed, v: &str) {
    // Все части сокращения наследуемые: `inherit` для них — это
    // «своего значения нет», то есть ОЧИСТКА слота. Подстановка
    // родительского значения тут не работает: ниже по разбору
    // `own.font_size.or(parent.font_size)` вернул бы свой прежний
    // (`font: 0 Ahem; font: inherit` оставлял нулевой кегль).
    if v == "inherit" {
        style.font_size = None;
        style.font_kerning = None;
        style.font_alternates = None;
        style.font_family = None;
        style.font_families = None;
        style.font_weight = None;
        style.font_weight_step = 0;
        style.italic = None;
        style.oblique = None;
        style.line_height = None;
        return;
    }
    // `font: 50px / 1 Ahem` — вокруг косой черты разрешены пробелы,
    // а кегль с высотой строки обязаны разбираться одним куском:
    // иначе «/ 1 Ahem» уезжало в семейство шрифта целиком.
    let value = join_slash(v);
    let (head, family) = split_font(&value);
    // Неизвестное слово в голове сокращения тоже валит его целиком
    // (§4.2): `font: bold highlighted 100% serif` не задаёт ни
    // начертания, ни кегля (`c71-fwd-parsing-003`). Слова головы —
    // это начертание, наклон, вариант, растяжение и системные
    // ключевые слова; всё прочее начинается с цифры или точки.
    let head_word = |t: &str| {
        matches!(
            t,
            "normal"
                | "italic"
                | "oblique"
                | "small-caps"
                | "bold"
                | "bolder"
                | "lighter"
                | "ultra-condensed"
                | "extra-condensed"
                | "condensed"
                | "semi-condensed"
                | "semi-expanded"
                | "expanded"
                | "extra-expanded"
                | "ultra-expanded"
                | "xx-small"
                | "x-small"
                | "small"
                | "medium"
                | "large"
                | "x-large"
                | "xx-large"
                | "larger"
                | "smaller"
                | "caption"
                | "icon"
                | "menu"
                | "message-box"
                | "small-caption"
                | "status-bar"
        ) || t.starts_with(|c: char| c.is_ascii_digit() || c == '.')
    };
    if split_outside_parens(head)
        .iter()
        .any(|t| !head_word(&t.to_ascii_lowercase()) && !font_size_token(t))
    {
        return;
    }
    // Недействительная часть валит СОКРАЩЕНИЕ целиком (§4.2):
    // `font: 4em/-2em serif` не задаёт ни кегля, ни семейства
    // (`font-146`). Проверка идёт до записи любого куска.
    if head.split_whitespace().any(|t| {
        t.split_once('/').is_some_and(|(_, lh)| {
            let neg = |l: &Len| {
                matches!(
                    l,
                    Len::Px(v) | Len::Em(v) | Len::Pct(v) | Len::Ex(v) | Len::Ch(v)
                        if *v < 0.0
                )
            };
            lh.parse::<f32>().is_ok_and(|m| m < 0.0) || Len::parse(lh).as_ref().is_some_and(neg)
        })
    }) {
        return;
    }
    // Сокращение сперва сбрасывает ВСЕ свои части к начальным
    // значениям (CSS 2.1 §15.8), и только потом пишет названные.
    // Пустой слот у нас — «наследовать», поэтому сброс явный:
    // `normal` у веса, наклона и высоты строки (`Len::Auto` —
    // метка `normal`, см. `"line-height"`). Без него
    // `p { font: 16px serif }` под `html { font: 20px/1 Ahem }`
    // наследовал `line-height: 1` (`numbers-units-018`), а
    // `em { font: 1em/1 Ahem }` — курсив UA-листа (`c42-ibx-ht-000`).
    style.italic = Some(false);
    style.oblique = Some(false);
    style.font_weight = Some(400);
    style.font_weight_step = 0;
    style.font_kerning = Some(2);
    style.font_alternates = Some(crate::fonts::alternates::normal());
    style.line_height = Some(Len::Auto);
    for token in split_outside_parens(head) {
        let t = token.as_str();
        match t.to_ascii_lowercase().as_str() {
            "italic" => style.italic = Some(true),
            "oblique" => {
                style.italic = Some(true);
                style.oblique = Some(true);
            }
            "small-caps" => style.apply_one("font-variant-caps", "small-caps"),
            "bold" | "bolder" | "lighter" => style.apply_one("font-weight", t),
            // Кегль — и `0` (`font: 0 Ahem`: вес 0 зацикливал
            // подбор шрифта, vars-font-shorthand-001).
            _ if font_size_token(t) => match font_slash(t) {
                Some((size, lh)) => {
                    style.apply_one("font-size", size);
                    style.apply_one("line-height", lh);
                }
                None => style.apply_one("font-size", t),
            },
            _ if t.starts_with(|c: char| c.is_ascii_digit()) => {
                style.font_weight = t.parse().ok().filter(|w| (1..=1000).contains(w));
            }
            _ => {}
        }
    }
    if !family.is_empty() {
        style.apply_one("font-family", family);
    }
}
