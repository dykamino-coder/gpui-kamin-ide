//! The shadow trees of a document as an edit's restyle sees them from the
//! light tree (`ShadowFacts`): which light elements host one, and the sheets
//! the shadow trees hold.
//!
//! A shadow tree is a scope of its own (css-scoping-1 §3.1): its rules match
//! only its own elements, its host (`:host`, `:host()`, `:host-context()`)
//! and the light children assigned to its slots (`::slotted()`), and the
//! document's rules never match inside it. An element outside every host's
//! subtree is therefore styled by the document's rules alone and inherits
//! along light ancestors only; a restyle of such an element needs no shadow
//! scope, unless a shadow rule reads a name the edit changes on the host's
//! ancestors (`:host-context()`). Stylo keeps a `CascadeData` per shadow root
//! and invalidates each scope by its own selectors
//! (`invalidation/element/invalidator.rs`, `stylist.rs`); this is the light
//! tree's half of that split. The facts are read from the source DOM when the
//! probe is built: a batch that adds or removes a `<template>` changes
//! children and rebuilds, and one that changes `shadowrootmode` is not a
//! `class`/`data-*` batch, so a scope appears or goes only with a new probe.

use super::declarative_shadow;
use crate::budget::admission::{Admission, Error};
use crate::dom::source_walk::{SourceWalk, Templates};
use markup5ever_rcdom::{Handle, NodeData};

/// The declarative shadow trees of a document's light tree.
#[derive(Debug, Default)]
pub struct ShadowFacts {
    /// The light elements that host a shadow tree, in tree order.
    pub hosts: Vec<Handle>,
    /// The `<style>` texts of the shadow trees, nested ones included.
    pub sheets: Vec<String>,
    /// A shadow tree holds a `<link>`: the text of its sheet is not known
    /// here, so it may read any name.
    pub linked: bool,
}

/// The shadow facts of the light tree under `root`.
pub(in crate::dom) fn shadow_facts(
    root: &Handle,
    admission: &Admission,
) -> Result<ShadowFacts, Error> {
    let mut facts = ShadowFacts::default();
    let mut walk = SourceWalk::new(root, Templates::None, admission.clone())?;
    while let Some(node) = walk.try_next()? {
        if let Some((_, shadow)) = declarative_shadow(&node) {
            facts.hosts.push(node.clone());
            read_sheets(&shadow, &mut facts, admission)?;
        }
    }
    Ok(facts)
}

/// The sheets of the shadow tree `root` and of the shadow trees inside it.
fn read_sheets(root: &Handle, facts: &mut ShadowFacts, admission: &Admission) -> Result<(), Error> {
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        admission.work(1)?;
        if let NodeData::Element {
            name,
            template_contents,
            ..
        } = &node.data
        {
            match &*name.local {
                "style" => facts.sheets.push(text_of(&node)),
                "link" => facts.linked = true,
                _ => {}
            }
            if let Some(contents) = &*template_contents.borrow() {
                stack.push(contents.clone());
            }
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    Ok(())
}

fn text_of(node: &Handle) -> String {
    let mut text = String::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            text.push_str(&contents.borrow());
        }
    }
    text
}
