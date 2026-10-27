//! Native CB задаёт отложенный abspos, разные fixed-повторы и transformed fixed.

use super::*;
use crate::dom::Node;
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use crate::layout::fragment::types::Frag;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::metrics::layout::PageSource;
use gpui::{IntoElement, point, px, size};
use std::rc::Rc;

fn kid(source: PageSource) -> super::super::PageKid {
    super::super::PageKid {
        #[cfg(feature = "wpt-harness")]
        probe: Default::default(),
        el: gpui::div().into_any_element(),
        regrow: None,
        percentages: None,
        leaf: None,
        oof: Some(source),
        watched: None,
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
        end: std::cell::Cell::new(20.),
    }
}

fn geom(index: usize) -> PageGeom {
    let area = [(100., 70.), (120., 60.), (140., 80.)][index.min(2)];
    PageGeom {
        size: area,
        area,
        margin: [0.; 4],
        padding: [0.; 4],
        border: (0., gpui::black()),
        outline: (0., 0., gpui::black()),
        bg: gpui::white(),
        canvas: None,
        turn: 0,
    }
}

#[gpui::test]
fn actual_page_cb_repeats_fixed_through_the_last_absolute_fragment(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let tree = crate::dom::parse("<section style='position:relative;height:20px'><div style='position:absolute;top:95px;width:80px;height:85px'></div><div style='position:fixed;top:10%;width:10%;height:10%'><div style='position:absolute;top:95px;width:10px;height:150px'></div></div><div style='transform:translateX(0px);height:10px;width:40px'><div style='position:fixed;top:5px;width:10px;height:150px'><div style='position:absolute;top:95px;width:10px;height:150px'></div></div></div></section>", "");
        let mut opts = Box::new(RenderOpts { viewport: (400., 400.), normal_line_height: 1.2, text: Default::default(), cx: Default::default(), frag: Default::default() });
        opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
        opts.cx.values.viewport = opts.viewport;
        crate::style::cascade::materialize::materialize(&tree, opts.root_style().style_rc(), opts.cx.vcx());
        let mut pending: Vec<_> = tree.iter().collect();
        let host = loop {
            let Some(node) = pending.pop() else { panic!("section") };
            if let Node::Element(e) = node {
                if e.tag == "section" { break e.clone() }
                pending.extend(&e.children);
            }
        };
        let nodes = vec![Node::Element(host)];
        let root = opts.root_style();
        let source: PageSource = Rc::new(move |area, window, app| {
            crate::layout::positioned::containing_block::metrics::layout::collect_paged(
                &nodes, &root, (area, gpui::div()), &opts, window, app,
            )
        });
        let stack = PageStack::new(vec![kid(source)], Rc::new(|i, _| geom(i)), Vec::new(), 0., Vec::new(), None);
        let normal = Plan { fragments: vec![Frag { kid: 0, copy: 0, col: 0, y: 0., from: 0., h: 20., head: 0., foot: 0. }], ..Default::default() };
        stack.plan_oof(Normal { plan: &normal, geoms: &[geom(0)], names: &[String::new()], count: 1 }, window, app);
        let plan = stack.oof.plan.borrow();
        assert_eq!(plan.failure, None);
        assert_eq!(plan.completion, Completion::Complete);
        let sources = stack.oof.sources.borrow();
        assert_eq!(sources.len(), 5);
        let abs = sources.iter().position(|(_, s)| !s.fixed).unwrap();
        let fixed = sources.iter().position(|(_, s)| s.policy == FixedPolicy::RepeatPages).unwrap();
        let contained = sources.iter().position(|(_, s)| s.fixed && s.policy == FixedPolicy::Fragment).unwrap();
        let fragments = |id| plan.iter().filter(|f| f.kid == id).map(|f| (f.col, f.y, f.from, f.h)).collect::<Vec<_>>();
        assert_eq!(fragments(abs), [(1, 25., 0., 35.), (2, 0., 35., 50.)]);
        assert_eq!(fragments(fixed), [(0, 7., 0., 7.), (1, 6., 0., 6.), (2, 8., 0., 8.)]);
        assert_eq!(fragments(contained), [(0, 5., 0., 150.)]);
        assert_ne!(sources[contained].1.metrics.cb_id, 0);
        let repeated_child = sources.iter().position(|(_, s)| !s.fixed && s.inside_fixed && s.policy == FixedPolicy::RepeatPages).unwrap();
        let contained_child = sources.iter().position(|(_, s)| !s.fixed && s.inside_fixed && s.policy == FixedPolicy::Fragment).unwrap();
        assert_eq!(fragments(repeated_child), [(0, 102., 0., 150.), (1, 101., 0., 150.), (2, 103., 0., 150.)]);
        assert_eq!(fragments(contained_child), [(0, 100., 0., 150.)]);
        let repeats = stack.oof.fixed.borrow();
        assert_eq!(repeats.iter().filter(|(_, _, s)| s.fixed).map(|(page, _, s)| (*page, f32::from(s.metrics.normal_size.width))).collect::<Vec<_>>(), [(0, 10.), (1, 12.), (2, 14.)]);
        assert_eq!(normal[0].h, 20.);
        gpui::div()
    });
}
