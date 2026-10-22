//! `prepaint` абзаца: округление, подбор кегля, атомы, строки, куски вне потока, выделение.

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
        // Larger shifts than half a device pixel come from `prepaint_at`.
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
        // Restore the device pixel lost by downward snapping so a line that
        // fitted the fractional measured width does not wrap during prepaint.
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
        self.publish_scroll_overflow(bounds, *state, window);
        self.place_atoms(*state, window, _cx);
        // Куски вне потока встают на своё место в строке: раскладываются
        // по содержимому и подготавливаются от угла своего знака.
        if !self.overlays.is_empty() {
            let segs = self.measure(window);
            let mut placed = std::mem::take(&mut self.overlays);
            // End of the text kept by `line-clamp`: an inline containing
            // block starting at or after it follows the clamp point, and
            // its absolute boxes are hidden with it (css-overflow-4 §5.3;
            // `line-clamp-with-abspos-015/017`). One displaced by the
            // ellipsis counts as after the point too.
            let clamp_end = self
                .lines
                .last()
                .filter(|l| l.clamped && l.range.end < self.text.len())
                .map(|l| l.range.end);
            self.overlay_hidden = placed
                .iter()
                .map(|(at, _, how)| {
                    clamp_end.is_some_and(|end| {
                        how.cb
                            .as_ref()
                            .is_some_and(|cb| (*at as isize + cb.start).max(0) as usize >= end)
                    })
                })
                .collect();
            let rotated = self.paint_context.is_rotated();
            let scale = window.scale_factor().max(0.01);
            // Строчные якоря и содержащие блоки — до куска внутри них:
            // носитель краёв стоит в конце коробки, а абсолют — раньше.
            for (at, _, how) in placed.iter() {
                if let Some(b) = how.inline_box.filter(|_| !rotated)
                    && let Some(info) = &how.inline_box_info
                {
                    self.register_inline_box(&segs, *at, b, info, bounds, window);
                }
            }
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
                // An inline-level box after a forced break with nothing after
                // it: its hypothetical box is alone on the NEXT line, which
                // the paragraph never formed (CSS 2.1 §10.6.4;
                // `hypothetical-inline-alone-on-second-line`).
                let past_break = !rotated
                    && !self.wrap.rtl
                    && self.lines.last().is_some_and(|l| *at >= l.range.end)
                    && self.text.get(..*at).is_some_and(|t| {
                        t.trim_end_matches([' ', '\u{feff}', '\u{200b}'])
                            .ends_with('\n')
                    });
                let next = &(how.next_line || past_break);
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
                if let Some(slot) = how.line_probe.as_ref().filter(|_| !rotated) {
                    // The line box height over its inline boxes (CSS 2.1
                    // §10.8.1), not the paragraph's uniform pitch: an inline
                    // ancestor's taller `line-height` grows the line.
                    let row = self
                        .lines
                        .iter()
                        .position(|l| *at < l.range.end)
                        .unwrap_or(self.lines.len().saturating_sub(1));
                    let (lh, start) = match self.lines.get(row) {
                        Some(l) => {
                            let (a, b) = self.line_extents(&l.range.clone());
                            let start = if *next {
                                bounds.origin.x
                            } else {
                                self.point_of(&segs, l.range.start, bounds).x
                            };
                            (px(a + b).max(self.line_height), start)
                        }
                        None => (self.line_height, bounds.origin.x),
                    };
                    let line = (origin.y, lh, start);
                    crate::layout::positioned::containing_block::with_line_box(slot, line, || {
                        el.prepaint_at(origin, window, _cx);
                    });
                } else {
                    el.prepaint_at(origin, window, _cx);
                }
            }
            self.overlays = placed;
        }
        self.id
            .is_some()
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }
}
