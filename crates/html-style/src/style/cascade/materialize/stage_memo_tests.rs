//! Box stage memory: a hit while the inputs hold, a miss by its reason, a style edit
//! seen synchronously, without a frame.

use super::super::tests::element;
use super::super::{box_style, materialize};
use super::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn find(parent: &BoxStyle, own: &crate::dom::SharedStyle, vcx: ValueCx<'_>) -> Result<(), Miss> {
    use crate::style::cascade::inherit::BoxParents;
    let facts = ParentFacts::of(BoxParents::same(parent), vcx);
    let own_key = own.stage_own();
    let materialized = own_key.materialized();
    let query = Query {
        paint: true,
        moved: false,
        parent: &facts,
        element_parent: parent.element_style.as_ref().filter(|_| materialized),
        anonymous: (!materialized).then(|| parent.style_rc()),
        own: own_key,
        vcx,
    };
    own.stage_memo().find(&query).map(|_| ())
}
#[test]
fn a_new_viewport_width_recomputes_only_the_boxes_that_read_it() {
    let parsed = crate::dom::parse(
        r#"<body><p style="width:40px">a</p><i style="font-size:10vw">b</i></body>"#,
        "",
    );
    let wide = ValueCx {
        viewport: (800.0, 600.0),
        ..parsed.values()
    };
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, wide);
    let parent = BoxStyle::shared(root.clone());
    let (p, i) = (element(&parsed.nodes, "p"), element(&parsed.nodes, "i"));
    assert_eq!(find(&parent, &p.style, wide), Err(Miss::First));
    let first = box_style(&parent, &p.style, wide);
    box_style(&parent, &i.style, wide);
    assert_eq!(find(&parent, &p.style, wide), Ok(()));
    assert_eq!(find(&parent, &i.style, wide), Ok(()));
    // The window narrows: only the element whose values read the viewport has a new stage.
    let narrow = ValueCx {
        viewport: (500.0, 600.0),
        ..wide
    };
    materialize(&parsed.nodes, &root, narrow);
    assert_eq!(find(&parent, &p.style, narrow), Ok(()));
    assert_eq!(find(&parent, &i.style, narrow), Err(Miss::Own));
    let again = box_style(&parent, &p.style, narrow);
    assert!(Rc::ptr_eq(first.style_rc(), again.style_rc()));
    assert_eq!(
        box_style(&parent, &i.style, narrow).inh.font.font_size,
        Some(Len::Px(50.0))
    );
}

#[test]
fn a_style_edit_is_read_at_once_and_a_changed_parent_misses() {
    let parsed = crate::dom::parse(r#"<body><p style="width:40px">a</p></body>"#, "");
    let vcx = parsed.values();
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, vcx);
    let parent = BoxStyle::shared(root.clone());
    let mut p = element(&parsed.nodes, "p").clone();
    box_style(&parent, &p.style, vcx);
    // A script writes the style: the next synchronous read sees it, with no frame between.
    p.style.cascaded_mut().reset_box_layout.width = Some(Len::Px(70.0));
    assert_eq!(find(&parent, &p.style, vcx), Err(Miss::Own));
    let read = box_style(&parent, &p.style, vcx);
    assert_eq!(read.reset_box_layout.width, Some(Len::Px(70.0)));
    assert_eq!(find(&parent, &p.style, vcx), Ok(()));
    // A frame key of the parent is no input; a value the stage reads is.
    let mut keyed = parent;
    keyed.self_node = 42;
    assert_eq!(find(&keyed, &p.style, vcx), Ok(()));
    assert_eq!(box_style(&keyed, &p.style, vcx).cb_node, 0);
    let mut flex = (*root).clone();
    flex.reset_box_layout.display = Some(crate::style::computed::Display::Flex);
    let flex = BoxStyle::new(flex);
    assert_eq!(find(&flex, &p.style, vcx), Err(Miss::Parent));
}

#[test]
fn records_stay_bounded_per_style() {
    let parsed = crate::dom::parse(r#"<body><p>a</p></body>"#, "");
    let vcx = parsed.values();
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, vcx);
    let p = element(&parsed.nodes, "p");
    for px in 0..20 {
        let mut parent = (*root).clone();
        parent.inh.font.font_size = Some(Len::Px(10.0 + px as f32));
        box_style(&BoxStyle::new(parent), &p.style, vcx);
    }
    let held = p.style.stage_memo().0.borrow().len();
    assert_eq!(held, 1);
    // One record: the remembered box style and its keys, the parent facts shared by siblings.
    assert!(entry_bytes() < 2 * std::mem::size_of::<BoxStyle>());
}

#[test]
fn a_stage_that_read_the_viewport_misses_on_a_new_width_alone() {
    // No materialization: the stage computes the cascaded style itself (anonymous path).
    let parsed = crate::dom::parse(r#"<b style="font-size:10vw">a</b><u>c</u>"#, "");
    let wide = ValueCx {
        viewport: (800.0, 600.0),
        ..parsed.values()
    };
    let parent = BoxStyle::default();
    let (b, u) = (element(&parsed.nodes, "b"), element(&parsed.nodes, "u"));
    box_style(&parent, &b.style, wide);
    box_style(&parent, &u.style, wide);
    let narrow = ValueCx {
        viewport: (500.0, 600.0),
        ..wide
    };
    assert_eq!(find(&parent, &b.style, narrow), Err(Miss::Viewport));
    assert_eq!(find(&parent, &u.style, narrow), Ok(()));
    assert_eq!(find(&parent, &b.style, wide), Ok(()));
}

#[test]
fn a_parent_value_the_stage_does_not_read_keeps_the_record() {
    // Layout writes used values into the parent's style after its stage (the window width
    // of the viewport box, resolved `vw`): a child that reads none of them keeps its stage.
    let parsed = crate::dom::parse(r#"<body><p style="width:40px">a</p></body>"#, "");
    let vcx = parsed.values();
    let root = Rc::new(Computed::default());
    materialize(&parsed.nodes, &root, vcx);
    let p = element(&parsed.nodes, "p");
    let mut wide = (*root).clone();
    wide.reset_box_layout.width = Some(Len::Px(800.0));
    wide.reset_box_layout.max_width = Some(Len::Px(720.0));
    let first = box_style(&BoxStyle::new(wide.clone()), &p.style, vcx);
    let mut narrow = wide;
    narrow.reset_box_layout.width = Some(Len::Px(500.0));
    narrow.reset_box_layout.max_width = Some(Len::Px(450.0));
    let narrow = BoxStyle::new(narrow);
    assert_eq!(find(&narrow, &p.style, vcx), Ok(()));
    let again = box_style(&narrow, &p.style, vcx);
    assert!(Rc::ptr_eq(first.style_rc(), again.style_rc()));
    // A value it reads is an input: a definite parent height defines the child's basis.
    let mut tall = (*root).clone();
    tall.reset_box_layout.height = Some(Len::Px(300.0));
    assert_eq!(find(&BoxStyle::new(tall), &p.style, vcx), Err(Miss::Parent));
}
