//! Параметры растеризации CSS basic shapes.

use crate::paint::background::*;

/// Параметры формы `circle(...)` / `ellipse(...)`: центр и радиусы в
/// точках растра; `scale` переводит точечные величины записи (CSS) в них.
pub fn shape_params(raw: &str, fw: f32, fh: f32, scale: f32) -> Option<(f32, f32, f32, f32)> {
    let (kind, rest) = raw.split_once('(')?;
    // Хвост после СВОЕЙ закрывающей скобки — опорная коробка
    // (`circle(50% at left 40px top 40px) border-box`, css-masking-1
    // §clip-path: `<basic-shape> || <geometry-box>`): форме он не нужен, а
    // оставленный в строке он превращал четырёхзначный центр в шесть
    // токенов, и центр падал в середину коробки (`shape-outside-circle-048`).
    let mut depth = 1i32;
    let rest = match rest.char_indices().find(|&(_, c)| {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        depth == 0
    }) {
        Some((end, _)) => &rest[..=end],
        None => rest,
    };
    let circle = kind.trim().eq_ignore_ascii_case("circle");
    // Снимается ОДНА закрывающая скобка — своей функции: `trim_end_matches`
    // съедал и скобку последнего `calc(...)` центра
    // (`circle(25% at calc(50% - 10px) calc(50% - 10px))`).
    let rest = rest.strip_suffix(')').unwrap_or(rest);
    // Ключевое слово `at` может стоять ПЕРВЫМ, без радиусов перед ним:
    // `ellipse(at 110px 50%)`. Деление по строке с двумя пробелами такую
    // запись не находило вовсе, и `at` уходило в радиус по X
    // (`shape-outside-ellipse-023`).
    let (rads, pos) = match rest.trim().strip_prefix("at ") {
        Some(p) => ("", Some(p.trim())),
        None => match rest.split_once(" at ") {
            Some((r, p)) => (r.trim(), Some(p.trim())),
            None => (rest.trim(), None),
        },
    };
    // Центр: `at X Y`; доля — от стороны коробки; одиночное слово — сторона.
    let axis = |token: &str, side: f32| -> Option<f32> {
        let t = token.trim();
        match t {
            "center" => Some(side * 0.5),
            "left" | "top" => Some(0.0),
            "right" | "bottom" => Some(side),
            // Смесь долей и точек в центре (`at calc(50% - 10px) …`).
            _ if t.starts_with("calc(") => {
                let (p, add) = crate::style::values::value::calc_pct_px(t)?;
                Some(p * side + add * scale)
            }
            _ => match crate::style::values::value::Len::parse(t)? {
                crate::style::values::value::Len::Px(v) => Some(v * scale),
                crate::style::values::value::Len::Pct(p) => Some(p * side),
                _ => None,
            },
        }
    };
    let (cx, cy) = match pos {
        Some(p) => {
            // Токены верхнего уровня: `calc(50% - 10px)` — один токен.
            let toks: Vec<&str> = split_top(p);
            // Позиционные слова в паре идут в любом порядке: горизонтальное
            // слово — всегда ось X (`at center right`, `at top left`).
            let horiz = |t: &str| matches!(t, "left" | "right");
            let vert = |t: &str| matches!(t, "top" | "bottom");
            // Четырёхзначная запись — пары «край смещение»: `at left 40px
            // top 40px`; от правого/нижнего края смещение зеркалится.
            if toks.len() == 4 {
                let pair = |edge: &str, off: &str, side: f32| -> Option<f32> {
                    let v = axis(off, side)?;
                    Some(match edge {
                        "right" | "bottom" => side - v,
                        _ => v,
                    })
                };
                let horiz_first = horiz(toks[0]);
                let (xe, xo, ye, yo) = if horiz_first {
                    (toks[0], toks[1], toks[2], toks[3])
                } else {
                    (toks[2], toks[3], toks[0], toks[1])
                };
                (
                    pair(xe, xo, fw).unwrap_or(fw * 0.5),
                    pair(ye, yo, fh).unwrap_or(fh * 0.5),
                )
            } else {
                let (tx, ty) = match toks.as_slice() {
                    [a, b] if vert(a) || horiz(b) => (*b, *a),
                    [a, b] => (*a, *b),
                    // Одно значение: второе — `center` (css-values-4
                    // §position), но слово `top`/`bottom` — вертикальная ось:
                    // `at top` = `center top` (offset-path-shape-circle-003,
                    // -ellipse-003). Прежде `top` уходило в X и центр вставал
                    // на левую сторону.
                    [a] if vert(a) => ("center", *a),
                    [a] => (*a, "center"),
                    _ => ("center", "center"),
                };
                (
                    axis(tx, fw).unwrap_or(fw * 0.5),
                    axis(ty, fh).unwrap_or(fh * 0.5),
                )
            }
        }
        None => (fw * 0.5, fh * 0.5),
    };
    // Радиус: точки, доля или ключевая сторона (css-shapes-1 §3.1.1.3):
    // closest/farthest — расстояние от центра до ближайшей/дальней стороны
    // ПО ОСИ (у эллипса — своей), у круга corner — до угла.
    let side_r = |keyword: &str, c: f32, side: f32| -> f32 {
        match keyword {
            "closest-side" => c.abs().min((side - c).abs()),
            "farthest-side" => c.max((side - c).abs()),
            _ => 0.0,
        }
    };
    let corner_r = |far: bool| -> f32 {
        let dx = if far {
            cx.max(fw - cx)
        } else {
            cx.min(fw - cx)
        };
        let dy = if far {
            cy.max(fh - cy)
        } else {
            cy.min(fh - cy)
        };
        (dx * dx + dy * dy).sqrt()
    };
    let radius = |token: &str, c: f32, side: f32, pct_base: f32| -> Option<f32> {
        match token {
            "closest-side" => Some(side_r("closest-side", c, side)),
            "farthest-side" => Some(side_r("farthest-side", c, side)),
            "closest-corner" => Some(corner_r(false)),
            "farthest-corner" => Some(corner_r(true)),
            _ => match crate::style::values::value::Len::parse(token)? {
                crate::style::values::value::Len::Px(v) => Some(v * scale),
                crate::style::values::value::Len::Pct(p) => Some(p * pct_base),
                _ => None,
            },
        }
    };
    let diag = ((fw * fw + fh * fh) / 2.0).sqrt();
    let (rx, ry) = if circle {
        let token = rads.split_whitespace().next().unwrap_or("closest-side");
        let r = radius(token, cx, fw, diag)
            .unwrap_or_else(|| side_r("closest-side", cx, fw).min(side_r("closest-side", cy, fh)));
        // Ключевые стороны у круга — по ОБЕИМ осям сразу.
        let r = match token {
            "closest-side" => cx
                .abs()
                .min((fw - cx).abs())
                .min(cy.abs().min((fh - cy).abs())),
            "farthest-side" => cx.max((fw - cx).abs()).max(cy.max((fh - cy).abs())),
            _ => r,
        };
        (r, r)
    } else {
        let mut it = rads.split_whitespace();
        let tx = it.next().unwrap_or("closest-side");
        let ty = it.next().unwrap_or("closest-side");
        // Угловые ключи у эллипса — ЕВКЛИДОВО расстояние до угла, как у
        // круга (clip-path-ellipse-2-ref задаёт rx=√(175²+175²)).
        (
            radius(tx, cx, fw, fw).unwrap_or_else(|| side_r("closest-side", cx, fw)),
            radius(ty, cy, fh, fh).unwrap_or_else(|| side_r("closest-side", cy, fh)),
        )
    };
    Some((cx, cy, rx, ry))
}
