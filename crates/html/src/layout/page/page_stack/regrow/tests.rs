//! Рост потоков и актуальность независимых продолжений страницы.
use super::*;
use crate::layout::fragment::flow::{LinePush, blocks::Pushed};
use std::{cell::RefCell, rc::Rc};

#[test]
fn unforced_line_break_consumes_unused_fragment_extent_once() {
    let blocks = BlockBox {
        id: 1,
        ..Default::default()
    };
    let growth = Rc::new(RefCell::new(Vec::new()));
    let mut laid = Laid::default();
    laid.lines.insert(
        1,
        vec![LinePush {
            at: 80.0,
            line: 2,
            growth: growth.clone(),
        }],
    );
    let kid = Kid {
        h: 160.0,
        cuts: vec![(80.0, 80.0)],
        ..Default::default()
    };
    let mut frag = Frag {
        kid: 0,
        copy: 0,
        col: 0,
        y: 0.0,
        from: 0.0,
        h: 80.0,
        head: 0.0,
        foot: 0.0,
    };
    let grows = growths(&blocks, &kid, &laid, &frag, 100.0);
    assert_eq!(grows.len(), 1);
    assert_eq!(grows[0].amount, 20.0);
    assert!(!grows[0].forced);
    let Pushed::Line(line) = &grows[0].target else {
        panic!("line boundary")
    };
    assert!(line.grow(grows[0].amount));
    assert_eq!(*growth.borrow(), vec![0.0, 0.0, 20.0]);
    // После публикации занятого места новый разрыв стоит у края.
    frag.h = 100.0;
    assert!(growths(&blocks, &kid, &laid, &frag, 100.0).is_empty());
}

#[test]
fn moving_a_main_float_rebuilds_continuations_with_the_new_floor() {
    use crate::layout::fragment::types::StackAxis;
    use crate::layout::fragment::{flow::FloatGeometry, paginate::watch::Watched, push::Regrow};
    use gpui::IntoElement;
    let doc = crate::dom::parse("<div></div>", "");
    let crate::dom::Node::Element(root) = &doc[0] else {
        panic!("document root")
    };
    let watched = Watched::for_blocks_axis(BlockBox::default(), StackAxis::Horizontal, true);
    let regrow = Regrow {
        axis: StackAxis::Horizontal,
        source: root.clone(),
        dom: root.clone(),
        build: Rc::new(|_, _, _, _| gpui::div().into_any_element()),
        opts: crate::layout::options::RenderOpts {
            text: Default::default(),
            viewport: (100., 100.),
            normal_line_height: 1.,
            cx: Default::default(),
            frag: Default::default(),
        },
        laid: Laid::default(),
        floats: vec![FloatGeometry {
            id: 7,
            anchor: 40.,
            top: 40.,
            bottom: 140.,
            margins: [0.; 2],
            first_break: None,
        }],
        float_leads: Vec::new(),
        space: None,
        laid_h: true,
        watched: watched.clone(),
    };
    let kid = super::super::PageKid {
        #[cfg(feature = "wpt-harness")]
        probe: Default::default(),
        el: gpui::div().into_any_element(),
        regrow: Some(regrow.clone()),
        percentages: None,
        leaf: None,
        oof: None,
        watched: Some(watched),
        float_id: None,
        float_extent: false,
        repeat: None,
        frags: Vec::new(),
        monolith: false,
        force_before: false,
        renamed: false,
        force_after: false,
        shape: None,
        page: String::new(),
        mt: 0.,
        mb: 0.,
        inner_top: 0.,
        reach: None,
        end: Default::default(),
    };
    let mut stack = PageStack::new(
        vec![kid],
        Rc::new(|_, _| panic!("no page layout")),
        Vec::new(),
        0.,
        Vec::new(),
        None,
    );
    stack.continuations.insert((0, 1), regrow.clone());
    stack.continuations.insert((1, 1), regrow);
    let frag = Frag {
        kid: 0,
        copy: 0,
        col: 0,
        y: 0.,
        from: 0.,
        h: 40.,
        head: 0.,
        foot: 0.,
    };
    assert!(stack.regrow_flex(
        &[Kid {
            h: 200.,
            ..Default::default()
        }],
        &[frag],
        &[100.]
    ));
    let current = stack.kids[0].regrow.as_ref().unwrap();
    assert_eq!(current.opts.frag.float_floors[&7], 100.);
    assert!(!stack.continuations.contains_key(&(0, 1)));
    assert!(stack.continuations.contains_key(&(1, 1)));
    // Следующая независимая копия наследует новый floor, а не старый Rc-снимок.
    assert_eq!(current.clone().opts.frag.float_floors[&7], 100.);
}
