//! Коробки перерассчитанных элементов в дереве и условия, при которых они и
//! их поддеревья принимают новые значения краски на месте.

use super::Restyle;
use crate::document::revision::patch::{Path, elements};
use crate::dom::{Element, Node};
use std::collections::HashMap;

/// The tree keeps its `body` box (under `html` and the viewport box).
pub(super) fn wrapped(nodes: &[Node]) -> bool {
    let mut level: Vec<&Node> = nodes.iter().collect();
    for _ in 0..3 {
        let mut next = Vec::new();
        for node in level {
            if let Node::Element(e) = node {
                if e.tag == "body" && e.node_id != 0 {
                    return true;
                }
                next.extend(e.children.iter());
            }
        }
        level = next;
    }
    false
}

/// Пути к коробкам перерассчитанных элементов (с их тегом и прежними
/// значениями краски), если каждая принимает новые значения на месте
/// (`takes`); у каждого элемента — хотя бы одна коробка.
pub(super) fn style_paths(
    nodes: &[Node],
    restyles: &HashMap<u64, Restyle>,
) -> Option<Vec<(Path, u64)>> {
    let mut paths = Vec::new();
    let mut fits = true;
    if !restyles.is_empty() {
        elements(nodes, |e, path| {
            match restyles.get(&e.node_id) {
                Some(r) if e.tag == r.tag => {
                    fits &= e.style.cascaded().same_paint_groups(&r.old) && takes(e, r);
                    paths.push((path.to_vec(), e.node_id));
                }
                _ => {}
            }
            fits
        });
    }
    let found = |id: &u64| paths.iter().any(|(_, at)| at == id);
    (fits && restyles.keys().all(found)).then_some(paths)
}

/// The box `e` and its subtree take the restyle `r` of its element in place.
/// The box itself keeps no animation frames or copies of its computed values
/// outside the cascaded stage (for a new background, no decoration inputs
/// either). Its subtree follows by rematerialization: under new inherited
/// values no box keeps a hover or first-letter/-line layer, animation frames
/// or such copies; under a new background alone only a style that inherits
/// reset values explicitly reads it (css-cascade-5 §7.3.1), and no such style
/// is a layer or keeps copies.
fn takes(e: &Element, r: &Restyle) -> bool {
    let (inherited, background) = (r.inherited(), r.background());
    if e.anim.is_some() || e.style.holds_inherited_copies() {
        return false;
    }
    if background && e.style.holds_decoration() {
        return false;
    }
    elements(std::slice::from_ref(&Node::Element(e.clone())), |b, _| {
        let copies = b.anim.is_some() || b.style.holds_inherited_copies();
        if inherited {
            !copies && layers(b).all(|l| l.is_none())
        } else {
            !(copies && b.style.inherits_reset())
                && layers(b).flatten().all(|l| !l.inherits_reset())
        }
    })
}

/// The hover and first-letter/-line layers of a box.
fn layers(b: &Element) -> impl Iterator<Item = &Option<crate::dom::SharedStyle>> {
    [&b.hover, &b.first_letter, &b.first_line].into_iter()
}
