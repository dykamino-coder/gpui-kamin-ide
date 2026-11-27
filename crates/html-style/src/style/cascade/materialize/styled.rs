//! The box stage of an element's box: remembered on its style (`stage_memo`) while its
//! inputs hold, else computed from the computed stage and the parent box (`inherit::box_stage`).

use super::{stage_memo, used};
use crate::dom::SharedStyle;
use crate::style::cascade::inherit::{BoxParents, ParentFacts, reidentify};
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use std::rc::Rc;

/// The box stage of `own` under `parent`: the remembered one while its inputs hold
/// (`stage_memo`), else computed and remembered.
pub(super) fn styled(
    parent: &BoxStyle,
    own: &SharedStyle,
    paint: bool,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    // Style share of a frame, hits included (`box_stage_ns` in the journal).
    crate::work::time!(_stage, BoxStageNs);
    // A box layout moved stays a child of its element-tree parent
    // (css-cascade-5 §7.2); `parent` is its new parent box.
    let moved = own.moved_from();
    let tree = moved.as_deref().unwrap_or(parent);
    // What the stage reads of the parents: the key, and the stage's only view of them.
    let facts = ParentFacts::of(
        BoxParents {
            layout: parent,
            tree,
        },
        vcx,
    );
    let own_key = own.stage_own();
    let materialized = own_key.materialized();
    let query = stage_memo::Query {
        paint,
        moved: moved.is_some(),
        parent: &facts,
        element_parent: tree.element_style.as_ref().filter(|_| materialized),
        anonymous: (!materialized).then(|| parent.style_rc()),
        own: own_key,
        vcx,
    };
    let memo = own.stage_memo();
    match memo.find(&query) {
        Ok(mut boxed) => {
            crate::work::add!(BoxStageHits, 1);
            reidentify(parent, &mut boxed);
            return boxed;
        }
        Err(miss) => miss.note(),
    }
    let arenas = vcx.doc.frame_arena_mark();
    let (mut boxed, read_viewport) = crate::values::viewport_reads::track(vcx.doc, || {
        stage(parent, own, tree, &facts, paint, vcx)
    });
    // Handles into the frame region of the arenas and a position-try source (a style
    // handle) are valid for this frame only or would hold the records alive.
    if vcx.doc.frame_arena_mark() == arenas && boxed.try_style.is_none() {
        memo.store(query, read_viewport, &boxed);
    } else {
        crate::work::add!(BoxStageUncached, 1);
    }
    reidentify(parent, &mut boxed);
    boxed
}

/// The stage itself. It reads the parents through `facts`, apart from the element stages
/// of the element parent (`tree.element_style`) and, for an anonymous box, the layout
/// parent's computed style — the other inputs of a remembered stage (`stage_memo::Query`).
fn stage(
    parent: &BoxStyle,
    own: &SharedStyle,
    tree: &BoxStyle,
    facts: &ParentFacts<'_>,
    paint: bool,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    let cascaded = own.cascaded();
    let element = own.slot().with_parent().map(|(original, element_parent)| {
        // Only a new computed stage of the element parent (a position-try
        // source, for example) recomputes the child. A used copy does not change it.
        let context = tree.element_style.as_ref();
        let computed = context
            .and_then(|p| p.variant_for(&element_parent))
            .map(|p| own.slot().compute(p, cascaded, vcx).legacy)
            .unwrap_or_else(|| original.clone());
        crate::style::computed::ElementStyle {
            original,
            computed,
            parent: context.and_then(|p| p.active()),
        }
    });
    let computed = match &element {
        Some(e) => match own.used_rc() {
            Some(used) if Rc::ptr_eq(&e.original, &e.computed) => used,
            Some(used) => {
                let mut c = (*e.computed).clone();
                used::lay_used(&mut c, &used, &e.original);
                Rc::new(c)
            }
            None => e.computed.clone(),
        },
        // An anonymous box inherits from its parent box.
        None => crate::style::cascade::compute::compute(parent, cascaded, vcx).legacy,
    };
    let view = own.used();
    // Whether colors are set comes from the cascade; a style without materialization has one stage.
    let declared = match own.slot().any() {
        Some(_) => cascaded,
        None => &*view,
    };
    let mut boxed = crate::style::cascade::inherit::box_stage_from(
        facts,
        declared,
        &view,
        crate::style::box_input::BoxStyleInput {
            style: computed,
            ..view.clone()
        },
        element,
        paint,
        vcx,
    );
    if !boxed.reset_positioned.position_try_fallbacks.is_empty() {
        // Variants of a box layout made from an element stage recompute that
        // element's cascade under its element parent (css-anchor-position-1
        // §fallback-rule, css-cascade-5 §7.3.1), not the box values under the parent box.
        boxed.try_style = Some(view.try_source.clone().unwrap_or_else(|| own.clone()));
    }
    boxed
}
