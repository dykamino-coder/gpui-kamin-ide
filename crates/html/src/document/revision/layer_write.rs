//! The paint-only path of a restyle: the document's scene layer
//! (`gpui::LayerHandle`) takes the new paint instead of a new frame.
//!
//! A restyle whose every element changed only its own paint (`paint_damage`)
//! leaves layout and the element tree as they are, and the recorded frame
//! stays. A value the layer binds (the colour of a painted background) is
//! written now; any other own-paint change, and a write the layer refuses
//! (no fill was painted), records the box's own commands again on the next
//! frame (`Store::queue_repaints`). Blink invalidates the display item
//! client (`display_item_client.h:60`) and repaints its items into a new
//! artifact, moving every cached item of the chunk
//! (`paint_controller.cc:228`, `:267`); here the layer's other commands stay
//! where they are. A box whose paint is not its own commands only (hover
//! layers, decorations painted as child elements, the root's canvas) refuses,
//! and the document builds its frame as before.

use super::paint_damage::{PaintDamage, PaintValue, classify};
use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use gpui::{Hsla, LayerHandle};

/// An owner's fill before and after.
type Write = (u32, Option<Hsla>, Option<Hsla>);

/// What the layer takes for the restyled elements `ids` (box numbers) from
/// the tree `old` to the tree `new`: `None` when an element's change is not
/// own paint; otherwise the values are written and the boxes whose own
/// commands the next frame records again are returned.
pub(super) fn write_paint_values(
    old: &[Node],
    new: &[Node],
    ids: &[u64],
    layer: &LayerHandle,
    plain: impl Fn(&Element, &Element) -> bool,
) -> Option<Vec<u64>> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        // No element's style changed: the frame stands as it is.
        return Some(Vec::new());
    }
    let (before, after) = (boxes(old, &ids)?, boxes(new, &ids)?);
    if before.len() != after.len() {
        return None;
    }
    let mut writes: Vec<Write> = Vec::with_capacity(ids.len());
    let mut records = Vec::new();
    for (a, b) in before.iter().zip(&after) {
        let damage = classify(a, b);
        if damage == PaintDamage::Rebuild {
            crate::paint_path::add(&crate::paint_path::REFUSED_NOT_PAINT, 1);
            return None;
        }
        if !plain(a, b) {
            crate::paint_path::add(&crate::paint_path::REFUSED_NOT_OWN, 1);
            return None;
        }
        u32::try_from(a.node_id).ok()?;
        let fill = match damage {
            PaintDamage::Value(bits) if bits.contains(PaintValue::BACKGROUND_COLOR) => {
                painted_background(a).zip(painted_background(b))
            }
            PaintDamage::Value(_) => continue,
            _ => None,
        };
        match fill {
            Some((Some(from), Some(to))) => {
                let owner = a.node_id as u32;
                // Fragments of one element share an owner: one write each.
                if writes.last().is_none_or(|w: &Write| w.0 != owner) {
                    writes.push((owner, Some(from), Some(to)));
                }
            }
            _ => records.push(a.node_id),
        }
    }
    // A box the layer cannot record again builds the frame as before;
    // nothing is written then.
    let held = |ids: &[u64]| {
        ids.iter()
            .all(|&id| u32::try_from(id).is_ok_and(|o| layer.holds_box(o)))
    };
    if !held(&records) {
        crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
        return None;
    }
    if layer.set_backgrounds(&writes).is_none() {
        // A fill the layer does not hold as painted: its box records again.
        let owners: Vec<u64> = writes.iter().map(|w| u64::from(w.0)).collect();
        if !held(&owners) {
            crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
            return None;
        }
        records.extend(owners);
    }
    records.sort_unstable();
    records.dedup();
    crate::paint_path::add(&crate::paint_path::RECORDS, records.len() as u64);
    crate::paint_path::add(
        &crate::paint_path::VALUES,
        (writes.len()
            - writes
                .iter()
                .filter(|w| records.binary_search(&u64::from(w.0)).is_ok())
                .count()) as u64,
    );
    Some(records)
}

/// The boxes of the elements `ids` (sorted), in tree order; `None` unless
/// every element has one in `nodes`.
pub(in crate::document) fn boxes<'a>(nodes: &'a [Node], ids: &[u64]) -> Option<Vec<&'a Element>> {
    let (low, high) = (*ids.first()?, *ids.last()?);
    let mut found = Vec::new();
    // Pre-order without recursion: the tree's depth is bounded only by the
    // representation's limit.
    let mut stack: Vec<std::slice::Iter<'a, Node>> = vec![nodes.iter()];
    while let Some(list) = stack.last_mut() {
        let Some(node) = list.next() else {
            stack.pop();
            continue;
        };
        let Node::Element(e) = node else { continue };
        if (low..=high).contains(&e.node_id) && ids.binary_search(&e.node_id).is_ok() {
            found.push(e);
        }
        stack.push(e.children.iter());
    }
    let every = ids.iter().all(|id| found.iter().any(|e| e.node_id == *id));
    every.then_some(found)
}

/// The plain colour the box's background paints (`Some(None)`: no fill);
/// `None` for a gradient, an image stack, a flattened stack or a colour
/// clipped apart from the box (`background-clip`), which no single fill of
/// the box's own painter (`Style::paint_box`) carries.
fn painted_background(e: &Element) -> Option<Option<Hsla>> {
    let style = BoxStyle::shared(e.style.used());
    let values = style.paint_background();
    let plain = values.gradient.is_none()
        && values.gradient_image.is_none()
        && values.layers.is_none()
        && values.flat.is_none()
        && values.color_clip.is_none();
    plain.then(|| values.color.map(|c| c.to_hsla()))
}
