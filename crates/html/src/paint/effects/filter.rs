//! Фильтры.
// owner: A

use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window, px};

/// Слой `filter: url(#id)`: SVG с прямоугольником цвета фона под этим
/// фильтром, растрированный resvg по размеру коробки. Холст вдвое больше
/// коробки, прямоугольник в центре — область фильтра по умолчанию выходит
/// за коробку на десятую часть с каждой стороны, заданная — сколько
/// угодно.
pub struct FilterLayer {
    pub def: String,
    pub id: String,
    pub fill: String,
}

impl Hooks for FilterLayer {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            inset: gpui::Edges {
                top: gpui::px(0.0).into(),
                right: gpui::px(0.0).into(),
                bottom: gpui::px(0.0).into(),
                left: gpui::px(0.0).into(),
            },
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::Prepaint {
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if w < 0.0 || h < 0.0 {
            return;
        }
        // Холст в системе коробки: полкоробки запаса с каждой стороны
        // покрывают умолчание −10 %/120 %, а область `userSpaceOnUse` задана
        // от угла коробки и может лежать где угодно — холст расширяется до
        // неё (у коробки 0×0 он из неё одной и состоит,
        // `empty-element-with-filter-002/004`).
        let (mut x0, mut y0, mut x1, mut y1) = (-0.5 * w, -0.5 * h, 1.5 * w, 1.5 * h);
        // `calc()` в области (`filter-region-calc-001`) usvg не читает —
        // она сводится к числу заранее, для холста и для растеризатора.
        let def = super::filter_region::resolve_calc(&self.def, w, h);
        let [rx, ry, rw, rh] = super::filter_region::filter_region(&def, w, h);
        if rw.is_finite() && rh.is_finite() && rx.is_finite() && ry.is_finite() {
            x0 = x0.min(rx).max(-4096.0);
            y0 = y0.min(ry).max(-4096.0);
            x1 = x1.max(rx + rw).min(4096.0);
            y1 = y1.max(ry + rh).min(4096.0);
        }
        // Края холста — на точках устройства: растр ложится один к одному,
        // без пересэмплирования. Полкоробки запаса при масштабе 1.25 давали
        // дробный угол (−62.5 точки), и вся заливка расплывалась на
        // полточки (`filter-chained-url-url-001`, `svg-feimage-005`).
        let sf = window.scale_factor();
        let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let x0 = ((ox + x0) * sf).floor() / sf - ox;
        let y0 = ((oy + y0) * sf).floor() / sf - oy;
        let x1 = ((ox + x1) * sf).ceil() / sf - ox;
        let y1 = ((oy + y1) * sf).ceil() / sf - oy;
        let (cw, ch) = (x1 - x0, y1 - y0);
        if cw <= 0.0 || ch <= 0.0 {
            return;
        }
        // Начало пользовательских координат — угол коробки (сдвиг группы).
        // У коробки 0×0 фильтр — на группе: `<rect>` нулевого размера
        // выключает отрисовку (SVG 2 §10.2), а `feFlood` даёт выход и без
        // источника.
        let body = if w > 0.0 && h > 0.0 {
            format!(
                r##"<rect width="{w}" height="{h}" fill="{}" filter="url(#{})"/>"##,
                self.fill, self.id
            )
        } else {
            format!(r##"<g filter="url(#{})"/>"##, self.id)
        };
        let markup = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}" viewBox="0 0 {cw} {ch}"><defs>{}</defs><g transform="translate({} {})">{body}</g></svg>"##,
            def, -x0, -y0
        );
        let (pw, ph) = ((cw * sf).round(), (ch * sf).round());
        let image = crate::svg::raster::rasterize(&markup, pw, ph);
        let Some(image) = image else { return };
        let area = Bounds {
            origin: gpui::point(bounds.origin.x + px(x0), bounds.origin.y + px(y0)),
            size: gpui::size(px(cw), px(ch)),
        };
        let _ = window.paint_image_with_sampling(
            area,
            gpui::Corners::default(),
            image,
            0,
            false,
            gpui::ImageSampling::Linear,
        );
    }
}
