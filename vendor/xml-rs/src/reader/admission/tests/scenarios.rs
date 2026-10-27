//! Directed canonical semantics and admission lifetime regressions.

use super::*;

#[test]
fn every_callback_refusal_aborts_and_releases_partial_state() {
    let policy = Policy::default();
    parse(XML, policy.clone()).unwrap();
    let callbacks = policy.metrics.callbacks.load(Relaxed);
    for fail_callback in 0..callbacks {
        let policy = Policy {
            fail_callback,
            ..Default::default()
        };
        assert!(matches!(
            parse(XML, policy.clone()),
            Err(Failure::Admission(Refusal::Callback))
        ));
        assert_eq!(policy.metrics.callbacks.load(Relaxed), fail_callback + 1);
        assert_eq!(policy.metrics.live.load(Relaxed), 0);
    }
}

#[test]
fn exact_work_names_and_live_storage_boundaries() {
    let observed = Policy::default();
    parse(XML, observed.clone()).unwrap();
    let limits = [
        observed.metrics.work.load(Relaxed),
        observed.metrics.names.load(Relaxed),
        observed.metrics.peak.load(Relaxed),
    ];
    for (kind, boundary) in limits.into_iter().enumerate() {
        for limit in [boundary - 1, boundary, boundary + 1] {
            let mut policy = Policy::default();
            match kind {
                0 => policy.work_limit = limit,
                1 => policy.name_limit = limit,
                _ => policy.live_limit = limit,
            }
            let result = parse(XML, policy.clone());
            if limit < boundary {
                let expected = [Refusal::Work, Refusal::Names, Refusal::Storage][kind];
                assert!(matches!(result, Err(Failure::Admission(reason)) if reason == expected));
            } else {
                result.unwrap();
            }
            assert_eq!(policy.metrics.live.load(Relaxed), 0);
        }
    }
}

#[test]
fn event_storage_outlives_parser_and_documents_are_independent() {
    let first = Policy::default();
    let second = Policy::default();
    let mut reader = AbortableReader::new(
        "<KEEP a='v'/>".as_bytes(),
        ParserConfig2::new(),
        first.clone(),
    )
    .unwrap();
    let mut events = Vec::new();
    for event in reader.by_ref() {
        events.push(event.unwrap());
    }
    assert!(first.metrics.live.load(Relaxed) > 0);
    parse(XML, second.clone()).unwrap();
    assert_eq!(second.metrics.live.load(Relaxed), 0);
    assert!(first.metrics.live.load(Relaxed) > 0);
    drop(reader);
    assert!(first.metrics.live.load(Relaxed) > 0);
    drop(events);
    assert_eq!(first.metrics.live.load(Relaxed), 0);
}

#[test]
fn syntax_is_distinct_and_sentinel_tail_is_never_read_after_refusal() {
    for source in [
        "",
        "<a><b></a>LOST",
        "<a>&#invalid;</a>LOST",
        "<a p:x='v'/>LOST",
    ] {
        let policy = Policy::default();
        assert!(matches!(
            parse(source, policy.clone()),
            Err(Failure::Xml(_))
        ));
        assert_eq!(policy.metrics.live.load(Relaxed), 0);
        assert!(policy.metrics.read.load(Relaxed) <= source.len().saturating_sub(4).max(1));
    }
    let policy = Policy {
        work_limit: 1,
        ..Default::default()
    };
    assert!(matches!(
        parse("<KEEP/>LOST", policy.clone()),
        Err(Failure::Admission(Refusal::Work))
    ));
    assert_eq!(policy.metrics.read.load(Relaxed), 0);
}

#[test]
fn legacy_and_admitted_events_have_identical_canonical_semantics() {
    for source in [
        XML,
        "<r xmlns:p='a'><p:a xmlns:p='b'/><p:b/></r>",
        "<r> a &#65; b </r>",
    ] {
        let config = ParserConfig2::new().ignore_comments(false);
        let expected: Vec<_> =
            crate::reader::EventReader::new_with_config(source.as_bytes(), config.clone())
                .into_iter()
                .map(Result::unwrap)
                .collect();
        let policy = Policy::default();
        let actual: Vec<_> = AbortableReader::new(source.as_bytes(), config, policy.clone())
            .unwrap()
            .map(|event| event.unwrap().into_legacy())
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(policy.metrics.live.load(Relaxed), 0);
    }
}

#[test]
fn expanded_attribute_names_and_namespace_declarations_are_unique() {
    for source in [
        "<r a='KEEP' a='LOST'/>",
        "<r xmlns='KEEP' xmlns='LOST'/>",
        "<r xmlns:p='KEEP' xmlns:p='LOST'/>",
        "<r xmlns:p='urn:a' xmlns:q='urn:a' p:a='KEEP' q:a='LOST'/>",
    ] {
        let policy = Policy::default();
        assert!(matches!(
            parse(source, policy.clone()),
            Err(Failure::Xml(_))
        ));
        assert_eq!(policy.metrics.live.load(Relaxed), 0);
    }
    parse(
        "<r xmlns:p='urn:a' xmlns:q='urn:b' p:a='KEEP' q:a='LOST'/>",
        Policy::default(),
    )
    .unwrap();
}

#[test]
fn cyclic_entities_are_syntax_errors_without_reading_the_tail() {
    let source = "<!DOCTYPE r [<!ENTITY cycle '&cycle;'>]><r>KEEP&cycle;LOST</r>";
    let policy = Policy::default();
    assert!(matches!(
        parse(source, policy.clone()),
        Err(Failure::Xml(_))
    ));
    assert!(policy.metrics.read.load(Relaxed) <= source.find("LOST").unwrap());
}

#[test]
fn entity_scopes_end_after_the_replacement_and_allow_repeated_siblings() {
    for source in [
        "<!DOCTYPE r [<!ENTITY a 'KEEP'>]><r>&a;&a;LOST</r>",
        "<!DOCTYPE r [<!ENTITY a '&b;'><!ENTITY b 'KEEP'>]><r>&a;&b;&a;LOST</r>",
        "<!DOCTYPE r [<!ENTITY a '<child/>'>]><r>&a;&a;LOST</r>",
    ] {
        parse(source, Policy::default()).unwrap();
    }
    for source in [
        "<!DOCTYPE r [<!ENTITY a '&b;'><!ENTITY b '&a;'>]><r>KEEP&a;LOST</r>",
        "<!DOCTYPE r [<!ENTITY a 'x&b;'><!ENTITY b 'y&a;'>]><r>KEEP&a;LOST</r>",
    ] {
        let policy = Policy::default();
        assert!(matches!(
            parse(source, policy.clone()),
            Err(Failure::Xml(_))
        ));
        assert!(policy.metrics.read.load(Relaxed) <= source.find("LOST").unwrap());
    }
}
