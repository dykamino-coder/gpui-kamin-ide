//! Резкие тени `box-shadow` и контур `outline` коробки с `corner-shape`
//! (css-borders-4 §corner-shaping: рамка, тень, контур и обрезка следуют
//! ОДНОЙ форме). Форма — контур, раздвинутый на разлёт или сдвиг
//! (`contour::offset`, Blink `ContouredRect` с исходным прямоугольником):
//! стороны параллельны исходным, концы углов продлены митрами до сторон.
//! Тени лежат подложкой ПОД буфером группы (`Grouped::under`), контур —
//! накладкой НАД ним (`Grouped::over`); прежние скруглённые кольца
//! (`shadows`, `outline`) у такой коробки не ставятся.

use crate::document::state::ValueCx;
use crate::paint::background::{contour_offset, rrect_spec};
use crate::paint::effects::grouped_element::FillLayer;
use crate::style::computed::{Computed, OUTLINE_DOUBLE};
use crate::svg::path_commands::{Command, Geometry, Sink};
use crate::svg::path_scene::*;

/// Сцена контуров по размеру коробки (bw, bh), выносу (l, t) и холсту (aw, ah).
pub(crate) type Layer =
    Box<dyn Fn(f32, f32, f32, f32, f32, f32) -> Option<crate::svg::path_scene::Scene>>;

/// Аргументы `rrect(...)` коробки с фигурными углами; None — углы круглые
/// или форму задаёт `border-shape`.
fn args(c: &Computed, vcx: ValueCx<'_>) -> Option<String> {
    if !c.corner_shaped() || c.border_shape.is_some() {
        return None;
    }
    let s = rrect_spec(c, None, vcx);
    let inner = s.strip_prefix("rrect(")?.strip_suffix(')')?;
    Some(inner.to_string())
}

/// Тени без размытия идут по контуру (`outer_shadows` их пропускает).
pub(crate) fn shapes_shadows(c: &Computed, vcx: ValueCx<'_>) -> bool {
    args(c, vcx).is_some()
}

/// Контур `outline` идёт по форме (`outline::decorations` его пропускает):
/// сплошной и двойной; `auto` остаётся кольцом фокуса.
pub(crate) fn shapes_outline(c: &Computed, vcx: ValueCx<'_>) -> bool {
    args(c, vcx).is_some()
        && c.outline
            .as_ref()
            .is_some_and(|o| matches!(o.style, Some(1) | Some(OUTLINE_DOUBLE)))
        && c.contour_outline_params().is_some()
}

/// Вынос слоёв за border-box (l t r b): разлёт со смещением теней и
/// сдвиг с толщиной контура, с запасом в точку под сглаживание.
pub(crate) fn reach(c: &Computed, vcx: ValueCx<'_>) -> [f32; 4] {
    let mut e = [0.0f32; 4];
    if shapes_shadows(c, vcx) {
        for (sh, _) in c.resolved_shadows(false) {
            let s = sh.spread.max(0.0) + 1.0;
            e[0] = e[0].max(s - sh.x);
            e[1] = e[1].max(s - sh.y);
            e[2] = e[2].max(s + sh.x);
            e[3] = e[3].max(s + sh.y);
        }
    }
    if shapes_outline(c, vcx)
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
pub(crate) fn shadow_layer(c: &Computed, vcx: ValueCx<'_>) -> Option<Layer> {
    let args = args(c, vcx)?;
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
        let mut children = Vec::new();
        for (sh, col) in shadows.iter().rev() {
            let contour = contour_offset(&args, bw, bh, [-sh.spread; 4])?;
            let mut d = Geometry::default();
            contour.commands(&mut d, sl + sh.x, st + sh.y);
            children.push(shape(
                d.finish()?,
                FillRule::NonZero,
                Some(paint(*col)),
                None,
            ));
        }
        let border = contour_offset(&args, bw, bh, [0.0; 4])?;
        let mut d = Geometry::default();
        for command in [
            Command::Move([0.0, 0.0]),
            Command::Horizontal(aw),
            Command::Vertical(ah),
            Command::Horizontal(0.0),
            Command::Close,
        ] {
            d.command(false, command);
        }
        border.commands(&mut d, sl, st);
        let mut root = group(children);
        root.clip = Some(PathShape {
            data: d.finish()?,
            fill: Some(gray(false)),
            rule: FillRule::EvenOdd,
            stroke: None,
        });
        Some(Scene::new(aw, ah, root))
    }))
}

fn opaque_background(c: &Computed) -> bool {
    c.background.is_some_and(|b| b.a >= 1.0)
}

/// Резкие тени при непрозрачном фоне — заливками по пути под буфером
/// (`Grouped::under_fills`): вырез под коробкой не нужен, а край ложится
/// так же, как у `div` с фоном и `clip-path: path()`.
pub(crate) fn shadow_fills(c: &Computed, vcx: ValueCx<'_>) -> Option<FillLayer> {
    let args = args(c, vcx)?;
    let shadows: Vec<_> = c
        .resolved_shadows(false)
        .into_iter()
        .filter(|(s, col)| s.blur <= 0.0 && col.a > 0.0)
        .collect();
    if shadows.is_empty() || !opaque_background(c) {
        return None;
    }
    Some(Box::new(move |bw, bh, sl, st, aw, ah| {
        shadows
            .iter()
            .rev()
            .filter_map(|(sh, col)| {
                let p = contour_offset(&args, bw, bh, [-sh.spread; 4])?;
                let mut d = Geometry::default();
                p.commands(&mut d, sl + sh.x, st + sh.y);
                Some((coverage(aw, ah, Some(d.finish()?), FillRule::NonZero), *col))
            })
            .collect()
    }))
}

/// Накладка контура: полоса между контурами, раздвинутыми на сдвиг и на
/// сдвиг с толщиной (css-ui-4 §outline; у `double` — две полосы по трети).
pub(crate) fn outline_layer(c: &Computed, vcx: ValueCx<'_>) -> Option<FillLayer> {
    if !shapes_outline(c, vcx) {
        return None;
    }
    let args = args(c, vcx)?;
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
    Some(Box::new(move |bw, bh, sl, st, aw, ah| {
        let mut d = Geometry::default();
        for (outer, inner) in &bands {
            let (Some(o), Some(i)) = (
                contour_offset(&args, bw, bh, [-outer; 4]),
                contour_offset(&args, bw, bh, [-inner; 4]),
            ) else {
                return Vec::new();
            };
            o.commands(&mut d, sl, st);
            i.commands(&mut d, sl, st);
        }
        d.finish()
            .map(|data| vec![(coverage(aw, ah, Some(data), FillRule::EvenOdd), colour)])
            .unwrap_or_default()
    }))
}
