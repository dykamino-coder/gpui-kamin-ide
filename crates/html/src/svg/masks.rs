//! CSS-маски на детях `<svg>`: срезы разметки по порядку отрисовки и слои.

mod units;
use units::Rect;
use units::Unit;
use units::filtered;
use units::masked_units;
use units::non_rendering;
use units::user_to_css;

use super::raster::rasterize;
use super::serialize::serialize_sized;
use crate::dom::{Element, Node};
use gpui::{AnyElement, ImageSource, IntoElement};

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
