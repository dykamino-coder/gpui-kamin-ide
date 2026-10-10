//! Prepaint for element; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{
    App, Bounds, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, point, px,
};

impl Paragraph {
    pub(crate) fn prepaint_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        // Предел переноса — длина строки по её физической оси.
        self.vertical_layout_origin =
            window.layout_origin_unrounded(*state) - window.element_offset();
        self.width_nudge = {
            let dw = window.layout_size_unrounded(*state).width - bounds.size.width;
            let one = 1.0 / window.scale_factor().max(0.01) + 1e-4;
            if self.vertical || f32::from(dw).abs() > one {
                px(0.0)
            } else {
                dw
            }
        };
        // Snapping moves an edge by at most half a device pixel; anything
        // larger means the node was placed outside its tree (`prepaint_at`).
        self.glyph_nudge = {
            let d = window.layout_origin_unrounded(*state) - bounds.origin;
            let half = 0.5 / window.scale_factor().max(0.01) + 1e-4;
            let ok = |v: Pixels| f32::from(v).abs() <= half;
            if self.vertical || !(ok(d.x) && ok(d.y)) {
                point(px(0.0), px(0.0))
            } else {
                d
            }
        };
        let limit = if self.vertical {
            bounds.size.height
        } else {
            bounds.size.width
        };
        // Коробка приходит округлённой ВНИЗ до точки устройства, а мерили её
        // по дробной ширине: строка, влезавшая ровно, на отрисовке уже не
        // влезала и рвалась заново (`hyphens-manual-011`). Возвращаем себе эту
        // одну точку устройства — иначе раскладка кадра расходится с замером.
        let scale = window.scale_factor().max(1.0);
        let limit = limit + px(1.0 / scale);
        self.indent_basis = {
            let exact = window.layout_size_unrounded(*state);
            let exact = if self.vertical {
                exact.height
            } else {
                exact.width
            };
            let snapped = if self.vertical {
                bounds.size.height
            } else {
                bounds.size.width
            };
            (f32::from(exact - snapped).abs() <= 1.0 / scale + 1e-4).then_some(exact)
        };
        self.apply_measured_fit();
        if !self.vertical {
            let avail = f32::from(window.layout_size_unrounded(*state).width);
            self.refit_atoms(avail, window, _cx);
        }
        self.lines = self.split(Some(limit), window);
        self.unbalanced_steps = None;
        if self.clamp_tag.is_some() && self.wrap.balance && self.clamp.is_none() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            let segs = self.measure(window);
            let unbalanced = self.lay(Some(limit), &segs);
            let balanced = std::mem::replace(&mut self.lines, unbalanced);
            let lh = f32::from(self.line_height);
            self.unbalanced_steps = Some(
                self.line_padding()
                    .iter()
                    .map(|(a, b)| lh + a + b)
                    .collect(),
            );
            self.lines = balanced;
        }
        self.place_atoms(*state, window, _cx);
        // Куски вне потока встают на своё место в строке: раскладываются
        // по содержимому и подготавливаются от угла своего знака.
        if !self.overlays.is_empty() {
            let segs = self.measure(window);
            let mut placed = std::mem::take(&mut self.overlays);
            let rotated = crate::text::vertical::in_rotated_frame();
            let scale = window.scale_factor().max(0.01);
            for (at, el, how) in placed.iter_mut() {
                // Абсолют от строчного содержащего блока: края считает
                // раскладка от коробки размером в этот блок (`inline_cb_rect`).
                if let Some(cb) = how.cb.filter(|_| !rotated) {
                    let s = (*at as isize + cb.start).max(0) as usize;
                    let e = (*at as isize + cb.end).max(0) as usize;
                    if let Some(r) = self.inline_cb_rect(&segs, s, e, cb.pad, bounds) {
                        use gpui::{ParentElement, Styled};
                        let inner = std::mem::replace(el, gpui::Empty.into_any_element());
                        *el = gpui::div()
                            .relative()
                            .w(r.size.width)
                            .h(r.size.height)
                            .child(inner)
                            .into_any_element();
                        el.layout_as_root(
                            gpui::size(
                                gpui::AvailableSpace::Definite(r.size.width),
                                gpui::AvailableSpace::Definite(r.size.height),
                            ),
                            window,
                            _cx,
                        );
                        let o = point(r.origin.x + px(cb.shift.0), r.origin.y + px(cb.shift.1));
                        el.prepaint_at(o, window, _cx);
                        continue;
                    }
                }
                let next = &how.next_line;
                let origin = if *next {
                    self.next_line_point(*at, bounds)
                } else {
                    self.point_of(&segs, *at, bounds)
                };
                // Повёрнутый абзац: до-поворотная y — блочная ось экрана.
                // Коробка ложится краем туда же, куда глиф соседнего текста:
                // глиф — целая часть физической точки (`paint_glyph`), а
                // раскладка округлила бы до ближайшей. Расхождение в точку
                // оставляло столбец красного (`static-position/vlr-*`).
                let origin = if rotated {
                    let y = f32::from(origin.y + px(how.rot_dy)) * scale;
                    point(origin.x + px(how.rot_dx), px((y + 1e-3).floor() / scale))
                } else {
                    origin
                };
                // Ширина абсолютного элемента — «по содержимому» (CSS 2.1
                // §10.3.7): по МИНИМАЛЬНОМУ содержимому он рвался бы по
                // словам (`static-position/htb-*`).
                let size = el.layout_as_root(
                    gpui::size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    _cx,
                );
                // Блочная коробка при `rtl` вешается ПРАВЫМ краем на правый
                // край содержимого (§10.3.7: `right` = статическая позиция).
                let origin = if *next && self.wrap.rtl {
                    // `origin.x` здесь — край содержимого плюс сдвиг предков.
                    point(origin.x + bounds.size.width - size.width, origin.y)
                } else if how.bidi_hang && self.rtl_level_at(*at) {
                    point(origin.x - size.width, origin.y)
                } else {
                    origin
                };
                el.prepaint_at(origin, window, _cx);
            }
            self.overlays = placed;
        }
        self.id
            .is_some()
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }
}
