//! Выбор SVG-единиц с масками и фильтрами для отдельной растеризации.

use crate::dom::{Element, Node};
use crate::svg::clip::{shape_box, translate_only};

/// Единица рисунка в порядке отрисовки: прямой ребёнок `<svg>` (`inner` =
/// None) либо ребёнок группы `<g>` с чистым сдвигом (`inner` = его номер).
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Unit {
    pub(super) top: usize,
    pub(super) inner: Option<usize>,
}

/// Прямоугольник (x, y, w, h) в CSS-точках коробки `<svg>`.
pub(super) type Rect = (f32, f32, f32, f32);

/// Маскированная единица: stroke-box (коробка слоя), fill-box, стиль.
pub(super) type MaskedUnit = (Unit, Rect, Rect, crate::style::computed::Computed);

/// Теги, которые ничего не рисуют и нужны любому срезу разметки
/// (определения, стили, заголовки).
pub(super) fn non_rendering(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "defs"
            | "style"
            | "title"
            | "desc"
            | "metadata"
            | "lineargradient"
            | "radialgradient"
            | "pattern"
            | "mask"
            | "clippath"
            | "filter"
            | "symbol"
            | "marker"
    )
}

/// Пользовательские единицы → CSS-точки коробки `<svg>` w×h: масштаб и
/// сдвиг. При `viewBox` — по нему (только равномерный масштаб: иначе
/// `preserveAspectRatio` центрирует, и прямой пересчёт неверен); без него
/// — `zoom` (css-viewport-1 §zoom, см. `serialize_sized`).
pub(super) fn user_to_css(e: &Element, w: f32, h: f32) -> Option<(f32, f32, f32)> {
    if let Some(vb) = e.attr("viewBox") {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if p.len() != 4 || p[2] <= 0.0 || p[3] <= 0.0 {
            return None;
        }
        let (sx, sy) = (w / p[2], h / p[3]);
        if (sx - sy).abs() > 1e-3 {
            return None;
        }
        return Some((sx, -p[0] * sx, -p[1] * sx));
    }
    Some((e.style.zoom_eff.unwrap_or(1.0), 0.0, 0.0))
}

/// Дети `<svg>` с CSS-маской (`mask-image` из каскада): единица и её
/// stroke-box в CSS-точках коробки рисунка. `None` — масок нет либо у
/// какой-то рамка не считается (путь прежний, без маски).
///
/// css-masking-1 §7 применяется и к SVG-элементам, но usvg маску из CSS не
/// знает (`write_element` её не мостит): слой маскированного ребёнка
/// режется отдельным растром и оборачивается `render::grouped` с ЕГО
/// `Computed` — тем же путём, что HTML-коробка (scout-masking A2).
pub(super) fn masked_units(e: &Element, w: f32, h: f32) -> Option<Vec<MaskedUnit>> {
    let (s, ox, oy) = user_to_css(e, w, h)?;
    let mut out = Vec::new();
    // Рамки ребёнка в CSS-точках: stroke-box (коробка слоя) и fill-box.
    let box_of = |c: &Element, dx: f32, dy: f32| -> Option<(Rect, Rect)> {
        if c.style.transform.is_some() || c.style.translate.is_some() {
            return None;
        }
        let (tx, ty) = match c.attr("transform") {
            Some(t) => translate_only(t)?,
            None => (0.0, 0.0),
        };
        let place = |(bx, by, bw, bh): (f32, f32, f32, f32)| -> Rect {
            (
                (bx + tx + dx) * s + ox,
                (by + ty + dy) * s + oy,
                bw * s,
                bh * s,
            )
        };
        Some((place(shape_box(c, true)?), place(shape_box(c, false)?)))
    };
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if c.style.mask_image.is_some() {
            let (sb, fb) = box_of(c, 0.0, 0.0)?;
            out.push((
                Unit {
                    top: i,
                    inner: None,
                },
                sb,
                fb,
                c.style.clone(),
            ));
            continue;
        }
        if c.tag.eq_ignore_ascii_case("g")
            && c.style.transform.is_none()
            && c.style.translate.is_none()
        {
            let (gx, gy) = match c.attr("transform") {
                Some(t) => match translate_only(t) {
                    Some(v) => v,
                    None => continue,
                },
                None => (0.0, 0.0),
            };
            for (j, m) in c.children.iter().enumerate() {
                let Node::Element(k) = m else { continue };
                if k.style.mask_image.is_some() {
                    let (sb, fb) = box_of(k, gx, gy)?;
                    out.push((
                        Unit {
                            top: i,
                            inner: Some(j),
                        },
                        sb,
                        fb,
                        k.style.clone(),
                    ));
                }
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Копия `<svg>` только с указанными единицами (и всем нерисующим):
/// группа `<g>` остаётся с подмножеством детей.
pub(super) fn filtered(e: &Element, keep: &dyn Fn(Unit) -> bool) -> Element {
    let mut copy = e.clone();
    let mut kids = Vec::new();
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if non_rendering(&c.tag) {
            kids.push(n.clone());
            continue;
        }
        if keep(Unit {
            top: i,
            inner: None,
        }) {
            kids.push(n.clone());
            continue;
        }
        if c.tag.eq_ignore_ascii_case("g") {
            let mut g = c.clone();
            g.children = c
                .children
                .iter()
                .enumerate()
                .filter(|(j, m)| match m {
                    Node::Element(k) => {
                        non_rendering(&k.tag)
                            || keep(Unit {
                                top: i,
                                inner: Some(*j),
                            })
                    }
                    Node::Text(_) => false,
                })
                .map(|(_, m)| m.clone())
                .collect();
            if g.children
                .iter()
                .any(|m| matches!(m, Node::Element(k) if !non_rendering(&k.tag)))
            {
                kids.push(Node::Element(g));
            }
        }
    }
    copy.children = kids;
    copy
}
