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
//! and the document builds its frame as before; a box with decorations whose
//! change leaves them as painted (`fill::kept_decorations`) takes a new fill
//! colour as a value all the same; a group root records its group (`grouped`).

use super::paint_damage::{PaintDamage, PaintValue, classify, unpainted_svg};
use super::patch::Path;
use crate::dom::{Element, Node};
use crate::layout::reuse::Wait;
use crate::paint::box_tree::styled::{Own, Refusal};
use gpui::{Hsla, LayerHandle, LayerPainter};

/// An owner's fill before and after.
type Write = (u32, Option<Hsla>, Option<Hsla>);
mod values;

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
/// on the next frame (`layer_band`). A box the layer cannot record whose
/// whole paint lies off the screen (`waits`) waits in `frames` (`offscreen`).
#[allow(clippy::too_many_arguments)]
pub(super) fn write_paint_values(
    old: &[Node],
    new: &[Node],
    paths: &[(Path, u64)],
    layer_for: impl Fn(u64) -> Option<LayerHandle>,
    own: impl Fn(&Element, bool) -> Own,
    grouped: impl Fn(&Element) -> bool,
    unseen: impl Fn(&Path) -> bool,
    off: impl Fn(u64) -> bool,
    waits: impl Fn(Wait, &Path, &Element, &Element) -> Option<super::offscreen::FrameWait>,
    frames: &mut Vec<super::offscreen::FrameWait>,
) -> Option<Taken> {
    if paths.is_empty() {
        // No element's style changed: the frame stands as it is.
        return Some(Taken::default());
    }
    let held = |id| layer_for(id).is_some_and(|layer| values::held(&layer, id));
    let mut writes = values::Values::default();
    let mut records: Vec<(Path, u64)> = Vec::new();
    let mut deferred: Vec<(Path, u64)> = Vec::new();
    for (path, id) in paths {
        let (a, b) = (at(old, path, *id)?, at(new, path, *id)?);
        // A box in an `svg` subtree: an `svg` element on its path.
        let in_svg = || {
            super::patch::chain(old, path)
                .iter()
                .any(|e| e.tag == "svg")
        };
        let damage = classify(a, b, in_svg);
        if damage == PaintDamage::Rebuild {
            crate::paint_path::add(&crate::paint_path::REFUSED_NOT_PAINT, 1);
            return None;
        }
        // No painted value of the box changed (a restyle that matched other
        // rules to the same paint): its painter, recorded or not, paints the
        // same commands, and nothing is written or recorded.
        let unchanged = damage == PaintDamage::Value(PaintValue::default());
        if unchanged || unpainted_svg(old, path) || super::paint_damage::unboxed(a, b) {
            continue;
        }
        if unseen(path) {
            crate::paint_path::add(&crate::paint_path::UNSEEN, 1);
            continue;
        }
        // The pieces of an inline box's band take value writes only, at once:
        // a write paints nothing, on the screen or off it.
        let owner = u32::try_from(a.node_id).ok()?;
        let layer = layer_for(a.node_id)?;
        let recorded = layer.painter(owner);
        // A box under an ancestor that skips its contents paints nothing: asked
        // only before a refusal, the walk up its path costs no other edit.
        let skip = || {
            let skipped = skipped::skipped(old, path);
            if skipped {
                crate::paint_path::add(&crate::paint_path::UNSEEN, 1);
            }
            skipped
        };
        let mut wait = |kind, reason| {
            if let Some(frame) = waits(kind, path, a, b) {
                frames.push(frame);
            } else if !skip() {
                refuse_not_own(reason);
                return None;
            }
            Some(())
        };
        // A box whose decorations stay as painted takes its own fill
        // (`kept_decorations`); its decorations are elements of the frame.
        let decorated = recorded == Some(LayerPainter::DecoratedBox);
        // A box that is all its group holds paints the group again with
        // its decorations (`retained::box_repaint`, `group_painters`).
        let group = || layer.holds_group_root(owner) && grouped(b);
        // A box recorded as a box of its own stays one: layout is kept.
        let boxed = recorded.is_some_and(|p| p != LayerPainter::Band);
        let banded = match (recorded, own(b, boxed)) {
            (Some(LayerPainter::Box), Own::Plain) => false,
            (Some(LayerPainter::DecoratedBox), Own::Not(Refusal::Decoration))
                if kept_decorations(a, b) =>
            {
                false
            }
            (
                Some(LayerPainter::Box | LayerPainter::DecoratedBox),
                Own::Not(Refusal::Decoration),
            )
            | (Some(LayerPainter::DecoratedBox), Own::Plain) => {
                if !group() {
                    wait(Wait::Decoration, Refusal::Decoration)?;
                    continue;
                }
                false
            }
            (Some(LayerPainter::Band) | None, Own::Band) => true,
            _ if skip() => continue,
            // Not own paint, or the recorded painter and the new paint have no
            // write between them.
            (_, wanted) => {
                let reason = if let Own::Not(r) = wanted {
                    r
                } else {
                    Refusal::Kind
                };
                refuse_not_own(reason);
                why::not_own(reason, owner, &b.tag, &layer, recorded, (damage, wanted));
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
                    } else if !skip() {
                        crate::paint_path::refuse_with(
                            &crate::paint_path::REFUSED_BY_LAYER,
                            || {
                                let why = layer.no_run(owner).unwrap_or("not painted");
                                format!(
                                    "band of <{}> {owner} has no run in the layer ({why})",
                                    b.tag
                                )
                            },
                        );
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
                writes.push(layer.clone(), (owner, Some(from), Some(to)));
            }
            // The layer records no decorated run again: a fill that appears
            // or goes builds the frame unless it waits off the screen.
            _ if decorated && !group() => {
                wait(Wait::Decoration, Refusal::Decoration)?;
                continue;
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
    if let Some((_, id)) = records.iter().find(|r| !held(r.1)) {
        crate::paint_path::refuse_with(&crate::paint_path::REFUSED_BY_LAYER, || {
            format!("box {id} has no run the layer can record again")
        });
        return None;
    }
    let values = writes.apply(paths, &mut records)?;
    crate::paint_path::add(&crate::paint_path::RECORDS, records.len() as u64);
    crate::paint_path::add(&crate::paint_path::VALUES, values as u64);
    crate::paint_path::add(&crate::paint_path::DEFERRED, deferred.len() as u64);
    super::offscreen::count(frames);
    Some(Taken { records, deferred })
}

mod fill;
mod skipped;
mod taken;
mod why;
use fill::kept_decorations;
pub(super) use fill::painted_background;
pub(super) use taken::Taken;
pub(in crate::document) use taken::{at, parent_at};
use why::refuse_not_own;
