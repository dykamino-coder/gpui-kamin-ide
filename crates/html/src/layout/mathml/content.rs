//! Содержимое с учётом baseline: строки MathML сами размещают своих потомков.

use super::geometry::Metrics;
use super::{arrangement, parameters::Kind};
use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, point, px, size};

pub(super) struct Content {
    pub values: crate::document::state::ValueDoc,
    pub children: Vec<AnyElement>,
    pub kind: Kind,
    pub factories: Vec<Option<(u8, super::stretch::Factory)>>,
    pub operator: Option<(char, super::stretch::Target)>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub rtl: bool,
    pub offsets: Vec<(f32, f32)>,
    /// Physical `[top, right, bottom, left]` margins of each child.
    pub margins: Vec<[f32; 4]>,
    pub roots: Vec<LayoutId>,
    pub rule: arrangement::Rule,
    pub color: gpui::Hsla,
    pub font: gpui::Font,
    pub font_size: f32,
    pub glyph: Option<super::glyph::Plan>,
    pub glyph_offset: (f32, f32),
}

impl Hooks for Content {
    type Layout = LayoutId;
    type Prepaint = gpui::Point<Pixels>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        self.roots.clear();
        let metrics = if let Kind::Space(leaf) = self.kind {
            leaf
        } else {
            let mut children = Vec::with_capacity(self.children.len());
            for child in &mut self.children {
                let (root, metrics) = super::stretch::measure(child, window, cx);
                children.push(metrics);
                self.roots.push(root);
            }
            super::stretch::constrain(
                self.kind,
                &mut self.children,
                &self.factories,
                &mut children,
                &mut self.roots,
                window,
                cx,
            );
            let margins = (self.margins.len() == children.len()).then_some(&self.margins);
            for (m, [t, r, b, l]) in children.iter_mut().zip(margins.into_iter().flatten()) {
                m.width += l + r;
                m.ascent += t;
                m.descent += b;
            }
            if let Some((ch, target)) = self.operator {
                self.glyph = super::glyph::Plan::new(
                    &self.font,
                    self.font_size,
                    ch,
                    target.length,
                    target.vertical,
                    window,
                    self.values.vcx(),
                );
                if let Some(glyph) = &self.glyph {
                    self.children.clear();
                    self.roots.clear();
                    children = vec![glyph.metrics];
                }
            }
            let (metrics, offsets, rule) = if let Kind::Radical(p) = self.kind {
                let base = super::radical::base(&children, p, self.rtl);
                self.glyph = super::glyph::Plan::new(
                    &self.font,
                    self.font_size,
                    '\u{221a}',
                    base.ascent + base.descent + p.gap + p.rule,
                    true,
                    window,
                    self.values.vcx(),
                );
                let surd = self
                    .glyph
                    .as_ref()
                    .map_or(Metrics::default(), |g| g.metrics);
                let (metrics, offsets, rule, origin) =
                    super::radical::layout(&children, surd, p, self.rtl);
                self.glyph_offset = origin;
                (metrics, offsets, rule)
            } else {
                arrangement::arrange(&children, self.kind, self.rtl, self.width, self.height)
            };
            if self.operator.is_some() && self.glyph.is_some() {
                self.glyph_offset = offsets.first().copied().unwrap_or_default();
            }
            self.offsets = offsets;
            if self.margins.len() == self.offsets.len() {
                for ((x, y), [t, _, _, l]) in self.offsets.iter_mut().zip(&self.margins) {
                    *x += l;
                    *y += t;
                }
            }
            self.rule = rule;
            metrics
        };
        let id = window.request_measured_layout_with_baselines(
            gpui::Style::default(),
            move |known, _, _, _| {
                (
                    size(
                        known.width.unwrap_or(px(metrics.width)),
                        known.height.unwrap_or(px(metrics.ascent + metrics.descent)),
                    ),
                    Some(px(metrics.ascent)),
                    Some(px(metrics.ascent)),
                )
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        // Absolute unrounded origin: `layout_exact` is relative to the parent.
        let origin = window.layout_origin_unrounded(*state);
        for ((child, root), (x, y)) in self.children.iter_mut().zip(&self.roots).zip(&self.offsets)
        {
            window.set_layout_root_origin(*root, origin + point(px(*x), px(*y)));
            let cancel = -window.element_offset();
            window.with_exact_element_offset(cancel, |window| child.prepaint(window, cx));
        }
        // GPUI forbids `element_offset` during paint: keep the origin here.
        origin
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        origin: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = *origin;
        if let Some((x, y, w, h)) = self.rule {
            window.paint_quad(gpui::fill(
                Bounds::new(origin + point(px(x), px(y)), size(px(w), px(h))),
                self.color,
            ));
        }
        if let Some(glyph) = &self.glyph {
            glyph.paint(
                origin + point(px(self.glyph_offset.0), px(self.glyph_offset.1)),
                self.color,
                window,
            );
        }
        for child in &mut self.children {
            child.paint(window, cx);
        }
    }
}
