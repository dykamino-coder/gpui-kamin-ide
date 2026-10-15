//! Резкие тени `box-shadow` и контур `outline` коробки с `corner-shape`
//! (css-borders-4 §corner-shaping: рамка, тень, контур и обрезка следуют
//! ОДНОЙ форме). Форма — контур, раздвинутый на разлёт или сдвиг
//! (`contour::offset`, Blink `ContouredRect` с исходным прямоугольником):
//! стороны параллельны исходным, концы углов продлены митрами до сторон.
//! Тени лежат подложкой ПОД буфером группы (`Grouped::under`), контур —
//! накладкой НАД ним (`Grouped::over`); прежние скруглённые кольца
//! (`shadows`, `outline`) у такой коробки не ставятся.

use crate::paint::background::contour::{Path, Seg};
use crate::paint::background::{contour_offset, rrect_spec};
use crate::paint::effects::grouped_element::FillLayer;
use crate::style::computed::{Computed, OUTLINE_DOUBLE};
use crate::style::values::value::Color;

/// Слой SVG по размеру коробки (bw, bh), выносу (l, t) и холсту (aw, ah).
pub(crate) type Layer = Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<String>>;

/// Аргументы `rrect(...)` коробки с фигурными углами; None — углы круглые
/// или форму задаёт `border-shape`.
fn args(c: &Computed) -> Option<String> {
    if !c.corner_shaped() || c.border_shape.is_some() {
        return None;
    }
    let s = rrect_spec(c, None);
    let inner = s.strip_prefix("rrect(")?.strip_suffix(')')?;
    Some(inner.to_string())
}

/// Тени без размытия идут по контуру (`outer_shadows` их пропускает).
pub(crate) fn shapes_shadows(c: &Computed) -> bool {
    args(c).is_some()
}

/// Контур `outline` идёт по форме (`outline::decorations` его пропускает):
/// сплошной и двойной; `auto` остаётся кольцом фокуса.
pub(crate) fn shapes_outline(c: &Computed) -> bool {
    args(c).is_some()
        && c.outline
            .as_ref()
            .is_some_and(|o| matches!(o.style, Some(1) | Some(OUTLINE_DOUBLE)))
        && c.contour_outline_params().is_some()
}

/// Запись `d` пути со сдвигом на (dx, dy).
pub(crate) fn moved(p: &Path, dx: f32, dy: f32) -> String {
    let m = |(x, y): (f32, f32)| format!("{} {}", x + dx, y + dy);
    let mut d = format!("M{}", m(p.start));
    for s in &p.segs {
        match *s {
            Seg::Line(a) => d.push_str(&format!(" L{}", m(a))),
            Seg::Cubic(a, b, c) => d.push_str(&format!(" C{} {} {}", m(a), m(b), m(c))),
        }
    }
    d.push_str(" Z");
    d
}

fn rgb(c: Color) -> String {
    format!(
        "rgb({},{},{})",
        (c.r * 255.0).round(),
        (c.g * 255.0).round(),
        (c.b * 255.0).round()
    )
}

fn svg(aw: f32, ah: f32, body: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{aw}" height="{ah}">{body}</svg>"##)
}

/// Вынос слоёв за border-box (l t r b): разлёт со смещением теней и
/// сдвиг с толщиной контура, с запасом в точку под сглаживание.
pub(crate) fn reach(c: &Computed) -> [f32; 4] {
    let mut e = [0.0f32; 4];
    if shapes_shadows(c) {
        for (sh, _) in c.resolved_shadows(false) {
            let s = sh.spread.max(0.0) + 1.0;
            e[0] = e[0].max(s - sh.x);
            e[1] = e[1].max(s - sh.y);
            e[2] = e[2].max(s + sh.x);
            e[3] = e[3].max(s + sh.y);
        }
    }
    if shapes_outline(c)
        && let Some((w, off, _)) = c.contour_outline_params()
    {
        let r = (off + w).max(0.0) + 1.0;
        e = e.map(|v| v.max(r));
    }
    e
}

/// Подложка резких наружных теней: контур, раздвинутый на разлёт и
/// сдвинутый на смещение (css-backgrounds-3 §7.1), вне border-box. При
/// непрозрачном фоне вырез не нужен — фон его кроет (Blink
/// `PaintNormalBoxShadow`: у непрозрачной коробки вырез ужат на точку), и
/// край фигуры ложится на тень, как у эталонов «тень, затем коробка».
pub(crate) fn shadow_layer(c: &Computed) -> Option<Layer> {
    let args = args(c)?;
    let shadows: Vec<_> = c
        .resolved_shadows(false)
        .into_iter()
        .filter(|(s, col)| s.blur <= 0.0 && col.a > 0.0)
        .collect();
    if shadows.is_empty() {
        return None;
    }
    if opaque_background(c) {
        return None;
    }
    Some(Box::new(move |bw, bh, sl, st, aw, ah| {
        let mut body = String::new();
        // css-backgrounds-3 §7.1: первая тень сверху — рисуем снизу вверх.
        for (sh, col) in shadows.iter().rev() {
            let p = contour_offset(&args, bw, bh, [-sh.spread; 4])?;
            body.push_str(&format!(
                r#"<path fill="{}" fill-opacity="{}" d="{}"/>"#,
                rgb(*col),
                col.a,
                moved(&p, sl + sh.x, st + sh.y)
            ));
        }
        {
            let border = contour_offset(&args, bw, bh, [0.0; 4])?;
            body = format!(
                r#"<defs><clipPath id="o"><path clip-rule="evenodd" d="M0 0 H{aw} V{ah} H0 Z {}"/></clipPath></defs><g clip-path="url(#o)">{body}</g>"#,
                moved(&border, sl, st)
            );
        }
        Some(svg(aw, ah, &body))
    }))
}

fn opaque_background(c: &Computed) -> bool {
    c.background.is_some_and(|b| b.a >= 1.0)
}

/// Резкие тени при непрозрачном фоне — заливками по пути под буфером
/// (`Grouped::under_fills`): вырез под коробкой не нужен, а край ложится
/// так же, как у `div` с фоном и `clip-path: path()`.
pub(crate) fn shadow_fills(c: &Computed) -> Option<FillLayer> {
    let args = args(c)?;
    let shadows: Vec<_> = c
        .resolved_shadows(false)
        .into_iter()
        .filter(|(s, col)| s.blur <= 0.0 && col.a > 0.0)
        .collect();
    if shadows.is_empty() || !opaque_background(c) {
        return None;
    }
    Some(Box::new(move |bw, bh, sl, st, _aw, _ah| {
        shadows
            .iter()
            .rev()
            .filter_map(|(sh, col)| {
                let p = contour_offset(&args, bw, bh, [-sh.spread; 4])?;
                Some((
                    format!("pathdef:nonzero:{}", moved(&p, sl + sh.x, st + sh.y)),
                    *col,
                ))
            })
            .collect()
    }))
}

/// Накладка контура: полоса между контурами, раздвинутыми на сдвиг и на
/// сдвиг с толщиной (css-ui-4 §outline; у `double` — две полосы по трети).
pub(crate) fn outline_layer(c: &Computed) -> Option<FillLayer> {
    if !shapes_outline(c) {
        return None;
    }
    let args = args(c)?;
    let (w, off, colour) = c.contour_outline_params()?;
    if colour.a <= 0.0 {
        return None;
    }
    let third = (w / 3.0).round();
    let double = c.outline.as_ref().and_then(|o| o.style) == Some(OUTLINE_DOUBLE) && third >= 1.0;
    let bands: Vec<(f32, f32)> = if double {
        vec![(off + w, off + w - third), (off + third, off)]
    } else {
        vec![(off + w, off)]
    };
    Some(Box::new(move |bw, bh, sl, st, _aw, _ah| {
        let mut d = String::new();
        for (outer, inner) in &bands {
            let (Some(o), Some(i)) = (
                contour_offset(&args, bw, bh, [-outer; 4]),
                contour_offset(&args, bw, bh, [-inner; 4]),
            ) else {
                return Vec::new();
            };
            d.push_str(&format!("{} {} ", moved(&o, sl, st), moved(&i, sl, st)));
        }
        vec![(format!("pathdef:evenodd:{}", d.trim_end()), colour)]
    }))
}
