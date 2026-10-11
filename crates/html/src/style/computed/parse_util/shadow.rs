//! Разбор теней box-shadow/text-shadow: валидность и список теней.

use super::*;

/// Годна ли запись `box-shadow` ЦЕЛИКОМ (css-backgrounds-3 §7.1:
/// `none | <shadow>#`; тень — 2-4 длины, не больше одного цвета и одного
/// `inset`). `none` внутри списка делает декларацию негодной.
pub(in crate::style::computed) fn box_shadow_valid(v: &str) -> bool {
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

pub(in crate::style::computed) fn parse_shadows(v: &str) -> Vec<Shadow> {
    let mut out = vec![];
    for s in crate::style::css::split_args(v) {
        // Внутренние тени не рисуются — но синтаксис их проверяется: одна
        // невалидная тень роняет ВСЮ декларацию (css-backgrounds-3 §7.2).
        let inner = s.contains("inset");
        let mut lens = vec![];
        let mut color = None;
        for token in tokenize_shadow(s) {
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
                    lens.push(
                        (bare == crate::style::values::value::Sum::default()).then_some(sum.px),
                    );
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
fn tokenize_shadow(s: &str) -> Vec<String> {
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
