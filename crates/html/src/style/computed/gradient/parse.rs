//! Разбор linear-/radial-gradient(): направление, пространство интерполяции, стопы с позициями.

use super::*;

/// `linear-gradient(90deg, #000, #fff)`. Направления словами приводим к углу.
/// `linear-gradient(...)` и `radial-gradient(...)`.
///
/// Позиции стопов сохраняются: без них полосы не расставить, а именно они
/// задают, где цвет меняется.
/// Угол направления градиента по единице (css-values-4 §7.1): `Some(Some(deg))`
/// — законный угол, `Some(None)` — число с НЕЗНАКОМОЙ единицей (`90degree`,
/// `0.25turns`): вся запись негодна; `None` — не размерность вовсе (цвет,
/// `to right`), решают прочие ветки.
pub(super) fn gradient_angle(a: &str) -> Option<Option<f32>> {
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

pub(super) fn parse_gradient(v: &str, current: Color) -> Option<Gradient> {
    let authored = v;
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
                "srgb-linear"
                | "xyz"
                | "xyz-d50"
                | "xyz-d65"
                | "display-p3-linear"
                | "rec2020-linear"
                | "a98-rgb-linear"
                | "prophoto-rgb-linear" => Some(GradSpace::Linear),
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
    let (angle, direction_idx) = gradient_direction(head, radial, idx)?;
    idx = direction_idx;

    // Color operands align with both positions of a double-position stop.
    let mut raw: Vec<(Color, Option<f32>)> = vec![];
    let mut raw_px: Vec<(Color, Option<f32>)> = vec![];
    let mut any_pct = false;
    // Умолчание пространства держится на ЗАПИСИ цветов, а не на их значениях
    // (css-color-4 §12.2), поэтому решается прямо здесь, пока текст стопа
    // ещё под рукой.
    let mut all_legacy = true;
    let mut colors = Vec::new();
    for p in &parts[idx..] {
        let words = split_outside_parens(p);
        let Some(expression) = words
            .first()
            .and_then(|w| crate::style::values::color_space::ColorExpression::parse(w))
        else {
            continue;
        };
        let colour = expression.resolve_for_interpolation(current);
        all_legacy &= legacy_srgb_color(words[0].as_str())
            && !matches!(
                expression,
                crate::style::values::color_space::ColorExpression::Relative(_)
            );
        if words.len() == 1 {
            colors.push(expression);
            raw.push((colour, None));
            raw_px.push((colour, None));
            continue;
        }
        for t in &words[1..] {
            let before = raw.len();
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
                // Percentage and pixels remain paired until the axis is known.
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
            if raw.len() > before {
                colors.push(expression.clone());
            }
        }
    }
    // A single stop expands to two identical endpoint colors (Images 4 §3.4.1).
    if raw.is_empty() {
        return None;
    }
    if raw.len() == 1 {
        let (colour, _) = raw[0];
        raw = vec![(colour, Some(0.0)), (colour, Some(1.0))];
        // Точечный список остаётся ПУСТЫМ: у сплошного цвета полос в точках
        // нет, а `stops_px` собирается только когда позиция есть у всех.
        raw_px = vec![(colour, None), (colour, None)];
        colors = vec![colors[0].clone(); 2];
    }
    // Фиксация по css-images-3 §3.5.3: позиция не меньше предыдущей, стопы
    // без позиции — поровну между соседями С позициями (а не по номеру в
    // списке). Тот же расклад, что у растра.
    let last = raw.len() - 1;
    let stops: Vec<(Color, f32)> = crate::style::computed::place_stops(raw.clone());
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
    let space = named_space.unwrap_or(if all_legacy {
        GradSpace::Srgb
    } else {
        GradSpace::Oklab
    });
    let polar =
        crate::style::values::color_space::DirectGradient::new(authored, stops.len(), space);
    Some(Gradient {
        shape: GradientShape::parse(authored)?,
        polar,
        angle_deg: angle,
        radial,
        circle,
        from: stops[0].0,
        to: stops[last].0,
        stops,
        stops_px,
        stops_raw,
        space,
        hue: hue_arc,
        colors: if colors.iter().any(|c| c.depends_on_current()) {
            colors
        } else {
            vec![]
        },
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
