//! Fragmented ancestor edges and collapsed end margins at a clamp point.

/// A probed block box inside the clamp container, in box-tree preorder.
#[derive(Debug)]
pub(crate) struct ClampBox {
    /// Border-box top and padding-box bottom.
    pub y0: f32,
    pub y1: f32,
    /// Definite height: not fragmented at the clamp point.
    pub fixed: bool,
    /// End border + padding and end margin.
    pub bp: f32,
    pub margin: f32,
    /// `min-height` border-box extent from `y0`, 0 when none.
    pub min_end: f32,
    pub node: u64,
    /// Paragraph numbers `[first, end)` of its descendants.
    pub span: Option<(u32, u32)>,
}

impl ClampBox {
    /// Inside a monolithic ancestor that ends before `c`: its overflow is
    /// that ancestor's, parked at the ancestor's end, so the box can
    /// neither hold nor push the clamp point.
    pub(super) fn inside_fitting_fixed(&self, all: &[ClampBox], c: f32) -> bool {
        let Some((a, z)) = self.span else {
            return false;
        };
        all.iter().any(|o| {
            !std::ptr::eq(o, self)
                && o.fixed
                && o.y1 <= c + 0.5
                && o.y0 <= self.y0
                && o.span.is_some_and(|(oa, oz)| oa <= a && z <= oz)
        })
    }

    /// Lowest edge the box occupies in the flow: padding box, end border
    /// and padding, end margin.
    pub(super) fn flow_end(&self) -> f32 {
        self.y1 + self.bp + self.margin.max(0.0)
    }
}

/// Where the clamp container's content ends for a clamp point at `point`
/// inside the boxes `chain` (fragmented at it), in preorder (outermost
/// first): end border and padding of each box, `min-height` of each box,
/// end margins collapsing through boxes without border/padding (css-
/// overflow-4 §5.3; Blink `LineClampAncestorChain::
/// InnerFinalLineClampBlockSize`). The clamp container is an independent
/// BFC, so the margin left over at its end stays inside it.
pub(super) fn chain_end(point: f32, chain: &[&ClampBox]) -> f32 {
    chain_end_with(point, chain, true)
}

/// Space the end edges of `chain` take after the point, `min-height` aside
/// (the line budget of `line-clamp: auto`).
pub(super) fn chain_extra(chain: &[&ClampBox]) -> f32 {
    chain_end_with(0.0, chain, false)
}

/// Used border-box heights of the boxes of `chain` (see `chain_end`).
pub(super) fn chain_sizes(point: f32, chain: &[&ClampBox]) -> Vec<(u64, f32)> {
    (0..chain.len())
        .map(|i| {
            let b = chain[i];
            // The chain from the point up to `b`, margins after `b` aside.
            let inner = &chain[i + 1..];
            let mut y = chain_end_with(point, inner, true);
            // Margins pending below `b` collapse through it unless it has
            // end border/padding (then they are inside it).
            if b.bp > 0.0 {
                y += b.bp;
            } else {
                y -= pending_margin(inner);
            }
            let y = if b.min_end > 0.0 {
                y.max(b.y0 + b.min_end)
            } else {
                y
            };
            (b.node, (y - b.y0).max(0.0))
        })
        .filter(|(n, _)| *n != 0)
        .collect()
}

/// End margin still collapsing at the end of `chain` (no border/padding
/// below it).
fn pending_margin(chain: &[&ClampBox]) -> f32 {
    let (mut pos, mut neg) = (0.0f32, 0.0f32);
    for b in chain.iter().rev() {
        if b.bp > 0.0 {
            pos = 0.0;
            neg = 0.0;
        }
        pos = pos.max(b.margin);
        neg = neg.min(b.margin);
    }
    pos + neg
}

fn chain_end_with(point: f32, chain: &[&ClampBox], min: bool) -> f32 {
    let (mut y, mut pos, mut neg) = (point, 0.0f32, 0.0f32);
    for b in chain.iter().rev() {
        if b.bp > 0.0 {
            y += pos + neg + b.bp;
            pos = 0.0;
            neg = 0.0;
        }
        if min && b.min_end > 0.0 && b.y0 + b.min_end > y {
            y = b.y0 + b.min_end;
            pos = 0.0;
            neg = 0.0;
        }
        pos = pos.max(b.margin);
        neg = neg.min(b.margin);
    }
    y + pos + neg
}
