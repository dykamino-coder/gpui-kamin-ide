//! Градиенты и image-set: разбор, угол, растр, image-resolution.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

/// Выбор кандидата `image-set()` (css-images-4 §2.5).
///
/// * `None` — запись НЕГОДНА (отрицательное разрешение числом, вложенный
///   `image-set()`, чужое слово): объявление отбрасывается, прежнее живёт;
/// * `Some(None)` — запись годна, но пригодных кандидатов нет: «invalid image»;
/// * `Some(Some(src))` — СЫРАЯ запись выбранного `<image>` (`url(...)`,
///   градиент); строка-адрес оборачивается в `url(...)` (§2.5: «Each
///   `<string>` inside image-set() represents a `<url>`»).
pub(super) fn image_set_pick(inner: &str) -> Option<Option<String>> {
    let mut options: Vec<(String, f32)> = vec![];
    for cand in crate::style::css::split_args(inner) {
        let mut image: Option<String> = None;
        let mut res: Option<f32> = None;
        let mut type_ok = true;
        for token in split_outside_parens(cand.trim()) {
            let low = token.to_ascii_lowercase();
            if let Some(body) = low.strip_prefix("type(") {
                // Неподдержанный тип снимает КАНДИДАТА, а не запись (§2.5).
                // Список — форматы, которые читает `background::decode`.
                let mime = body.trim_end_matches(')').trim().trim_matches(is_quote);
                type_ok &= matches!(
                    mime,
                    "image/png"
                        | "image/jpeg"
                        | "image/gif"
                        | "image/webp"
                        | "image/bmp"
                        | "image/svg+xml"
                );
                continue;
            }
            if let Some(r) = image_resolution(&low) {
                // Отрицательное ЧИСЛО вне диапазона по определению — ошибка
                // разбора. Из `calc()` оно приходит вычисленным: запись годна,
                // непригоден только кандидат (`negative-resolution-3`).
                if r < 0.0 && !low.starts_with("calc(") {
                    return None;
                }
                res = Some(r);
                continue;
            }
            if image.is_none() {
                if low.starts_with("image-set(") || low.starts_with("-webkit-image-set(") {
                    return None;
                }
                if token.len() >= 2 && token.starts_with(is_quote) {
                    image = Some(format!("url({token})"));
                    continue;
                }
                if low.contains('(') {
                    image = Some(token.clone());
                    continue;
                }
            }
            return None;
        }
        let Some(image) = image else { continue };
        let res = res.unwrap_or(1.0);
        // Шаг 1 (тип), нулевая/отрицательная плотность (картинке не из чего
        // взять природный размер) и шаг 2 (дубль разрешения среди оставшихся).
        if !type_ok || res <= 0.0 || options.iter().any(|(_, r)| *r == res) {
            continue;
        }
        options.push((image, res));
    }
    // Шаг 4 отдан UA: наименьшее разрешение, которого хватает на плотность
    // 1x, иначе наибольшее (Blink `CSSImageSetValue::GetBestOption`).
    let best = options
        .iter()
        .filter(|(_, r)| *r >= 1.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .or_else(|| options.iter().max_by(|a, b| a.1.total_cmp(&b.1)));
    Some(best.map(|(src, _)| src.clone()))
}

/// `<resolution>` в `dppx` (css-values-4 §6.3), в том числе внутри `calc()`.
/// Единицы разрешения переписываются точками (`1x` → `1px`, `96dpi` → `1px`),
/// арифметику считает готовый разборщик длин. `None` — в записи нет ни одного
/// разрешения либо она не сводится к числу.
fn image_resolution(token: &str) -> Option<f32> {
    if !token.is_ascii() {
        return None;
    }
    let b = token.as_bytes();
    let mut expr = String::with_capacity(token.len() + 8);
    let mut found = false;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        // Имя (`calc`, `url`, слово пути) переписывается целиком: цифра внутри
        // имени числом не начинается.
        if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_') {
                i += 1;
            }
            expr.push_str(&token[s..i]);
            continue;
        }
        let number = c.is_ascii_digit()
            || (c == b'.' && b.get(i + 1).is_some_and(|n| n.is_ascii_digit()));
        if !number {
            expr.push(c as char);
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        let n: f32 = token[s..i].parse().ok()?;
        let u = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let k = match &token[u..i] {
            "" => {
                expr.push_str(&token[s..i]);
                continue;
            }
            "x" | "dppx" => 1.0,
            "dpi" => 1.0 / 96.0,
            "dpcm" => 2.54 / 96.0,
            _ => return None,
        };
        found = true;
        expr.push_str(&format!("{}px", n * k));
    }
    if !found {
        return None;
    }
    match Len::parse(&expr)? {
        Len::Px(v) => Some(v),
        _ => None,
    }
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v168, `scout-bgimg-2026-09.md` §4
/// IMG-IMAGE-SET, 2 хунка): выбор кандидата `image-set()` по спеке — отсев по
/// MIME, дублям разрешения и «кандидатов не осталось ⇒ негодная картинка».
/// Обещание +13. Полный свод против v37: **+9/−17**. Плюсы — семья, где
/// кандидат ДОЛЖЕН быть отвергнут (`image-set-type-unsupported-*`,
/// `-zero-resolution-*`, `-negative-resolution-*`, градиенты). Минусы — 17
/// пар с единственным годным кандидатом (`image-set-rendering`, `-dpi-*`,
/// `-dppx-*`, `-calc-x-*`, `-no-res-*`, `-type-*`), все ровно 2.08: картинка
/// встала не на место. Отбор верен, теряется адрес выбранного кандидата —
/// возвращать вместе с разбором `<string>` как адреса (стенд `wptrun.rs:704`
/// перебазирует только `url(...)`).
/// Записи `<gradient>`, которых GPU-путь не выражает: коническая (обход по
/// углу) и все повторяющиеся (узор стопов мостится вдоль линии —
/// css-images-3 §3.6). Такие рисуются растровой плиткой — тем же путём,
/// которым уже ходит `conic-gradient()`.
pub(crate) fn gradient_as_raster(v: &str) -> bool {
    // `cross-fade()` (css-images-4 §2.6) — смесь картинок растром той же
    // плиткой (`background::rasterize_cross_fade`).
    v.starts_with("cross-fade(")
        || v.starts_with("conic-gradient(")
        || v.starts_with("repeating-linear-gradient(")
        || v.starts_with("repeating-radial-gradient(")
        || v.starts_with("repeating-conic-gradient(")
}

/// Пересчитать фильтром цвета стопов в ЗАПИСИ растрового градиента: такие
/// градиенты живут строкой в `bg_image`, а не полем `gradient`. Слова, не
/// являющиеся цветом (направление, позиции, `from`/`at`), остаются как есть.
pub(crate) fn filter_gradient_text(raw: &str, f: &Filter) -> String {
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return raw.to_string();
    };
    if close <= open {
        return raw.to_string();
    }
    let parts: Vec<String> = crate::style::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            split_outside_parens(part)
                .into_iter()
                .map(|w| match Color::parse(&w) {
                    Some(c) => {
                        let c = f.apply(c);
                        format!(
                            "rgba({},{},{},{})",
                            (c.r * 255.0).round(),
                            (c.g * 255.0).round(),
                            (c.b * 255.0).round(),
                            c.a
                        )
                    }
                    None => w,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    format!("{}{})", &raw[..=open], parts.join(", "))
}

/// `linear-gradient(90deg, #000, #fff)`. Направления словами приводим к углу.
/// `linear-gradient(...)` и `radial-gradient(...)`.
///
/// Позиции стопов сохраняются: без них полосы не расставить, а именно они
/// задают, где цвет меняется.
/// Угол направления градиента по единице (css-values-4 §7.1): `Some(Some(deg))`
/// — законный угол, `Some(None)` — число с НЕЗНАКОМОЙ единицей (`90degree`,
/// `0.25turns`): вся запись негодна; `None` — не размерность вовсе (цвет,
/// `to right`), решают прочие ветки.
fn gradient_angle(a: &str) -> Option<Option<f32>> {
    let a = a.trim();
    let cut = a.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = a.split_at(cut);
    let n: f32 = num.parse().ok()?;
    Some(match unit.to_ascii_lowercase().as_str() {
        "deg" => Some(n),
        "grad" => Some(n * 0.9),
        "rad" => Some(n.to_degrees()),
        "turn" => Some(n * 360.0),
        _ => None,
    })
}

pub(crate) fn parse_gradient(v: &str) -> Option<Gradient> {
    // Повторяющаяся запись отличается от обычной ТОЛЬКО тем, что узор стопов
    // мостится вдоль линии (css-images-3 §3.6): разбор у них общий, а
    // повторение делает растеризатор, заворачивая долю точки.
    let v = v.strip_prefix("repeating-").unwrap_or(v);
    // Повторяющаяся запись отличается от обычной ТОЛЬКО тем, что узор стопов
    // мостится вдоль линии (css-images-3 §3.6): разбор у них общий, а
    // повторение делает растеризатор, заворачивая долю точки.
    let v = v.strip_prefix("repeating-").unwrap_or(v);
    let radial = v.starts_with("radial-gradient(");
    let inner = v
        .strip_prefix(if radial {
            "radial-gradient("
        } else {
            "linear-gradient("
        })?
        .strip_suffix(')')?;
    let parts: Vec<&str> = crate::style::css::split_args(inner);
    if parts.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    let circle = radial && parts[0].contains("circle");
    // Способ ИНТЕРПОЛЯЦИИ (css-images-4 §3.4.1.1): `to right in hsl longer
    // hue` — суффикс отделяется от направления, иначе матч направления
    // промахивался и первый аргумент уходил в стопы.
    let (head, interp) = match parts[0].find(" in ") {
        Some(at) => (parts[0][..at].trim(), Some(parts[0][at + 4..].trim())),
        None if parts[0].trim_start().starts_with("in ") => {
            ("", Some(parts[0].trim_start()[3..].trim()))
        }
        None => (parts[0].trim(), None),
    };
    // Дуга тона нужна ЛЮБОМУ полярному пространству, не только `hsl`
    // (css-color-4 §12.4); умолчание — shorter.
    let hue_arc: u8 = interp.map_or(0, |i| {
        if i.contains("longer") {
            1
        } else if i.contains("increasing") {
            2
        } else if i.contains("decreasing") {
            3
        } else {
            0
        }
    });
    // Пространство смешения: явное из записи, иначе решается ниже по составу
    // стопов (css-color-4 §12.2).
    let named_space: Option<GradSpace> =
        interp
            .and_then(|i| i.split_whitespace().next())
            .and_then(|s| match s {
                "srgb" => Some(GradSpace::Srgb),
                "srgb-linear" | "xyz" | "xyz-d50" | "xyz-d65" | "display-p3-linear"
                | "rec2020-linear" | "a98-rgb-linear" | "prophoto-rgb-linear" => {
                    Some(GradSpace::Linear)
                }
                "oklab" => Some(GradSpace::Oklab),
                "oklch" => Some(GradSpace::Oklch),
                "lab" => Some(GradSpace::Lab),
                "lch" => Some(GradSpace::Lch),
                "hsl" => Some(GradSpace::Hsl),
                "hwb" => Some(GradSpace::Hwb),
                // Пространства с собственным охватом (`display-p3`, `a98-rgb`,
                // `rec2020`, `prophoto-rgb`) гамма-кодированы, и для цветов
                // ВНУТРИ охвата sRGB смешение в них от sRGB не отличается:
                // кривая одна и та же, а матрица первичных с интерполяцией
                // коммутирует. Заводить их отдельно нечем.
                _ => None,
            });
    if interp.is_some() && head.is_empty() {
        idx = 1;
    }
    let angle = match head {
        a if a.ends_with("deg") => {
            idx = 1;
            a.trim_end_matches("deg").trim().parse().unwrap_or(180.0)
        }
        // Размерность с другой единицей: `grad`/`rad`/`turn` — законный угол,
        // прочее (`90degree`, `100gradian`, `1.57radian`, `0.25turns`) делает
        // запись негодной целиком (`angle-units-001`). Прежде такой довод
        // падал в `_ => 180.0`, не читался цветом и молча пропускался —
        // градиент из оставшихся стопов КРАСИЛ.
        a if !radial && gradient_angle(a).is_some() => {
            idx = 1;
            gradient_angle(a).flatten()?
        }
        "to right" => {
            idx = 1;
            90.0
        }
        "to left" => {
            idx = 1;
            270.0
        }
        "to bottom" => {
            idx = 1;
            180.0
        }
        "to top" => {
            idx = 1;
            0.0
        }
        "to bottom right" | "to right bottom" => {
            idx = 1;
            135.0
        }
        // Остальные два угла (css-images-3 §3.1): без них направление
        // уходило в разбор стопов и выбрасывалось, а градиент шёл сверху вниз.
        "to bottom left" | "to left bottom" => {
            idx = 1;
            225.0
        }
        "to top left" | "to left top" => {
            idx = 1;
            315.0
        }
        "to top right" | "to right top" => {
            idx = 1;
            45.0
        }
        // У радиального первым идёт описание формы (`circle at center`) —
        // цветом оно не разбирается, поэтому просто пропускается.
        first
            if radial && Color::parse(first.split_whitespace().next().unwrap_or("")).is_none() =>
        {
            idx = 1;
            180.0
        }
        _ => 180.0,
    };

    // Стоп несёт до ДВУХ позиций (css-images-4 §3.4.1): `yellow 0% 25%` — это
    // два стопа одного цвета, так записывают жёсткие полосы. Цвет с запятыми
    // внутри (`rgba(…)`) остаётся одним словом только при резке вне скобок.
    let mut raw: Vec<(Color, Option<f32>)> = vec![];
    let mut raw_px: Vec<(Color, Option<f32>)> = vec![];
    let mut any_pct = false;
    // Умолчание пространства держится на ЗАПИСИ цветов, а не на их значениях
    // (css-color-4 §12.2), поэтому решается прямо здесь, пока текст стопа
    // ещё под рукой.
    let mut all_legacy = true;
    for p in &parts[idx..] {
        let words = split_outside_parens(p);
        let Some(colour) = words.first().and_then(|w| crate::style::values::color_space::interpolation_color(w)) else {
            continue;
        };
        all_legacy &= legacy_srgb_color(words[0].as_str());
        if words.len() == 1 {
            raw.push((colour, None));
            raw_px.push((colour, None));
            continue;
        }
        for t in &words[1..] {
            if let Some(n) = t
                .strip_suffix('%')
                .and_then(|n| n.trim().parse::<f32>().ok())
            {
                any_pct = true;
                raw.push((colour, Some(n / 100.0)));
                raw_px.push((colour, None));
            } else if let Some(Len::Px(v)) = Len::parse(t) {
                // Позиция в точках: долей не выразить, длина оси известна
                // только при отрисовке. Хранится своим списком.
                raw.push((colour, None));
                raw_px.push((colour, Some(v)));
            } else if let Some((pct, px)) = pct_px_pair(t) {
                // `calc(100% - 10px)` (css-images-4 §3.4.1: `<color-stop-length>`
                // = `<length-percentage>{1,2}`): доля и точки едут ПАРОЙ в
                // `stops_raw`, растр сложит их по длине оси. Прежде такой стоп
                // отбрасывался целиком, а градиент из одних `calc`-стопов
                // (`#five` в calc-background-linear-gradient-1) гас вовсе.
                any_pct = true;
                raw.push((colour, Some(pct)));
                raw_px.push((colour, Some(px)));
            } else if !t.trim().is_empty()
                && Len::parse(t).is_none()
                && t.trim().parse::<f32>().is_err()
            {
                continue;
            } else {
                raw.push((colour, None));
                raw_px.push((colour, None));
            }
        }
    }
    // css-images-4 §3.4.1 «Color Stop Lists»: список из ОДНОГО и более
    // стопов законен, и градиент из одного стопа красит этим цветом всю
    // картинку. Разворачиваем в пару одинаковых стопов на краях линии:
    // ниже `last = raw.len() - 1` при одном стопе давал 0/0 = NaN, а
    // `return None` гасил фон вовсе (`gradient-single-stop-001/-002/-004`).
    if raw.is_empty() {
        return None;
    }
    if raw.len() == 1 {
        let (colour, _) = raw[0];
        raw = vec![(colour, Some(0.0)), (colour, Some(1.0))];
        // Точечный список остаётся ПУСТЫМ: у сплошного цвета полос в точках
        // нет, а `stops_px` собирается только когда позиция есть у всех.
        raw_px = vec![(colour, None), (colour, None)];
    }
    // Фиксация по css-images-3 §3.5.3: позиция не меньше предыдущей, стопы
    // без позиции — поровну между соседями С позициями (а не по номеру в
    // списке). Тот же расклад, что у растра.
    let last = raw.len() - 1;
    let stops: Vec<(Color, f32)> = crate::paint::background::place_stops(raw.clone());
    // TODO(gradient): `in hsl longer hue` — дуга тона синтетическими стопами
    // (css-images-4 §3.4.1.1) была за отладочным флагом HSL_ARC, замерена в
    // минус (−13/+2) и удалена; вернуться с точной математикой полос.
    // Точечные стопы пригодны к отрисовке, только когда позиции есть у ВСЕХ:
    // смешение точек с долями требует длины оси уже при разборе.
    let stops_px: Vec<(Color, f32)> = if !any_pct && raw_px.iter().all(|(_, p)| p.is_some()) {
        raw_px.iter().map(|(c, p)| (*c, p.unwrap_or(0.0))).collect()
    } else {
        vec![]
    };
    let stops_raw = raw
        .iter()
        .zip(raw_px.iter())
        .map(|((c, f), (_, p))| (*c, *f, *p))
        .collect();
    Some(Gradient {
        angle_deg: angle,
        radial,
        circle,
        from: stops[0].0,
        to: stops[last].0,
        stops,
        stops_px,
        stops_raw,
        // §12.2: без записи — sRGB, пока ВСЕ цвета устаревших форм, иначе
        // OKLab. Именно эта оговорка и держит совместимость: почти весь набор
        // пишет градиенты именами, `#hex` и `rgb()`, и остаётся в sRGB.
        space: named_space.unwrap_or(if all_legacy {
            GradSpace::Srgb
        } else {
            GradSpace::Oklab
        }),
        hue: hue_arc,
    })
}

/// Записан ли цвет УСТАРЕВШЕЙ формой sRGB: имя, `#hex`, `rgb()`, `rgba()`,
/// `hsl()`, `hsla()`, `hwb()` и их формы с прозрачностью (css-color-4 §12.2).
/// От ответа зависит пространство интерполяции по умолчанию.
fn legacy_srgb_color(token: &str) -> bool {
    let t = token.trim().to_ascii_lowercase();
    if t.starts_with('#') {
        return true;
    }
    match t.split_once('(') {
        // `rgb()` с `none` устаревшей записью не выражается: Blink вычисляет
        // его в `color(srgb …)`, и умолчание смешения становится OKLab
        // (`gradient-analogous-missing-components-004`, `gradient-eval-004`).
        // У `hsl()`/`hwb()` пространство и с `none` остаётся устаревшим.
        Some((name, body)) => match name.trim() {
            "rgb" | "rgba" => !body.contains("none"),
            "hsl" | "hsla" | "hwb" => true,
            _ => false,
        },
        // Имя цвета, `transparent` и `currentcolor` — тоже устаревшие формы.
        None => true,
    }
}
