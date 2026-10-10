//! Область композита группы с запасом для blur, формы и изоляции.

use crate::paint::effects::grouped_element::Grouped;
use gpui::{Bounds, Pixels, Window, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn group_area(
    group: &Grouped,
    bounds: Bounds<Pixels>,
    window: &Window,
) -> (Bounds<Pixels>, (f32, f32, f32, f32)) {
    let margin = px(group.blur * 4.0);
    // Форма клипа НЕ ограничена коробкой (css-masking §1.2: обрезается
    // только краска ФОРМОЙ): `circle(closest-corner at ...)` выходит за
    // края, и содержимое в её пределах обязано остаться видимым
    // (clip-path-circle-closest-corner). Область композита расширяется
    // до объединения коробки с рамкой формы.
    let shape_ext = group.mask.as_deref().and_then(|src| {
        // `border-shape`: вынос области записан в спеке маски
        // (`render::grouped` ← `Computed::border_shape_ext`): половина
        // обводки наружу, margin-box, запас под митры. Порядок в записи
        // t r b l, здесь — l t r b.
        if let Some(spec) = src.strip_prefix("bordershape:") {
            let head = spec.split_once(':')?.0;
            let v: Vec<f32> = head
                .split_whitespace()
                .filter_map(|t| t.parse::<f32>().ok())
                .collect();
            if v.len() != 9 {
                return None;
            }
            return Some((v[8], v[5], v[6], v[7]));
        }
        let raw = src.strip_prefix("shape:")?;
        if raw.starts_with("rrect(") {
            return None;
        }
        let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let (cx, cy, rx, ry) = crate::paint::background::shape_params(raw, bw, bh, 1.0)?;
        let l = (rx - cx).max(0.0);
        let t = (ry - cy).max(0.0);
        let r = (cx + rx - bw).max(0.0);
        let b = (cy + ry - bh).max(0.0);
        (l + t + r + b > 0.0).then_some((l, t, r, b))
    });
    let (sl, st, sr, sb) = shape_ext.unwrap_or((0.0, 0.0, 0.0, 0.0));
    // Опорная коробка шире `bounds` (`clip-path: margin-box`, срезы с
    // отрицательными краями): буфер группы кроет и поля, иначе краска
    // там (outline) терялась бы вместе с буфером.
    let (sl, st, sr, sb) = match group.clip_inset {
        Some([t, r, b, l]) => {
            let neg = |v: crate::style::values::value::Len| match v {
                crate::style::values::value::Len::Px(p) if p < 0.0 => -p,
                _ => 0.0,
            };
            (
                sl.max(neg(l)),
                st.max(neg(t)),
                sr.max(neg(r)),
                sb.max(neg(b)),
            )
        }
        None => (sl, st, sr, sb),
    };
    let area = Bounds {
        origin: gpui::point(
            bounds.origin.x - margin - px(sl),
            bounds.origin.y - margin - px(st),
        ),
        size: gpui::size(
            bounds.size.width + margin * 2.0 + px(sl + sr),
            bounds.size.height + margin * 2.0 + px(st + sb),
        ),
    };
    // Изоляция не обрезает (css-compositing-1 §isolation: группа меняет
    // только порядок сложения): смешиваемый ребёнок на 50 точек за краем
    // контейнера (`mix-blend-mode-overflowing-child`) и кольцо 10 точек
    // (`-blended-element-with-transparent-pixels`) обязаны попасть в кадр.
    // Буфер группы — во всё окно, поэтому и область композита — окно;
    // обрезку предков несёт маска содержимого самого композита.
    let area = if group.spill {
        Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: window.viewport_size(),
        }
    } else {
        area
    };
    (area, (sl, st, sr, sb))
}
