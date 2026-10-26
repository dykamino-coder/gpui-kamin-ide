//! Admission failures are terminal, including entity work without reader calls.
use super::*;
use crate::reader::EventReader;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Exhausted {
    limit: usize,
    attempted: usize,
}
struct Policy {
    limit: usize,
    calls: Rc<Cell<usize>>,
}
impl Admission for Policy {
    type Error = Exhausted;
    fn check(&mut self, _: Operation) -> Result<(), Self::Error> {
        let attempted = self.calls.get() + 1;
        self.calls.set(attempted);
        if attempted > self.limit {
            Err(Exhausted {
                limit: self.limit,
                attempted,
            })
        } else {
            Ok(())
        }
    }
}
struct Source<'a> {
    bytes: &'a [u8],
    reads: Rc<Cell<usize>>,
}
impl Read for Source<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.bytes.read(output)
    }
}
fn run(source: &str, limit: usize) -> (Result<Vec<XmlEvent>, Failure<Exhausted>>, usize) {
    let calls = Rc::new(Cell::new(0));
    let reads = Rc::new(Cell::new(0));
    let mut reader = AbortableReader::new(
        Source {
            bytes: source.as_bytes(),
            reads: reads.clone(),
        },
        ParserConfig2::new(),
        Policy {
            limit,
            calls: calls.clone(),
        },
    );
    let mut events = Vec::new();
    loop {
        match reader.next() {
            Ok(XmlEvent::EndDocument) => {
                events.push(XmlEvent::EndDocument);
                return (Ok(events), calls.get());
            },
            Ok(event) => events.push(event),
            Err(error) => {
                let stopped = (calls.get(), reads.get(), reader.source().bytes.len());
                for _ in 0..5 {
                    assert_eq!(reader.next(), Err(error.clone()));
                }
                assert_eq!(
                    stopped,
                    (calls.get(), reads.get(), reader.source().bytes.len())
                );
                return (Err(error), calls.get());
            },
        }
    }
}
const SOURCES: &[&str] = &[
    "<r>KEEP<a/>LOST</r>",
    "<?xml version='1.0'?><r xmlns='urn:r' xmlns:a='urn:a' a:b='v'>x&amp;y</r>",
    "<!DOCTYPE r [<!ENTITY e 'abc&amp;def'>]><r>&e;&e;KEEP<x/>LOST</r>",
    "<r><!--KEEP--><![CDATA[LOST]]><?target data?></r>",
    "<r><a x='unterminated",
    "<r>KEEP</m>LOST",
    "<r>&unknown;</r>",
];
#[test]
fn canonical_events_and_every_rejection_point() {
    for source in SOURCES {
        let expected = EventReader::new(source.as_bytes())
            .into_iter()
            .collect::<Result<Vec<_>, _>>();
        let (actual, count) = run(source, usize::MAX);
        assert_eq!(
            actual.map_err(|e| match e {
                Failure::Xml(e) => e,
                _ => unreachable!(),
            }),
            expected
        );
        for limit in 0..count {
            let (result, calls) = run(source, limit);
            assert_eq!(
                result,
                Err(Failure::Admission(Exhausted {
                    limit,
                    attempted: limit + 1
                }))
            );
            assert_eq!(calls, limit + 1, "{source}, limit {limit}");
        }
    }
}
#[test]
fn boundary_minus_one_exact_and_plus_one_preserve_keep_lost() {
    let source = SOURCES[0];
    let (expected, boundary) = run(source, usize::MAX);
    assert!(matches!(
        run(source, boundary - 1).0,
        Err(Failure::Admission(_))
    ));
    for limit in [boundary, boundary + 1] {
        let events = run(source, limit).0.unwrap();
        assert_eq!(Ok(events.clone()), expected);
        let text: String = events
            .into_iter()
            .filter_map(|e| match e {
                XmlEvent::Characters(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(text, "KEEPLOST");
    }
}
#[test]
fn entity_queue_can_abort_without_reading_another_byte() {
    let source = SOURCES[2];
    let (_, count) = run(source, usize::MAX);
    let mut found_queued_work = false;
    for limit in 1..count {
        let calls = Rc::new(Cell::new(0));
        let reads = Rc::new(Cell::new(0));
        let mut reader = AbortableReader::new(
            Source {
                bytes: source.as_bytes(),
                reads: reads.clone(),
            },
            ParserConfig2::new(),
            Policy { limit, calls },
        );
        while reader.next().is_ok() {}
        let offset = source.len() - reader.source().bytes.len();
        if offset == source.find("&e;").unwrap() + 3 {
            found_queued_work = true;
        }
    }
    assert!(
        found_queued_work,
        "admission must cover reparsing after an entity reference"
    );
}
