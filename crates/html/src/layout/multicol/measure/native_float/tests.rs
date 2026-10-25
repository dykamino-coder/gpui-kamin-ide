//! Native-копии floats: замер без оценки и сохранение parallel overflow P05.

use super::*;
use crate::layout::fragment::types::StackAxis;
use crate::layout::multicol::deferred::Recipe;
use crate::layout::options::RenderOpts;

fn host(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored host")
}

#[test]
fn unknown_width_float_reuses_native_solver_and_resolves_calc_margins() {
    let tree = crate::dom::parse(
        "<div><div style='margin:calc(10% + 3px) 0 0'><div style='float:left;width:20px;overflow:scroll'>float</div>text</div><div>ordinary</div></div>",
        "",
    );
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    let recipe = Recipe::new(
        host(&tree),
        &BoxStyle::default(),
        &opts,
        2,
        0.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .expect("deferred native float");
    let kids = recipe.children(120.0);
    assert_eq!(kids.len(), 2);
    assert!(kids[0].paginate.is_some());
    assert_eq!(kids[0].measure, Some(120.0));
    assert_eq!(kids[0].mt, 15.0);
    assert!(kids[1].regrow.is_some());
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[test]
fn empty_float_host_is_measured_without_a_height_estimate() {
    let tree = crate::dom::parse(
        "<div><div><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div></div>",
        "",
    );
    let opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let recipe = Recipe::new(
        host(&tree),
        &BoxStyle::default(),
        &opts,
        2,
        0.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .unwrap();
    let kids = recipe.children(100.0);
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].measure, Some(100.0));
    assert!(kids[0].paginate.is_none());
    assert!(kids[0].watched.as_ref().is_some_and(|w| w.has_blocks()));
    #[cfg(feature = "wpt-harness")]
    assert!(
        opts.frag
            .probe
            .take()
            .values()
            .all(|p| p.estimate_calls == 0)
    );
}

#[test]
fn parallel_float_continuations_are_not_silently_replaced() {
    // P05 сохраняет normal box заданной высоты и отдельные child tokens
    // переполнения. Пустой монолитный float теперь принадлежит native-копии,
    // поэтому прежний отказ по высоте родителя здесь больше не нужен.
    let tree = crate::dom::parse(
        "<div><div><div style='height:100px'><div style='float:left;width:100%;height:100px;break-inside:avoid'></div></div></div></div>",
        "",
    );
    let kids = pending(host(&tree), &BoxStyle::default(), tree.values())
        .expect("fixed-height empty float host keeps its parallel continuation");
    assert!(!kids[0].0.marks.paginate);
    assert!(empty_floats(&kids[0].0));
    for special in [
        "<div style='float:left;width:20px'>several flowing lines</div>",
        "<div style='float:left;width:100%;height:20px;margin:40px 0;contain:size'>float</div>",
        "<div style='float:left'><div>A</div><div>B</div></div>",
        "<div style='float:left;break-inside:avoid'><div>A</div><div>B</div></div>",
        "<div style='float:left'>float</div><div style='box-decoration-break:clone'>x</div>",
        "<div style='float:left'>float</div><div style='text-box-trim:trim-both'>x</div>",
        "<div style='float:left'>float</div><div style='position:absolute'>x</div>",
        "<div style='float:left'>float</div><div style='columns:2'>x</div>",
    ] {
        let tree = crate::dom::parse(&format!("<div><div>{special}</div></div>"), "");
        assert!(
            pending(host(&tree), &BoxStyle::default(), tree.values()).is_none(),
            "{special}"
        );
    }
}
