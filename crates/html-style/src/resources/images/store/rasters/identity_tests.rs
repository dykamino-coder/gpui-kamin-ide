//! Проверки замкнутости ресурсов и точного ключа для остальных SVG.

use super::*;

fn svg(body: &str) -> String {
    format!("<svg xmlns='http://www.w3.org/2000/svg'>{body}</svg>")
}

#[test]
fn local_names_do_not_change_identity_but_reference_edges_do() {
    let a = svg(
        "<defs><linearGradient id='paint'/><linearGradient id='other'/></defs><path fill='url(#paint)'/>",
    );
    let b = a.replace("paint", "new_paint").replace("other", "second");
    assert_eq!(Identity::new(&a), Identity::new(&b));
    assert!(Identity::new(&a).local);
    assert_ne!(
        Identity::new(&a),
        Identity::new(&a.replace("url(#paint)", "url(#other)"))
    );
    assert_ne!(
        Identity::new(&a),
        Identity::new(&a.replace("<path", "<rect"))
    );
}

#[test]
fn href_and_paint_server_links_follow_their_targets() {
    let a = svg(
        "<defs><path id='shape' d='M0 0L2 1'/></defs><use href='#shape'/><text><textPath href='#shape'>test</textPath></text>",
    );
    assert_eq!(
        Identity::new(&a),
        Identity::new(&a.replace("shape", "renamed"))
    );
    let xlink = a
        .replace("<svg ", "<svg xmlns:xlink='http://www.w3.org/1999/xlink' ")
        .replace("href=", "xlink:href=");
    assert_eq!(
        Identity::new(&xlink),
        Identity::new(&xlink.replace("shape", "renamed"))
    );
}

#[test]
fn incomplete_or_dynamic_scopes_retain_exact_identity() {
    for body in [
        "<path id='x'/><use href='#absent'/>",
        "<path id='x' fill='url(#absent)'/>",
        "<path id='x'/><path id='x'/>",
        "<path id='x'/><image href='data:image/png;base64,AA=='/>",
        "<path id='x'/><image href='#x'/>",
        "<path id='x'/><use href='other.svg#x'/>",
        "<path id='x'/><style>#x {fill:red}</style>",
        "<path id='x' style='fill:red'/>",
        "<path id='x' onclick='change()'/>",
        "<path id='x'/><animate attributeName='id'/>",
        "<foreignObject id='x'/>",
        "<path id='x'/><unknown/>",
        "<path id='x' xml:base='https://example.test/'/>",
        "<path id='x' fill='URL(#x)'/>",
        "<path id='x' fill='url(&quot;#x&quot;)'/>",
        "<path id='x' fill='url(#x) red'/>",
        "<path id='x' fill='url(#\\78)'/>",
        "<path id='x'/><use href='#%78'/>",
        "<path id='имя'/>",
        "<path id='x.y'/>",
        "<path id='x'/><?instruction keep-ids?>",
        "<path xmlns='urn:foreign' id='x'/>",
    ] {
        let markup = svg(body);
        let identity = Identity::new(&markup);
        assert!(!identity.local, "{body}");
        assert_eq!(&*identity.markup, markup);
        assert_eq!(identity, Identity::new(&markup));
    }
}

#[test]
fn fallback_and_local_keys_never_alias_even_with_the_same_bytes() {
    let local = Identity::new(&svg("<path id='x'/>"));
    let exact = Identity {
        local: false,
        markup: local.markup.clone(),
    };
    assert_ne!(local, exact);
    assert_eq!(local, Identity::new(&local.markup));
}

#[test]
fn entity_spelling_does_not_break_attribute_ranges() {
    let a = svg("<path id='&#120;'/><use href='#x'/>");
    let b = svg("<path id='other'/><use href='#other'/>");
    assert_eq!(Identity::new(&a), Identity::new(&b));
    let dtd = "<!DOCTYPE svg [<!ENTITY id 'x'>]><svg xmlns='http://www.w3.org/2000/svg'><path id='&id;'/></svg>";
    assert!(!Identity::new(dtd).local);
}

#[test]
fn only_id_tokens_change_and_other_source_bytes_are_preserved() {
    let a = svg("<path id='x' fill='  url(#x)  ' data-note='#x'>x</path>");
    let b = svg("<path id='y' fill='  url(#y)  ' data-note='#x'>x</path>");
    assert_eq!(Identity::new(&a), Identity::new(&b));
    for changed in [
        a.replace("  url(#x)  ", "url(#x)"),
        a.replace(">x<", ">y<"),
        a.replace("data-note='#x'", "data-note='#y'"),
    ] {
        assert_ne!(Identity::new(&a), Identity::new(&changed));
    }
}

#[test]
fn source_and_node_limits_fall_back_without_changing_the_key() {
    let large = svg(&format!(
        "<path id='x' data-note='{}'/>",
        "a".repeat(256 * 1024)
    ));
    let many = svg(&format!("<path id='x'/>{}", "<path/>".repeat(10_001)));
    for markup in [large, many, "malformed id".into(), svg("<path/>")] {
        let identity = Identity::new(&markup);
        assert!(!identity.local);
        assert_eq!(&*identity.markup, markup);
    }
}
