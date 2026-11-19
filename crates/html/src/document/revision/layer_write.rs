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
use crate::layout::reuse::Wait;
use crate::paint::box_tree::styled::{Own, Refusal};
use crate::style::computed::BoxStyle;
use gpui::{Hsla, LayerHandle, LayerPainter};

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
/// it (`layout::reuse::deferred`). An inline box whose own paint is its band
/// (`Own::Band`) takes a new background colour as a value written into every
/// piece of its band; any other change of its band paints its pieces again
/// on the next frame (`layer_band`).
#[allow(clippy::too_many_arguments)]
pub(super) fn write_paint_values(
    old: &[Node],
    new: &[Node],
    paths: &[(Path, u64)],
    layer: &LayerHandle,
    own: impl Fn(&Element) -> Own,
    unseen: impl Fn(&Path) -> bool,
    off: impl Fn(u64) -> bool,
    waits: impl Fn(Wait, &Path, &Element, &Element) -> Option<super::offscreen::FrameWait>,
    frames: &mut Vec<super::offscreen::FrameWait>,
) -> Option<Taken> {
    if paths.is_empty() {
        // No element's style changed: the frame stands as it is.
        return Some(Taken::default());
    }
    // A replacement must cover every recorded run of the owner.
    let held = |id: u64| u32::try_from(id).is_ok_and(|o| layer.holds_box(o) || layer.holds_band(o));
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
        // The pieces of an inline box's band take value writes only, at once:
        // a write paints nothing, on the screen or off it.
        let owner = u32::try_from(a.node_id).ok()?;
        let recorded = layer.painter(owner);
        let mut wait = |kind, reason| {
            if let Some(frame) = waits(kind, path, a, b) {
                frames.push(frame);
                Some(())
            } else {
                refuse_not_own(reason);
                None
            }
        };
        let banded = match (recorded, own(b)) {
            (Some(LayerPainter::Box), Own::Plain) => false,
            (
                Some(LayerPainter::Box | LayerPainter::DecoratedBox),
                Own::Not(Refusal::Decoration),
            )
            | (Some(LayerPainter::DecoratedBox), Own::Plain) => {
                wait(Wait::Decoration, Refusal::Decoration)?;
                continue;
            }
            (Some(LayerPainter::Band) | None, Own::Band) => true,
            (_, Own::Not(reason)) => {
                refuse_not_own(reason);
                return None;
            }
            _ => {
                refuse_not_own(Refusal::Kind);
                return None;
            }
        };
        if banded && recorded.is_none() {
            // Culled and nested bands can have line fragments but no scene run.
            match super::layer_band::band_change(old, path, a, b) {
                super::layer_band::BandChange::Nothing => (),
                super::layer_band::BandChange::Refused(reason) => wait(Wait::Inline, reason)?,
                super::layer_band::BandChange::Pieces => {
                    if let Some(frame) = waits(Wait::Inline, path, a, b) {
                        frames.push(frame);
                    } else {
                        crate::paint_path::add(&crate::paint_path::REFUSED_BY_LAYER, 1);
                        return None;
                    }
                }
            }
            continue;
        }
        if !banded && off(a.node_id) && held(a.node_id) {
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
                // Fragments of one element share an owner: one write each.
                if writes.last().is_none_or(|w: &Write| w.0 != owner) {
                    writes.push((owner, Some(from), Some(to)));
                }
            }
            // Any other change of a band: its pieces are painted again.
            _ if banded => match super::layer_band::band_change(old, path, a, b) {
                super::layer_band::BandChange::Nothing => continue,
                super::layer_band::BandChange::Pieces if held(a.node_id) => {
                    records.push((path.clone(), a.node_id))
                }
                super::layer_band::BandChange::Pieces => wait(Wait::Inline, Refusal::BandGeometry)?,
                super::layer_band::BandChange::Refused(reason) => {
                    wait(Wait::Inline, reason)?;
                }
            },
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
    super::offscreen::count(frames);
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

/// The element holding the box at `path` in `nodes`.
pub(in crate::document) fn parent_at<'a>(nodes: &'a [Node], path: &[usize]) -> Option<&'a Element> {
    path.len()
        .checked_sub(1)
        .and_then(|n| element_at(nodes, &path[..n]))
}

/// The plain colour the box's background paints (`Some(None)`: no fill);
/// `None` for a gradient, an image stack, a flattened stack or a colour
/// clipped apart from the box (`background-clip`), which no single fill of
/// the box's own painter (`Style::paint_box`) carries.
pub(super) fn painted_background(e: &Element) -> Option<Option<Hsla>> {
    let style = BoxStyle::shared(e.style.used());
    let values = style.paint_background();
    let plain = values.gradient.is_none()
        && values.gradient_image.is_none()
        && values.layers.is_none()
        && values.flat.is_none()
        && values.color_clip.is_none();
    plain.then(|| values.color.map(|c| c.to_hsla()))
}

/// Count a refusal of a box whose own paint the layer does not hold.
fn refuse_not_own(reason: Refusal) {
    crate::paint_path::add(&crate::paint_path::REFUSED_NOT_OWN, 1);
    crate::paint_path::not_own::add(reason.label());
}
