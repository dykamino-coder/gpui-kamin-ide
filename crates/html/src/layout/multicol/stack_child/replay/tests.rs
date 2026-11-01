//! Проверки повторного использования фрагментов многоколоночного потока.
use super::replayable;

fn first(html: &str) -> crate::dom::Element {
    let tree = crate::document::parsed::parse(html, "");
    fn find(nodes: &[crate::dom::Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            crate::dom::Node::Element(e) if e.tag == "section" => Some(e),
            crate::dom::Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(&tree).unwrap().clone()
}

#[test]
fn fragment_aware_painters_keep_their_copies() {
    let of = |html: &str| replayable(&first(html));
    assert_eq!(
        of("<section><p style='background:green'>a</p></section>"),
        (true, false)
    );
    assert_eq!(
        of("<section><p style='box-shadow:0 0 4px red'>a</p></section>"),
        (true, true)
    );
    assert_eq!(
        of("<section><div style='transform:rotate(1deg)'>a</div></section>"),
        (true, true)
    );
    assert_eq!(
        of("<section><div style='transform:rotateX(1deg)'>a</div></section>"),
        (false, false)
    );
    assert_eq!(
        of("<section><div style='columns:2'>a</div></section>"),
        (true, false)
    );
    assert_eq!(
        of("<section><div style='position:absolute'>a</div></section>"),
        (true, false)
    );
    assert_eq!(
        of(
            "<section><div style='transform:translateX(0)'><p style='position:fixed'>a</p></div></section>"
        ),
        (false, false)
    );
    assert_eq!(
        of("<section><div style='position:relative;opacity:0.5'>a</div></section>"),
        (true, false)
    );
    assert_eq!(
        of("<section><div style='display:grid'>a</div></section>"),
        (true, false)
    );
    assert_eq!(
        of(
            "<section><table style='border-collapse:collapse'><tr><td>a</td></tr></table></section>"
        ),
        (true, true)
    );
    assert_eq!(
        of("<section><div style='filter:url(#f)'></div></section>"),
        (true, true)
    );
    assert_eq!(
        of("<section><div style='backdrop-filter:blur(2px)'>a</div></section>"),
        (false, false)
    );
}

#[test]
fn sheet_groups_without_fragment_painters_or_layers_replay() {
    let of = |html: &str| super::page_replayable(&[crate::dom::Node::Element(first(html))]);
    assert!(of("<section><p style='background:green'>a</p></section>"));
    assert!(!of(
        "<section><p style='box-shadow:0 0 4px red'>a</p></section>"
    ));
    assert!(!of(
        "<section><div style='position:absolute'>a</div></section>"
    ));
    assert!(!of("<section><div style='columns:2'>a</div></section>"));
}
