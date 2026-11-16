//! Тесты правок разметки режима `edit`.

use super::Edit;

fn page() -> String {
    let body = "<p>alpha</p>".repeat(10);
    format!(
        "<html><head><style>p{{a:b}}</style></head><body>{body}<script>var a</script></body></html>"
    )
}

#[test]
fn text_edit_changes_one_letter_of_rendered_text_after_the_middle() {
    let page = page();
    let edited = Edit::Text.markup(&page).unwrap();
    assert_eq!(edited.len(), page.len());
    assert_eq!(edited.matches("<p>olpha</p>").count(), 1, "{edited}");
    assert!(edited.contains("var a"));
}

#[test]
fn attribute_and_class_edits_add_one_attribute() {
    let page = page();
    let edited = Edit::Attr.markup(&page).unwrap();
    assert_eq!(
        edited.matches("<p data-edit=\"1\">alpha").count(),
        1,
        "{edited}"
    );
    let edited = Edit::Class.markup(&page).unwrap();
    assert!(edited.starts_with("<html class=\"alt\">"), "{edited}");
    assert!(Edit::Class.markup("<html class=x>").is_err());
}

#[test]
fn insert_and_remove_change_one_leaf_element_after_the_middle() {
    let page = page();
    let inserted = Edit::Insert.markup(&page).unwrap();
    assert_eq!(inserted.matches("<p>alpha</p>").count(), 11, "{inserted}");
    assert_eq!(inserted.len(), page.len() + "<p>alpha</p>".len());
    let removed = Edit::Remove.markup(&page).unwrap();
    assert_eq!(removed.matches("<p>alpha</p>").count(), 9, "{removed}");
    assert!(removed.contains("<script>var a</script>"));
    assert_eq!(Edit::Remove.markup(&inserted).unwrap(), page);
}

#[test]
fn a_leaf_is_an_element_without_child_elements_outside_raw_text() {
    // The padding puts the middle of the document before every candidate.
    let tail = "<div><br><IMG src=x><em/><span>x</SPAN><b>y</b></div><script>a<b></b></script>";
    let page = format!("<body>{}{tail}</body>", " ".repeat(200));
    let removed = Edit::Remove.markup(&page).unwrap();
    assert_eq!(
        removed.trim_start_matches("<body>").trim_start(),
        tail.replace("<span>x</SPAN>", "") + "</body>"
    );
    let inserted = Edit::Insert.markup(&page).unwrap();
    assert!(
        inserted.contains("<span>x</SPAN><span>x</SPAN><b>y</b>"),
        "{inserted}"
    );
    assert!(
        Edit::Insert
            .markup("<body><br><script>x</script></body>")
            .is_err()
    );
}

#[test]
fn class_goes_on_the_implicit_root_after_the_doctype() {
    let page = "<!doctype html><style>p{}</style><div>x</div>";
    let edited = Edit::Class.markup(page).unwrap();
    assert_eq!(
        edited,
        "<!doctype html><html class=\"alt\"><style>p{}</style><div>x</div>"
    );
    assert_eq!(
        Edit::Class.markup("<div>x</div>").unwrap(),
        "<html class=\"alt\"><div>x</div>"
    );
}

#[test]
fn attribute_falls_back_to_the_last_start_tag_before_the_middle() {
    let page = format!("<body>{}{}", "<div>".repeat(20), "</div>".repeat(20));
    let edited = Edit::Attr.markup(&page).unwrap();
    assert_eq!(
        edited.matches("<div data-edit=\"1\">").count(),
        1,
        "{edited}"
    );
    assert!(
        edited.ends_with(&format!("<div data-edit=\"1\">{}", "</div>".repeat(20))),
        "{edited}"
    );
}

#[test]
fn unknown_edits_are_rejected() {
    assert!(Edit::parse("resize").is_err());
    assert!(Edit::parse("nope").is_err());
    assert_eq!(Edit::parse("attr").unwrap(), Edit::Attr);
}
