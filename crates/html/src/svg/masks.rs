//! CSS-маски на детях `<svg>`: срезы разметки по порядку отрисовки и слои.

use super::clip::{shape_box, translate_only};
use super::raster::rasterize;
use super::serialize::serialize_sized;
use crate::dom::{Element, Node};
use gpui::{AnyElement, ImageSource, IntoElement};

/// Единица рисунка в порядке отрисовки: прямой ребёнок `<svg>` (`inner` =
/// None) либо ребёнок группы `<g>` с чистым сдвигом (`inner` = его номер).
#[derive(Clone, Copy, PartialEq)]
struct Unit {
    top: usize,
    inner: Option<usize>,
}

/// Прямоугольник (x, y, w, h) в CSS-точках коробки `<svg>`.
type Rect = (f32, f32, f32, f32);

/// Маскированная единица: stroke-box (коробка слоя), fill-box, стиль.
type MaskedUnit = (Unit, Rect, Rect, crate::style::computed::Computed);

/// Теги, которые ничего не рисуют и нужны любому срезу разметки
/// (определения, стили, заголовки).
fn non_rendering(tag: &str) -> bool {
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
fn user_to_css(e: &Element, w: f32, h: f32) -> Option<(f32, f32, f32)> {
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
fn masked_units(e: &Element, w: f32, h: f32) -> Option<Vec<MaskedUnit>> {
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
fn filtered(e: &Element, keep: &dyn Fn(Unit) -> bool) -> Element {
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

/// Рисунок с CSS-масками на детях: срезы разметки по порядку отрисовки —
/// пробеги немаскированных единиц полноразмерными растрами, каждая
/// маскированная — своим растром по её рамке в обёртке `render::grouped`
/// (маска, `mask-size/-repeat/-position/-mode` — из её стиля). Порядок
/// наложения сохраняется: слои кладутся абсолютно друг над другом.
pub(super) fn masked_layers(e: &Element, w: f32, h: f32, rw: f32, rh: f32) -> Option<AnyElement> {
    use gpui::{ParentElement as _, Styled as _};
    let units = masked_units(e, w, h)?;
    // Пользовательская единица в CSS-точках (та же, что у `masked_units`).
    let (s, _, _) = user_to_css(e, w, h)?;
    let masked: Vec<Unit> = units.iter().map(|(u, _, _, _)| *u).collect();
    // Края коробки `kind` от коробки слоя внутрь (t/r/b/l): fill-box,
    // stroke-box (= border-/margin-box у SVG, css-masking-1 §7.10: «for
    // border-box and margin-box is stroke-box», для content-/padding-box —
    // fill-box), view-box — вьюпорт рисунка (0, 0, w, h).
    let edges = |kind: Option<u8>, layer: Rect, fill: Rect| -> [f32; 4] {
        let (lx, ly, lw, lh) = layer;
        let (bx, by, bw, bh) = match kind {
            Some(2) | Some(3) | Some(4) => fill,
            Some(6) => (0.0, 0.0, w, h),
            _ => layer,
        };
        [
            by - ly,
            (lx + lw) - (bx + bw),
            (ly + lh) - (by + bh),
            bx - lx,
        ]
    };
    // Последовательность единиц в порядке отрисовки.
    let mut seq: Vec<Unit> = Vec::new();
    for (i, n) in e.children.iter().enumerate() {
        let Node::Element(c) = n else { continue };
        if non_rendering(&c.tag) {
            continue;
        }
        let split = c.tag.eq_ignore_ascii_case("g")
            && masked.iter().any(|u| u.top == i && u.inner.is_some());
        if split {
            for (j, m) in c.children.iter().enumerate() {
                if matches!(m, Node::Element(k) if !non_rendering(&k.tag)) {
                    seq.push(Unit {
                        top: i,
                        inner: Some(j),
                    });
                }
            }
        } else {
            seq.push(Unit {
                top: i,
                inner: None,
            });
        }
    }
    let mut root = gpui::div().w(gpui::px(w)).h(gpui::px(h)).flex_shrink_0();
    if let Some(bg) = e.style.background {
        root = root.bg(bg.to_hsla());
    }
    let mut run: Vec<Unit> = Vec::new();
    let flush = |run: &mut Vec<Unit>, root: gpui::Div| -> gpui::Div {
        if run.is_empty() {
            return root;
        }
        let keep = run.clone();
        run.clear();
        let part = filtered(e, &|u| keep.contains(&u));
        let Some(img) = rasterize(&serialize_sized(&part, rw, rh, 0.0, 0.0), rw, rh) else {
            return root;
        };
        root.child(
            gpui::img(ImageSource::Render(img))
                .w(gpui::px(rw))
                .h(gpui::px(rh))
                .absolute()
                .top_0()
                .left_0(),
        )
    };
    for u in seq {
        let Some((_, layer_box, fill_box, style)) = units.iter().find(|(m, _, _, _)| *m == u)
        else {
            run.push(u);
            continue;
        };
        let (bx, by, bw, bh) = layer_box;
        root = flush(&mut run, root);
        if *bw <= 0.0 || *bh <= 0.0 {
            continue;
        }
        let mut style = style.clone();
        style.mask_box_override = Some((
            edges(style.mask_origin, *layer_box, *fill_box),
            style
                .mask_clip
                .filter(|k| *k != 255)
                .map(|k| edges(Some(k), *layer_box, *fill_box)),
        ));
        // Маска живёт в пользовательских единицах ребёнка (css-masking-1
        // §7: длины и `auto`-размер — в системе координат элемента): при
        // `viewBox` 0 0 100 100 на 200×200 рисунок-маска 50×50 кроет
        // 100×100 CSS-точек (mask-origin-3, mask-clip-2). Точечные `mask-size`
        // и `mask-position` переводятся здесь, интринзик — в `Grouped`.
        if (s - 1.0).abs() > 1e-3 {
            let scale_len = |l: crate::style::values::value::Len| match l {
                crate::style::values::value::Len::Px(v) => {
                    crate::style::values::value::Len::Px(v * s)
                }
                other => other,
            };
            style.mask_user_scale = s;
            style.mask_size = style.mask_size.map(|(x, y)| (scale_len(x), scale_len(y)));
            style.mask_pos = style.mask_pos.map(|(x, y)| (scale_len(x), scale_len(y)));
            if let Some(list) = style.mask_pos_list.as_mut() {
                for (x, y, _, _) in list.iter_mut() {
                    *x = scale_len(*x);
                    *y = scale_len(*y);
                }
            }
        }
        let style = &style;
        // Срез по рамке: внешняя канва размером с рамку, `viewBox` — её
        // окно в CSS-точках коробки рисунка (вложенный `<svg>` — вьюпорт
        // с прежними единицами).
        let part = filtered(e, &|m| m == u);
        let markup = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{bw}" height="{bh}" viewBox="{bx} {by} {bw} {bh}">{}</svg>"#,
            serialize_sized(&part, rw, rh, 0.0, 0.0)
        );
        let Some(img) = rasterize(&markup, *bw, *bh) else {
            continue;
        };
        let layer = gpui::img(ImageSource::Render(img))
            .w(gpui::px(*bw))
            .h(gpui::px(*bh))
            .into_any_element();
        let layer = crate::paint::effects::grouped::grouped(layer, style);
        root = root.child(
            gpui::div()
                .absolute()
                .top(gpui::px(*by))
                .left(gpui::px(*bx))
                .w(gpui::px(*bw))
                .h(gpui::px(*bh))
                .child(layer),
        );
    }
    root = flush(&mut run, root);
    Some(root.into_any_element())
}
