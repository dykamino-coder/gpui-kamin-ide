//! `backdrop-filter` цветовой матрицей (filter-effects-2 §BackdropFilterProperty).
// owner: A

use crate::paint::effects::mask::{mask_def, svg_filter_matrix};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{IntoElement, Styled, px};

pub(super) fn backdrop_matrix(
    c: &Computed,
    out: &mut Vec<gpui::AnyElement>,
) {
    // `backdrop-filter` с цветовыми функциями (filter-effects-2
    // §BackdropFilterProperty: `<filter-value-list>` как у `filter`) — матрица
    // 4×5 тем же проходом подложки. Область — border-box со скруглением
    // (§3 шаг 3: «Apply a clip to the contents of T', using the border box
    // shape of B, with border-radius»): абсолютный слой отсчитывается от
    // padding-box, рамка выносится отрицательными краями. Под корнем
    // подложки у предка (§BackdropRoot) матрица не рисуется: наш кадр — не
    // «Backdrop Root Image» такого корня, копия внесла бы в подложку то, что
    // лежит под корнем (`backdrop-filter-backdrop-root-*`).
    // `backdrop-filter: url(#id)` (filter-effects-2 §BackdropFilterProperty:
    // `<filter-value-list>` допускает `<url>`): SVG `<filter>` из одного
    // примитива с аффинной формулой — та же матрица (`svg_filter_matrix`;
    // `backdrop-filter-svg`).
    let matrix = c
        .backdrop_color
        .and_then(|f| f.color_matrix())
        .or_else(|| {
            c.backdrop_ref
                .as_deref()
                .and_then(|id| mask_def(&format!("filter:{id}")))
                .and_then(|def| svg_filter_matrix(&def))
        })
        .filter(|_| !c.backdrop_root_above);
    if let Some(m) = matrix {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let radius = c.backdrop_blur.unwrap_or(0.0);
        // Своя `mask-image` делает элемент группой (`grouped`), а буфер группы
        // рисуется до кадра: подложку поднимает в кадр `paint_backdrop_filter`
        // (filter-effects-2 Overview.bs:61-66, шаги 1-5;
        // `backdrop-filter-good-and-bad-mask-image`). Форма `clip-path` —
        // нет: без обрезки формой подъём дал бы подложку во всю коробку.
        let hoist = c.mask_image.is_some()
            && c.clip_shape.is_none()
            && c.clip_polygon.is_none()
            && c.clip_inset.is_none()
            && c.clip_edges.is_none()
            && c.clip_xywh.is_none()
            && !c.clip_bare_box;
        let corners = [
            side(c.radius.tl),
            side(c.radius.tr),
            side(c.radius.br),
            side(c.radius.bl),
        ];
        let b = c.borders();
        out.push(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let [tl, tr, br, bl] = corners;
                    window.paint_backdrop_filter(
                        bounds,
                        gpui::Corners {
                            top_left: px(tl),
                            top_right: px(tr),
                            bottom_right: px(br),
                            bottom_left: px(bl),
                        },
                        radius,
                        m,
                        hoist,
                    );
                },
            )
            .absolute()
            .top(px(-side(b.top)))
            .right(px(-side(b.right)))
            .bottom(px(-side(b.bottom)))
            .left(px(-side(b.left)))
            .into_any_element(),
        );
    } else if let Some(radius) = c.backdrop_blur {
        // `backdrop-filter: blur(N)`: размывает то, что под элементом. Рисуется
        // проходом рендера (патч gpui), поэтому это канвас, а не стиль.
        let corner = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        out.push(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    window.paint_backdrop_blur_radius(
                        bounds,
                        gpui::Corners::all(px(corner)),
                        radius,
                    );
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element(),
        );
    }
}
