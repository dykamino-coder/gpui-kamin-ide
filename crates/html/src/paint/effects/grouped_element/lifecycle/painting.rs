//! Окраска сгруппированного поддерева и его масок.

mod area;
pub(super) use area::group_area;

mod masks;
pub(super) use masks::resolve_mask;

use crate::paint::effects::grouped_element::{Grouped, mask_layer_source};
use crate::paint::effects::{polygon_clip, rectangular_clip};
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, LayoutId, Pixels, Window, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    group: &mut Grouped,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>,
    _state: &mut LayoutId,
    _prepaint: &mut (Bounds<Pixels>, Bounds<Pixels>),
    window: &mut Window,
    cx: &mut App,
) {
    // Слой маски-картинки, которая НЕ загрузилась (файла нет, формат не
    // читается), — «image layer of transparent black» (css-masking-1
    // §7.1). Все слои такие — элемент скрыт целиком (`mask-image-4a`:
    // `url(non-existent.png)` рисовался без маски). Ссылки на
    // определения, градиенты и формы сюда не входят.
    if let Some(src) = group.mask.as_deref()
        && src.contains("url(")
    {
        let layers: Vec<String> = crate::style::css::split_args(src)
            .iter()
            .filter_map(|l| mask_layer_source(l))
            .collect();
        let plain = |l: &str| {
            !l.contains("-gradient(")
                && !l.contains("snap:")
                && !l.contains("def:")
                && !l.contains('#')
                && !l.starts_with("data:")
                // Только ЛОКАЛЬНЫЙ файл без схемы и запроса: `invalid://`
                // у SVG-элемента одиночным слоем игнорируется
                // (`bad-mask-image-svg-2/3`), а серверный путь с
                // `?pipe=` стенду недоступен вовсе
                // (`mask-image-svg-loading-error`) — их держит прежняя
                // ветка «маски нет».
                && !l.contains("://")
                && !l.contains('?')
        };
        if !layers.is_empty()
            && layers
                .iter()
                .all(|l| plain(l) && crate::paint::background::source(l).is_none())
        {
            return;
        }
    }
    // Размытая картинка выходит за края элемента — в браузере тоже.
    // Вчетверо шире радиуса: маска композита обязана лежать там, где
    // размытая картинка уже сошла на нет, иначе край режется прямоугольником.
    let (area, (sl, st, sr, sb)) = group_area(group, bounds, window);
    // Вершины считаются от ОПОРНОЙ коробки формы (bounds ± края:
    // margin-box шире, content-box уже); проценты — доли её сторон,
    // точки — как есть (clip-path-polygon-008).
    let (polygon, polygon_clip) = polygon_clip::geometry(
        group,
        bounds,
        _prepaint.0,
        window.scale_factor(),
        window.current_transformation() == gpui::TransformationMatrix::unit(),
    );
    // Плитка маски: у растра — его точки как CSS-точки (density 1), у
    // рисунка без размера и градиента — сама коробка (mask-size auto,
    // css-masking §7.4); `mask-size` подменяет размер, `mask-position`
    // смещает (доля — от свободного места, как у background-position).
    // Битый источник — маски нет, элемент виден целиком
    // (bad-mask-image-svg-*).
    // Полигон сверх восьми вершин (предел шейдера) или с `evenodd` —
    // растровой маской-путём в системе коробки (clip-path-polygon-004/005).
    let poly_mask = if group.mask.is_none() && (polygon.len() > 8 || group.polygon_evenodd) {
        let d: Vec<String> = polygon
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "{}{} {}",
                    if i == 0 { "M" } else { "L" },
                    f32::from(p.x - bounds.origin.x),
                    f32::from(p.y - bounds.origin.y)
                )
            })
            .collect();
        let rule = if group.polygon_evenodd {
            "evenodd"
        } else {
            "nonzero"
        };
        Some(format!("pathdef:{rule}:{} Z", d.join(" ")))
    } else {
        None
    };
    let polygon = if poly_mask.is_some() {
        Vec::new()
    } else {
        polygon
    };
    let mask = group
        .mask
        .as_deref()
        .or(poly_mask.as_deref())
        .and_then(|src| resolve_mask(src, group, bounds, _prepaint, window, sl, st, sr, sb));
    // Коробка окраски (`mask-clip`): вне её маска не красится — элемент
    // там скрыт (mask-size-contain-clip-padding).
    let mask_clip = polygon_clip::intersect(
        rectangular_clip::resolve(group, _prepaint.0, window.scale_factor()),
        polygon_clip,
    );
    // Подложка (наружные тени `border-shape`) — в текущий контекст ДО
    // композита группы: под буфером и вне его маски, на области выноса.
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let (aw, ah) = (bw + sl + sr, bh + st + sb);
    let layer_at = Bounds {
        origin: gpui::point(bounds.origin.x - px(sl), bounds.origin.y - px(st)),
        size: gpui::size(px(aw), px(ah)),
    };
    if let Some(under) = group.under.as_ref()
        && let Some(markup) = under(bw, bh, sl, st, aw, ah)
        && let Some(img) = crate::svg::raster::rasterize(&markup, aw, ah)
    {
        let _ = window.paint_image_with_sampling(
            layer_at,
            gpui::Corners::default(),
            img,
            0,
            false,
            gpui::ImageSampling::Linear,
        );
    }
    // css-shapes-1 §basic-shape-rect: `round` rounds the corners of the
    // clip rectangle itself; the composite quad carries those radii.
    // Percentages: Blink BasicShapeInset resolves radii against the
    // reference box; one scalar radius per corner takes the smaller axis.
    let round = match group.clip_round {
        Some(crate::style::values::value::Len::Px(v)) => v,
        Some(crate::style::values::value::Len::Pct(p)) => {
            p * f32::from(_prepaint.0.size.width).min(f32::from(_prepaint.0.size.height))
        }
        _ => 0.0,
    };
    let (area, corners) = match mask_clip {
        Some([x, y, w, h])
            if round > 0.0 && mask.is_none() && polygon.is_empty() && w > 0.0 && h > 0.0 =>
        {
            let sf = window.scale_factor();
            let (w, h) = (w / sf, h / sf);
            (
                Bounds {
                    origin: gpui::point(px(x / sf), px(y / sf)),
                    size: gpui::size(px(w), px(h)),
                },
                gpui::Corners::all(px(round.min(w / 2.0).min(h / 2.0))),
            )
        }
        _ => (area, gpui::Corners::default()),
    };
    let child = group.child.as_mut().unwrap();
    window.paint_group(
        area,
        corners,
        group.blur,
        group.opacity,
        group.blend,
        &polygon,
        mask,
        mask_clip,
        |window| child.paint(window, cx),
    );
    // Накладка (кольцо `border-shape` над обрезанным содержимым) — после
    // композита, в тот же контекст.
    for over in &group.over {
        if let Some(markup) = over(bw, bh, sl, st, aw, ah)
            && let Some(img) = crate::svg::raster::rasterize(&markup, aw, ah)
        {
            let _ = window.paint_image_with_sampling(
                layer_at,
                gpui::Corners::default(),
                img,
                0,
                false,
                gpui::ImageSampling::Linear,
            );
        }
    }
}
