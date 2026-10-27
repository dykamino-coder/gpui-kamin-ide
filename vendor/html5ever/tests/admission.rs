//! Terminal failure is distinct from HTML recovery, including during EOF repair.

use html5ever::admission::{self, Admission, Operation};
use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode};
use html5ever::tendril::StrTendril;
use html5ever::{tree_builder::TreeSink, Attribute, ExpandedName, QualName};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rejected(usize);

#[derive(Clone)]
struct Policy {
    reject: usize,
    calls: Rc<Cell<usize>>,
}

impl Admission<Sink> for Policy {
    type Error = Rejected;
    fn check(&self, _: Operation<'_, usize>) -> Result<(), Rejected> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if call == self.reject {
            Err(Rejected(call))
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct Sink {
    names: RefCell<Vec<QualName>>,
    finished: Rc<Cell<bool>>,
    dropped: Rc<Cell<bool>>,
}

impl Drop for Sink {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

impl TreeSink for Sink {
    type Handle = usize;
    type Output = ();
    type ElemName<'a> = Name<'a>;
    fn finish(self) {
        self.finished.set(true);
    }
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
impl html5ever::interface::ElemName for Name<'_> {
    fn ns(&self) -> &html5ever::Namespace {
        &self.0[self.1].ns
    }
    fn local_name(&self) -> &html5ever::LocalName {
        &self.0[self.1].local
    }
    fn expanded(&self) -> ExpandedName<'_> {
        self.0[self.1].expanded()
    }
}

type ParserFixture = (
    admission::BoundedParser<Sink, Policy>,
    Rc<Cell<usize>>,
    Rc<Cell<bool>>,
    Rc<Cell<bool>>,
);

fn parser(reject: usize) -> ParserFixture {
    let policy = Policy {
        reject,
        calls: Default::default(),
    };
    let calls = policy.calls.clone();
    let sink = Sink::default();
    let finished = sink.finished.clone();
    let dropped = sink.dropped.clone();
    (
        admission::parse_document(sink, policy, Default::default()),
        calls,
        finished,
        dropped,
    )
}

#[test]
fn every_admission_failure_is_sticky_and_drops_partial_sink() {
    for source in [
        "<!doctype html><p>KEEP</p><div>LOST</div>",
        "<table>KEEP<p><b><i>x</b>LOST</table>",
        "<template><slot>KEEP</slot><p>LOST",
        "<!--KEEP--><?target LOST?><div a='&amp;' a='duplicate'>",
        "<b><i>KEEP</b>LOST",
        "<p a='&bogus;'>&#xnotanumber;KEEP&amp LOST</p>",
        "<!DOCTYPE html PUBLIC 'KEEP' 'LOST'><html><head></head><body>x",
        "<!doctype html><table>KEEP<tr><td>LOST</table>",
        "<svg><foreignObject><b>KEEP</b></foreignObject><text>LOST</text></svg>",
        "<math><mi>KEEP</mi><annotation-xml encoding='text/html'>LOST</annotation-xml></math>",
        "<script>if (a < b) { KEEP }<!--LOST</script><p>x",
        "<style>p::before { content: 'KEEP<LOST'; }</style>",
        "<textarea>KEEP&amp;LOST</textarea><plaintext>tail",
        "<div><template shadowrootmode='open'><slot>KEEP</slot>LOST</template></div>",
        "<select><option>KEEP<option>LOST</select>",
        "<a><b><a>KEEP</b>LOST</a>",
        "<p>КЕЕP&amp;漢字\0LOST",
        "<!--KEEP--!><!bogus LOST>",
        "<?target KEEP LOST?>",
        "<p a='KEEP' unfinished='LOST",
        "<p a='&#12345678901234567890;LOST'>KEEP",
        "&NotEqualTilde;KEEP&bogus;LOST",
        "<!DOCTYPE html PUBLIC 'KEEP'",
        "<!--KEEP LOST",
        "<?target KEEP LOST",
        "<table><tbody><tr><td><b>KEEP</table>LOST",
        "",
    ] {
        let (mut full, calls, finished, dropped) = parser(usize::MAX);
        assert!(full.process_str(source).is_ok());
        assert!(full.finish().is_ok());
        assert!(finished.get());
        assert!(dropped.get());
        let total = calls.get();
        for reject in 1..=total {
            let (mut partial, calls, finished, dropped) = parser(reject);
            let processing = partial.process_str(source);
            if processing.is_err() {
                assert_eq!(processing, Err(Rejected(reject)));
                let next = partial.input_buffer.peek();
                assert_eq!(
                    partial.process_str("<div>must not enter input</div>"),
                    Err(Rejected(reject))
                );
                assert_eq!(partial.input_buffer.peek(), next);
                assert_eq!(calls.get(), reject);
            }
            assert_eq!(partial.finish(), Err(Rejected(reject)));
            assert_eq!(calls.get(), reject);
            assert!(!finished.get());
            assert!(dropped.get());
        }
    }
}

#[test]
fn rejection_in_data_state_leaves_following_input_unconsumed() {
    struct RejectText;
    impl Admission<Sink> for RejectText {
        type Error = Rejected;
        fn check(&self, operation: Operation<'_, usize>) -> Result<(), Rejected> {
            if matches!(operation, Operation::Append(_, NodeOrText::AppendText(_))) {
                Err(Rejected(0))
            } else {
                Ok(())
            }
        }
    }
    let mut parser = admission::parse_document(Sink::default(), RejectText, Default::default());
    assert_eq!(
        parser.process_str("<p>KEEP<div>LOST</div>"),
        Err(Rejected(0))
    );
    assert_eq!(parser.input_buffer.peek(), Some('<'));
    assert_eq!(parser.finish(), Err(Rejected(0)));
}

#[test]
fn standalone_tokenizer_latches_abort_without_a_sink_side_flag() {
    use html5ever::tokenizer::{Token, TokenSink, TokenSinkResult, Tokenizer};
    #[derive(Default)]
    struct Stop {
        calls: Cell<usize>,
        ended: Cell<bool>,
    }
    impl TokenSink for Stop {
        type Handle = ();
        fn process_token(&self, _: Token, _: u64) -> TokenSinkResult<()> {
            self.calls.set(self.calls.get() + 1);
            TokenSinkResult::Abort
        }
        fn end(&self) {
            self.ended.set(true);
        }
    }
    for source in ["KEEP<div>LOST</div>", "<p>KEEP</p>LOST", "&#x;KEEP<LOST"] {
        let tokenizer = Tokenizer::new(Stop::default(), Default::default());
        let input = html5ever::buffer_queue::BufferQueue::default();
        input.push_back(source.into());
        let _ = tokenizer.feed(&input);
        assert_eq!(tokenizer.sink.calls.get(), 1);
        let remaining = input.peek();
        let _ = tokenizer.feed(&input);
        tokenizer.end();
        assert_eq!(input.peek(), remaining);
        assert_eq!(tokenizer.sink.calls.get(), 1);
        assert!(!tokenizer.sink.ended.get());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NameError {
    Count,
    Bytes,
}

struct Names {
    admitted: Rc<RefCell<std::collections::BTreeSet<String>>>,
    count: usize,
    bytes: usize,
}

impl Admission<Sink> for Names {
    type Error = NameError;
    fn check(&self, operation: Operation<'_, usize>) -> Result<(), NameError> {
        if let Operation::Name(name) = operation {
            let mut admitted = self.admitted.borrow_mut();
            if admitted.contains(name) {
                return Ok(());
            }
            if admitted.len() == self.count {
                return Err(NameError::Count);
            }
            if admitted.iter().map(String::len).sum::<usize>() + name.len() > self.bytes {
                return Err(NameError::Bytes);
            }
            admitted.insert(name.into());
        }
        Ok(())
    }
}

#[test]
fn names_cover_implied_nodes_templates_adjustments_and_sticky_refusal() {
    let source = "<template><slot a='KEEP'></slot></template><svg xml:lang='en'><foreignObject><p a='x'>KEEP</p></foreignObject></svg><table><td>KEEP</table>";
    let admitted = Rc::new(RefCell::new(std::collections::BTreeSet::new()));
    let mut full = admission::parse_document(
        Sink::default(),
        Names {
            admitted: admitted.clone(),
            count: usize::MAX,
            bytes: usize::MAX,
        },
        Default::default(),
    );
    full.process_str(source).unwrap();
    full.finish().unwrap();
    for name in [
        "html",
        "head",
        "body",
        "template",
        "slot",
        "tbody",
        "tr",
        "foreignObject",
        "xml",
        "lang",
        "http://www.w3.org/XML/1998/namespace",
    ] {
        assert!(admitted.borrow().contains(name), "missing {name}");
    }
    let count = admitted.borrow().len();
    let bytes = admitted.borrow().iter().map(String::len).sum::<usize>();
    for (kind, boundary) in [(NameError::Count, count), (NameError::Bytes, bytes)] {
        for cap in [boundary - 1, boundary, boundary + 1] {
            let seen = Rc::new(RefCell::new(std::collections::BTreeSet::new()));
            let sink = Sink::default();
            let dropped = sink.dropped.clone();
            let finished = sink.finished.clone();
            let mut parser = admission::parse_document(
                sink,
                Names {
                    admitted: seen.clone(),
                    count: if kind == NameError::Count {
                        cap
                    } else {
                        usize::MAX
                    },
                    bytes: if kind == NameError::Bytes {
                        cap
                    } else {
                        usize::MAX
                    },
                },
                Default::default(),
            );
            let result = parser.process_str(source);
            if cap < boundary {
                assert_eq!(result, Err(kind));
                let names = seen.borrow().clone();
                let next = parser.input_buffer.peek();
                assert_eq!(parser.process_str("<lost-name>LOST</lost-name>"), Err(kind));
                assert_eq!(parser.input_buffer.peek(), next);
                assert_eq!(parser.finish(), Err(kind));
                assert_eq!(*seen.borrow(), names);
                assert!(!seen.borrow().contains("lost-name"));
                assert!(!finished.get());
            } else {
                result.unwrap();
                parser.finish().unwrap();
                assert_eq!(*seen.borrow(), *admitted.borrow());
                assert!(finished.get());
            }
            assert!(dropped.get());
        }
    }
}
