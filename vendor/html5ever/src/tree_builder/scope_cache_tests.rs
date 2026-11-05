//! Named scope proofs must survive only stack edits that preserve them.

use super::*;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Default)]
struct Count(Rc<Cell<usize>>);
impl Admission<Sink> for Count {
    type Error = Infallible;
    fn check(&self, op: Operation<'_, usize>) -> Result<(), Infallible> {
        if let Operation::Work(n) = op {
            self.0.set(self.0.get() + n);
        }
        Ok(())
    }
}

#[derive(Default)]
struct Sink {
    names: RefCell<Vec<QualName>>,
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = ();
    type ElemName<'a> = Name<'a>;
    fn finish(self) {}
    fn get_document(&self) -> usize {
        usize::MAX
    }
    fn same_node(&self, a: &usize, b: &usize) -> bool {
        a == b
    }
    fn elem_name(&self, target: &usize) -> Name<'_> {
        Name(self.names.borrow(), *target)
    }
    fn get_template_contents(&self, target: &usize) -> usize {
        *target
    }
    fn create_element(&self, name: QualName, _: Vec<Attribute>, _: ElementFlags) -> usize {
        let mut names = self.names.borrow_mut();
        let id = names.len();
        names.push(name);
        id
    }
    fn create_comment(&self, _: StrTendril) -> usize {
        usize::MAX - 1
    }
    fn create_pi(&self, _: StrTendril, _: StrTendril) -> usize {
        usize::MAX - 1
    }
    fn append(&self, _: &usize, _: NodeOrText<usize>) {}
    fn append_before_sibling(&self, _: &usize, _: NodeOrText<usize>) {}
    fn append_based_on_parent_node(&self, _: &usize, _: &usize, _: NodeOrText<usize>) {}
    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&self, _: &usize, _: Vec<Attribute>) {}
    fn remove_from_parent(&self, _: &usize) {}
    fn reparent_children(&self, _: &usize, _: &usize) {}
    fn set_quirks_mode(&self, _: QuirksMode) {}
    fn parse_error(&self, _: Cow<'static, str>) {}
}

#[derive(Debug)]
struct Name<'a>(std::cell::Ref<'a, Vec<QualName>>, usize);
impl crate::interface::ElemName for Name<'_> {
    fn ns(&self) -> &crate::Namespace {
        &self.0[self.1].ns
    }
    fn local_name(&self) -> &crate::LocalName {
        &self.0[self.1].local
    }
    fn expanded(&self) -> ExpandedName<'_> {
        self.0[self.1].expanded()
    }
}

type Builder = TreeBuilder<usize, Sink, Count>;
fn builder() -> Builder {
    let b = Builder::with_admission(Sink::default(), Default::default(), Count::default());
    push(&b, ns!(html), local_name!("html"));
    b
}
fn push(b: &Builder, ns: Namespace, name: LocalName) -> usize {
    let h = b.sink.create_element(
        QualName::new(None, ns, name),
        Vec::new(),
        Default::default(),
    );
    b.push(&h).unwrap();
    h
}

#[test]
fn growing_negative_scope_queries_are_linear() {
    let b = builder();
    for _ in 0..20_000 {
        assert!(!b.in_scope_named(button_scope, local_name!("p")).unwrap());
        push(&b, ns!(html), local_name!("div"));
    }
    assert!(b.admission.0.get() < 40_010, "{}", b.admission.0.get());
}

#[test]
fn proof_boundary_must_match_current_scope() {
    let b = builder();
    push(&b, ns!(html), local_name!("p"));
    push(&b, ns!(html), local_name!("button"));
    assert!(!b.in_scope_named(button_scope, local_name!("p")).unwrap());
    assert!(b.in_scope_named(default_scope, local_name!("p")).unwrap());
    b.pop().unwrap();
    assert!(b.in_scope_named(button_scope, local_name!("p")).unwrap());
}

#[test]
fn matching_push_and_arbitrary_edits_invalidate() {
    let b = builder();
    assert!(!b.in_scope_named(default_scope, local_name!("p")).unwrap());
    let p = push(&b, ns!(html), local_name!("p"));
    assert!(b.in_scope_named(default_scope, local_name!("p")).unwrap());
    b.pop().unwrap();
    assert!(!b.in_scope_named(default_scope, local_name!("p")).unwrap());
    b.insert_into(&b.open_elems, 1, p).unwrap();
    assert!(b.in_scope_named(default_scope, local_name!("p")).unwrap());
    b.remove_from(&b.open_elems, 1).unwrap();
    assert!(!b.in_scope_named(default_scope, local_name!("p")).unwrap());
}

#[test]
fn namespace_and_target_precedence_are_preserved() {
    let b = builder();
    assert!(!b.in_scope_named(default_scope, local_name!("p")).unwrap());
    push(&b, ns!(svg), local_name!("p"));
    assert!(!b.in_scope_named(default_scope, local_name!("p")).unwrap());
    push(&b, ns!(html), local_name!("table"));
    assert!(b.in_scope_named(table_scope, local_name!("table")).unwrap());
}

#[test]
fn parser_work_is_linear_for_nested_block_tokens() {
    let mut previous = 0;
    for depth in [1_000, 5_000, 20_000] {
        let count = Count::default();
        let calls = count.0.clone();
        let mut parser =
            crate::admission::parse_document(Sink::default(), count, Default::default());
        let source = format!("{}leaf{}", "<div>".repeat(depth), "</div>".repeat(depth));
        parser.process_str(&source).unwrap();
        parser.finish().unwrap();
        let total = calls.get();
        assert!(total < depth * 200, "depth={depth} work={total}");
        if previous != 0 {
            assert!(total < previous * 6);
        }
        previous = total;
    }
}

#[test]
fn cached_queries_match_the_canonical_scan_after_stack_edits() {
    let b = builder();
    let names = [
        local_name!("p"),
        local_name!("div"),
        local_name!("table"),
        local_name!("button"),
    ];
    let scopes: [fn(ExpandedName<'_>) -> bool; 3] = [default_scope, button_scope, table_scope];
    for i in 0..300 {
        let name = names[i % names.len()].clone();
        if i % 5 == 0 && b.open_elems.borrow().len() > 1 {
            b.pop().unwrap();
        } else {
            let h = push(&b, if i % 7 == 0 { ns!(svg) } else { ns!(html) }, name);
            if i % 3 == 0 {
                b.pop().unwrap();
                b.insert_into(&b.open_elems, 1, h).unwrap();
            }
        }
        for scope in scopes {
            for name in &names {
                let canonical = b
                    .in_scope(scope, |h| b.html_elem_named(&h, name.clone()))
                    .unwrap();
                assert_eq!(b.in_scope_named(scope, name.clone()).unwrap(), canonical);
            }
        }
    }
}
