//! Raw cascade snapshots stay separate from formatting and staged publication.
use super::*;
use crate::dom::{Node, input::Admitted, restyle_probe::RestyleCache};
use crate::style::css::Media;
use crate::values::DocumentValues;
use markup5ever_rcdom::{Handle, NodeData};

mod contexts;
mod ownership;
mod retirement;
mod stages;
mod structure;

fn build(source: &str) -> (Admitted, Rc<DocumentValues>, RestyleCache, Vec<Node>) {
    let input = Admitted::parse(source, false).unwrap();
    let (values, cache, nodes) = recascade(&input);
    (input, values, cache, nodes)
}

fn recascade(input: &Admitted) -> (Rc<DocumentValues>, RestyleCache, Vec<Node>) {
    let values = DocumentValues::detached(Media::default());
    values.admit_revision(input.context().budget().clone(), None);
    let mut cache = RestyleCache::default();
    let nodes =
        crate::dom::convert_restyling(input, "", Media::default(), &values, None, Some(&mut cache))
            .unwrap();
    (values, cache, nodes)
}

fn handle(input: &Admitted, id: &str) -> Handle {
    let mut work = vec![input.dom.document.clone()];
    while let Some(node) = work.pop() {
        if let NodeData::Element {
            attrs,
            template_contents,
            ..
        } = &node.data
        {
            if attrs
                .borrow()
                .iter()
                .any(|a| &*a.name.local == "id" && &*a.value == id)
            {
                return node;
            }
            work.extend(template_contents.borrow().iter().cloned());
        }
        work.extend(node.children.borrow().iter().cloned());
    }
    panic!("missing source element {id}")
}

#[test]
fn normalization_detaches_from_raw_cascade() {
    let (input, _values, cache, nodes) =
        build("<!doctype html><div id=p><span id=c style='display:inherit'>text</span></div>");
    let entry = cache.raw().get(handle(&input, "c").key()).unwrap();
    let raw =
        crate::dom::test_support::find(&entry.raw, &|e| e.node_id == entry.node.packed()).unwrap();
    let normalized =
        crate::dom::test_support::find(&nodes, &|e| e.node_id == entry.node.packed()).unwrap();
    let display = crate::style::computed::inh::DISPLAY;
    assert_ne!(raw.style.cascaded().reset_cascade.inherit_bits & display, 0);
    assert_eq!(
        normalized.style.cascaded().reset_cascade.inherit_bits & display,
        0
    );
    assert_ne!(cache.raw().roots().identity(), 0);
}

#[test]
fn sibling_inputs_share_parent_but_keep_distinct_counter_states() {
    let (input, _values, cache, _) = build("<ol id=p><li id=a>one</li><li id=b>two</li></ol>");
    let a = cache.raw().get(handle(&input, "a").key()).unwrap();
    let b = cache.raw().get(handle(&input, "b").key()).unwrap();
    assert!(Rc::ptr_eq(
        a.inputs.parent.as_ref().unwrap(),
        b.inputs.parent.as_ref().unwrap()
    ));
    assert!(a.inputs.counters != b.inputs.counters);
    assert!(a.exit == b.inputs.counters);
    assert_eq!(
        a.inputs.parent.as_ref().unwrap().node,
        handle(&input, "p").key()
    );
}

#[test]
fn no_output_is_a_cached_result_and_not_a_missing_entry() {
    let (input, _values, cache, _) = build("<body><script id=s>ignored</script><p id=p>text</p>");
    let entry = cache.raw().get(handle(&input, "s").key()).unwrap();
    assert!(entry.raw.is_empty());
    assert!(cache.raw().get(handle(&input, "p").key()).is_some());
}

#[test]
fn scroll_group_selection_is_a_replayable_source_effect() {
    let (input, _values, cache, _) = build(
        "<nav id=g style='scroll-target-group:auto'><a id=a href='#x'>one</a>\
         <a id=b href='#y'>two</a></nav><p id=x>x</p><p id=y>y</p>",
    );
    let group = handle(&input, "g").key();
    let a = cache.raw().get(handle(&input, "a").key()).unwrap();
    let b = cache.raw().get(handle(&input, "b").key()).unwrap();
    assert_eq!(a.selects_group, Some(group));
    assert_eq!(
        a.inputs.group,
        Some(GroupInput {
            node: group,
            selected: false
        })
    );
    assert_eq!(
        b.inputs.group,
        Some(GroupInput {
            node: group,
            selected: true
        })
    );
    assert_eq!(b.selects_group, None);
}

#[test]
fn one_source_entry_keeps_all_generated_sibling_boxes() {
    let (input, _values, cache, _) =
        build("<style>#s::scroll-button(right){content:'next'}</style><div id=s>text</div>");
    let entry = cache.raw().get(handle(&input, "s").key()).unwrap();
    assert_eq!(entry.raw.len(), 2);
    assert!(
        entry
            .raw
            .iter()
            .any(|n| matches!(n, Node::Element(e) if e.node_id == entry.node.packed()))
    );
}

#[test]
fn rejected_conversion_keeps_the_published_raw_cache() {
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits};
    let (input, _values, mut cache, _) = build("<p id=p>text</p>");
    let p = handle(&input, "p").key();
    let held = cache.raw().get(p).unwrap().clone();
    let roots = cache.raw().roots().identity();
    let budget = DocumentBudget::new(
        DocumentLimits::default()
            .lower(BudgetKind::LiveBytes, 0)
            .unwrap(),
        Default::default(),
    )
    .unwrap();
    let values = DocumentValues::detached(Media::default());
    values.admit_revision(budget.revision(), None);
    assert!(
        crate::dom::convert_restyling(
            &input,
            "",
            Media::default(),
            &values,
            None,
            Some(&mut cache),
        )
        .is_err()
    );
    assert!(Rc::ptr_eq(cache.raw().get(p).unwrap(), &held));
    assert_eq!(cache.raw().roots().identity(), roots);
}
