//! Фильтры.
// owner: A

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window, px,
};

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

impl IntoElement for FilterLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

thread_local! {
    /// Растры слоёв фильтра по (разметка, размер): кадр за кадром одно и то же.
    static FILTER_RASTERS: std::cell::RefCell<
        std::collections::HashMap<(String, u32, u32), std::sync::Arc<gpui::RenderImage>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Область `<filter filterUnits="userSpaceOnUse">` в точках от угла коробки
/// (filter-effects-1 §5: «origin at the top-left corner of the bounding
/// client rect … viewport … width and height of the bounding client rect»);
/// проценты — от размера коробки. Для `objectBoundingBox` — None: такая
/// область и так внутри холста −50 %…150 %.
fn user_space_region(def: &str, w: f32, h: f32) -> Option<[f32; 4]> {
    let open = &def[def.find("<filter")?..];
    let open = &open[..open.find('>')?];
    if !open.contains("userSpaceOnUse") {
        return None;
    }
    let attr = |name: &str| -> Option<&str> {
        let key = format!(" {name}=\"");
        let at = open.find(&key)? + key.len();
        let rest = &open[at..];
        Some(&rest[..rest.find('"')?])
    };
    let num = |name: &str, base: f32, def: f32| -> f32 {
        match attr(name).map(str::trim) {
            Some(v) if v.ends_with('%') => v[..v.len() - 1]
                .trim()
                .parse::<f32>()
                .map_or(def, |p| p / 100.0 * base),
            Some(v) => v.trim_end_matches("px").trim().parse().unwrap_or(def),
            None => def,
        }
    };
    Some([
        num("x", w, -0.1 * w),
        num("y", h, -0.1 * h),
        num("width", w, 1.2 * w),
        num("height", h, 1.2 * h),
    ])
}

impl Element for FilterLayer {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset = gpui::Edges {
            top: gpui::px(0.0).into(),
            right: gpui::px(0.0).into(),
            bottom: gpui::px(0.0).into(),
            left: gpui::px(0.0).into(),
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
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
        if let Some([rx, ry, rw, rh]) = user_space_region(&self.def, w, h) {
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
            self.def, -x0, -y0
        );
        let (pw, ph) = ((cw * sf).round(), (ch * sf).round());
        let key = (markup.clone(), pw as u32, ph as u32);
        let image = FILTER_RASTERS
            .with(|m| m.borrow().get(&key).cloned())
            .or_else(|| {
                let img = crate::svg::rasterize(&markup, pw, ph)?;
                FILTER_RASTERS.with(|m| m.borrow_mut().insert(key.clone(), img.clone()));
                Some(img)
            });
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
