//! Entry edits retain the original cache key; pruning never visits descendants.
use super::*;

struct Entry {
    lists: Rc<Cell<usize>>,
    enters: Rc<Cell<usize>>,
}
impl Transform for Entry {
    type Context = i64;
    fn before_list(
        &self,
        nodes: &mut Children,
        context: &i64,
        admission: &Admission,
    ) -> Result<(), Error> {
        self.lists.set(self.lists.get() + 1);
        for n in nodes.edit(admission, 0)? {
            if let Node::Element(e) = n {
                e.list_item = Some(*context);
            }
        }
        Ok(())
    }
    fn visit_child(&self, nodes: &Children, index: usize, _: &i64) -> bool {
        !matches!(&nodes[index], Node::Element(e) if e.tag == "skip")
    }
    fn enter(&self, _: &mut Element, context: &i64, _: &Admission) -> Result<i64, Error> {
        self.enters.set(self.enters.get() + 1);
        Ok(*context)
    }
}

#[test]
fn entry_edits_hit_original_input_and_pruned_changes_survive_abort() {
    let raw: Children = vec![
        node("skip", vec![node("never", vec![])]),
        node("yes", vec![]),
    ]
    .into();
    let lists = Rc::new(Cell::new(0));
    let enters = Rc::new(Cell::new(0));
    let mut pass = Pass::new(Entry {
        lists: lists.clone(),
        enters: enters.clone(),
    });
    let old = publish(&mut pass, &raw, 1, &Admission::default());
    assert_eq!(lists.replace(0), 2);
    assert_eq!(enters.replace(0), 1);
    let hit = publish(&mut pass, &raw, 1, &Admission::default());
    assert_eq!(hit.identity(), old.identity());
    assert_eq!((lists.get(), enters.get()), (0, 0));
    assert_eq!(element(&raw[0]).list_item, None);
    assert_eq!(element(&element(&old[0]).children[0]).list_item, None);
    let mut dirty = raw.clone();
    element_mut(&mut dirty[0])
        .children
        .push(node("also-never", vec![]));
    let prepared = pass.prepare(&dirty, 1, &Admission::default()).unwrap();
    assert_eq!(element(&prepared.output()[0]).children.len(), 2);
    drop(prepared);
    assert_eq!(
        pass.current.as_ref().unwrap().output.identity(),
        old.identity()
    );
    let actual = publish(&mut pass, &dirty, 2, &Admission::default());
    let full: Children = apply(&pass.transform, dirty.into_vec(), 2, &Admission::default())
        .unwrap()
        .into();
    assert!(same(&actual, &full));
    assert_eq!(element(&old[0]).children.len(), 1);
}
