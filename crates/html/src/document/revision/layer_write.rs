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
use super::patch::{Path, element_at};
use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use gpui::{Hsla, LayerHandle};

/// An owner's fill before and after.
type Write = (u32, Option<Hsla>, Option<Hsla>);

/// What the layer takes for the restyled boxes at `paths` (in tree order,
/// with their box numbers) from the tree `old` to the tree `new`, which share
/// their shape: `None` when a box's change is not own paint; otherwise the
/// values are written and the boxes whose own commands the next frame records
/// again are returned. Each box is reached along its path, without a walk of
/// either tree. A box under a block the frame skipped off the screen
/// (`unseen`) is in no run of the layer and shows no pixel of the frame: it
/// takes neither a write nor a record, and its block's record takes the
/// change for the frames that bring it near the screen (`Store::restyle`).
/// A box in the layer whose runs showed no pixel of its frame (`off`) takes
/// neither now: it is returned apart, its record deferred until a frame shows
/// it (`layout::reuse::deferred`).
pub(super) fn write_paint_values(
    old: &[Node],
    new: &[Node],
    paths: &[(Path, u64)],
    layer: &LayerHandle,
    plain: impl Fn(&Element, &Element) -> bool,
    unseen: impl Fn(&Path) -> bool,
    off: impl Fn(u64) -> bool,
) -> Option<Taken> {
    if paths.is_empty() {
        // No element's style changed: the frame stands as it is.
        return Some(Taken::default());
    }
    let held = |id: u64| u32::try_from(id).is_ok_and(|o| layer.holds_box(o));
    let mut writes: Vec<Write> = Vec::with_capacity(paths.len());
    let mut records: Vec<(Path, u64)> = Vec::new();
    let mut deferred: Vec<(Path, u64)> = Vec::new();
    for (path, id) in paths {
        let (a, b) = (at(old, path, *id)?, at(new, path, *id)?);
        // A box in an `svg` subtree: an `svg` element on its path.
        let in_svg = || {
            (1..=path.len()).any(|k| element_at(old, &path[..k]).is_some_and(|e| e.tag == "svg"))
        };
        let damage = classify(a, b, in_svg);
        if damage == PaintDamage::Rebuild {
            crate::paint_path::add(&crate::paint_path::REFUSED_NOT_PAINT, 1);
            return None;
        }
        if unseen(path) {
            crate::paint_path::add(&crate::paint_path::UNSEEN, 1);
            continue;
        }
        if !plain(a, b) {
            crate::paint_path::add(&crate::paint_path::REFUSED_NOT_OWN, 1);
            return None;
        }
        u32::try_from(a.node_id).ok()?;
        if off(a.node_id) && held(a.node_id) {
            deferred.push((path.clone(), a.node_id));
            continue;
        }
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
                // A value-only fill edit cannot change clip admission. Keep
                // the current layer; restyle drops the block's stale record.
                if damage == PaintDamage::Value(PaintValue::BACKGROUND_COLOR)
                    && from.a > 0.
                    && to.a > 0.
                    && layer.omits_box(owner)
                {
                    deferred.push((path.clone(), a.node_id));
                    continue;
                }
                // Fragments of one element share an owner: one write each.
                if writes.last().is_none_or(|w: &Write| w.0 != owner) {
                    writes.push((owner, Some(from), Some(to)));
                }
            }
            _ => records.push((path.clone(), a.node_id)),
        }
    }
    // A box the layer cannot record again builds the frame as before;
    // nothing is written then.
    if !records.iter().all(|r| held(r.1)) {
        crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
        return None;
    }
    let mut values = writes.len();
    if layer.set_backgrounds(&writes).is_none() {
        // A fill the layer does not hold as painted: its box records again.
        let owners = writes.iter().map(|w| u64::from(w.0));
        if !owners.clone().all(held) {
            crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
            return None;
        }
        let written = paths
            .iter()
            .filter(|(_, id)| owners.clone().any(|o| o == *id));
        records.extend(written.cloned());
        values = 0;
    }
    crate::paint_path::add(&crate::paint_path::RECORDS, records.len() as u64);
    crate::paint_path::add(&crate::paint_path::VALUES, values as u64);
    crate::paint_path::add(&crate::paint_path::DEFERRED, deferred.len() as u64);
    Some(Taken { records, deferred })
}

/// What the layer took of a restyle: the boxes the next frame records again
/// and the boxes off the screen whose records wait.
#[derive(Default)]
pub(super) struct Taken {
    pub(super) records: Vec<(Path, u64)>,
    pub(super) deferred: Vec<(Path, u64)>,
}

/// The box numbered `id` at `path` in `nodes`; `None` when the path leads
/// elsewhere (the trees do not share their shape).
pub(in crate::document) fn at<'a>(
    nodes: &'a [Node],
    path: &[usize],
    id: u64,
) -> Option<&'a Element> {
    super::patch::element_at(nodes, path).filter(|e| e.node_id == id)
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
