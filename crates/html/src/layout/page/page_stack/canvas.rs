//! Image layers of the document canvas on every sheet (css-page-3 §painting).
//!
//! The root/body background propagates to the canvas (CSS 2.1 §14.2), and in
//! paged media the canvas of each page is painted over the page box's border
//! box, above the `@page` background (css-page-3 §page-backgrounds-and-painting-order,
//! layer 2). The colour is a plain quad (`PageGeom::canvas`); gradients and images
//! are a real box of that size so the regular background painter positions,
//! tiles and clips them. Blink paints the same layer in `ViewPainter`
//! (`PaintRootElementGroup`) per page.

use super::{PageGeom, PageStack};
use crate::dom::{Element, Node};
use crate::render::{RenderOpts, blocks};
use crate::style::computed::Computed;
use gpui::{AnyElement, App, Bounds, IntoElement, ParentElement, Pixels, Window, point, px, size};

/// Builds the canvas image layer for a border box of `(w, h)` points.
pub type CanvasLayerFn = std::rc::Rc<dyn Fn(f32, f32) -> AnyElement>;

/// The image part of a propagated background: everything but the colour,
/// which `PageGeom::canvas` paints. `None` when there is no image layer.
pub(crate) fn image_layers(s: &Computed) -> Option<Computed> {
    if s.gradient.is_none() && s.bg_image.is_none() {
        return None;
    }
    Some(Computed {
        background_rcs: s.background_rcs.clone(),
        gradient: s.gradient.clone(),
        gradient_raw: s.gradient_raw.clone(),
        bg_image: s.bg_image.clone(),
        bg_size: s.bg_size,
        bg_pos: s.bg_pos,
        bg_repeat: s.bg_repeat,
        bg_clip: s.bg_clip,
        bg_origin: s.bg_origin,
        bg_fixed: s.bg_fixed,
        bg_lists: s.bg_lists.clone(),
        ..Default::default()
    })
}

/// A builder that lays the image layers out as a `w × h` block.
pub(crate) fn builder(layers: Computed, root: Computed, opts: RenderOpts) -> CanvasLayerFn {
    std::rc::Rc::new(move |w, h| {
        let mut st = layers.clone();
        for (k, v) in [
            ("width", format!("{w}px")),
            ("height", format!("{h}px")),
            ("margin", "0".to_string()),
        ] {
            st.apply_one(k, &v);
        }
        let el = Element {
            tag: "div".to_string(),
            inline: false,
            node_id: 0,
            style: st,
            hover: None,
            first_letter: None,
            first_line: None,
            children: vec![],
            attrs: vec![],
            anim: Default::default(),
            list_item: None,
        };
        let mut els = blocks(&[Node::Element(el)], &root, &opts);
        match els.pop() {
            Some(el) if els.is_empty() => el,
            Some(el) => {
                els.push(el);
                gpui::div().children(els).into_any_element()
            }
            None => gpui::div().into_any_element(),
        }
    })
}

/// The canvas rectangle of a sheet: its page border box, but not wider than
/// the sheet itself (negative page margins), relative to the sheet origin.
pub(super) fn canvas_rect(g: &PageGeom) -> (f32, f32, f32, f32) {
    let bx = g.margin[3];
    let by = g.margin[0];
    let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
    let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
    let (cx, cy) = (bx.max(0.0), by.max(0.0));
    let cw = ((bx + bw).min(g.size.0) - cx).max(0.0);
    let ch = ((by + bh).min(g.size.1) - cy).max(0.0);
    (cx, cy, cw, ch)
}

impl PageStack {
    /// Paint the canvas image layers built by `layer` on every sheet.
    pub fn with_canvas_layer(mut self, layer: Option<CanvasLayerFn>) -> Self {
        self.canvas_layer = layer;
        self
    }

    /// Lays out one canvas layer per painted sheet (after the page count is known).
    pub(super) fn prepaint_canvas(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        geoms: &[PageGeom],
    ) {
        self.canvas_els.clear();
        let Some(make) = self.canvas_layer.clone() else {
            return;
        };
        // The background positioning area is the root box, fragmented like
        // the flow (css-break-3 §box-decoration-break `slice`): sheet `p`
        // shows it shifted up by the page areas before it, so a `no-repeat`
        // image appears on the first page only (`page-background-001..003`).
        let mut offset = 0.0f32;
        for (p, g) in geoms.iter().enumerate() {
            let (x, y, w, h) = canvas_rect(g);
            let (sx, sy) = self.sheet_origin(p);
            let (y, h) = (y - offset, h + offset);
            offset += g.area.1;
            let mut el = make(w, h);
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(w)),
                    gpui::AvailableSpace::Definite(px(h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(
                point(bounds.origin.x + px(sx + x), bounds.origin.y + px(sy + y)),
                window,
                cx,
            );
            self.canvas_els.push((p, el));
        }
    }

    /// Paints the canvas layer of sheet `p`, clipped to its canvas rectangle.
    pub(super) fn paint_canvas(
        &mut self,
        p: usize,
        mask: Bounds<Pixels>,
        bounds: Bounds<Pixels>,
        s: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (q, el) in &mut self.canvas_els {
            if *q != p {
                continue;
            }
            let mask = gpui::ContentMask { bounds: mask };
            window.with_content_mask(Some(mask), |window| {
                window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
            });
        }
    }
}
