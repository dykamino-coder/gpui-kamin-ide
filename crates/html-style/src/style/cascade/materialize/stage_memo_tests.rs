//! Память стадии коробки: попадание при неизменных входах, промах по причине, правка стиля
//! видна синхронно, без кадра.

use super::super::tests::element;
use super::super::{box_style, materialize};
use super::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn query<'a>(parent: &'a BoxStyle, own: &crate::dom::SharedStyle, vcx: ValueCx<'a>) -> Query<'a> {
    Query {
        paint: true,
        layout: parent,
        tree: None,
        own: own.stage_own(),
        vcx,
    }
}

fn find(parent: &BoxStyle, own: &crate::dom::SharedStyle, vcx: ValueCx<'_>) -> Result<(), Miss> {
    own.stage_memo().find(&query(parent, own, vcx)).map(|_| ())
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
    let mut keyed = parent.clone();
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
    let held = p.style.stage_memo().0.get().unwrap().borrow().len();
    assert_eq!(held, 1);
    println!(
        "box stage record: {} bytes, BoxStyle {} bytes",
        entry_bytes(),
        std::mem::size_of::<BoxStyle>()
    );
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
