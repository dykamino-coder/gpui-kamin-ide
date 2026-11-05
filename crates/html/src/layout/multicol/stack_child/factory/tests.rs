//! Регрессии независимых копий фрагментов.
use super::*;
use crate::dom::Node;
use crate::layout::positioned::layer_scope::LayerScope;
use gpui::{IntoElement, ParentElement, Styled, point, px, size};

fn find<'a>(nodes: &'a [Node], tag: &str) -> &'a Element {
    for node in nodes {
        if let Node::Element(e) = node {
            if e.tag == tag {
                return e;
            }
            if e.children.iter().any(|n| matches!(n, Node::Element(_))) {
                return find(&e.children, tag);
            }
        }
    }
    panic!("missing {tag}");
}

#[gpui::test]
fn lazy_fragment_keeps_fixed_containing_block_after_layer_closes(cx: &mut gpui::TestAppContext) {
    let tree = crate::document::parsed::parse(
        "<section style='transform:translateX(0)'><article style='position:relative;width:50px;height:20px'><aside style='position:absolute'><footer style='position:fixed;top:0;left:0;width:100%;height:100%'></footer></aside></article></section>",
        "",
    );
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    crate::style::cascade::materialize::materialize(
        &tree,
        opts.root_style().style_rc(),
        opts.cx.vcx(),
    );
    let merged = box_style(
        &opts.root_style(),
        &find(&tree, "section").style,
        opts.cx.vcx(),
    );
    let copy = find(&tree, "article");
    let fixed = find(&tree, "footer").node_id;
    let inner = box_style(&merged, &copy.style, opts.cx.vcx());
    let watch = Watched::new(&[fixed]);
    let icb = LayerScope::icb(&opts.cx.position.state);
    let layer = LayerScope::fixed_only(&opts.cx.position.state);
    let build = new(
        &opts,
        false,
        false,
        Some(50.),
        Default::default(),
        &merged,
        copy,
        20.,
        20.,
        None,
        0.,
        inner,
        false,
        watch.clone(),
        Default::default(),
    );
    let (el, _, handles) = fragments(build, copy, false, 1);
    let fixed_host = layer.finish();
    let icb_host = icb.finish();
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(120.), px(80.)),
        |window, cx| {
            // Проба предшествует выбранной копии и не должна публиковать OOF.
            reblessive::Stack::new()
                .enter(|stack| async {
                    handles[0]
                        .measurement_async(window, cx, stack)
                        .await
                        .measure_async(
                            size(
                                gpui::AvailableSpace::Definite(px(50.)),
                                gpui::AvailableSpace::Definite(px(20.)),
                            ),
                            window,
                            cx,
                            stack,
                        )
                        .await;
                })
                .finish();
            let mut root = gpui::div()
                .w(px(120.))
                .h(px(80.))
                .child(el)
                .children(fixed_host)
                .children(icb_host)
                .into_any_element();
            root.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(120.)),
                    gpui::AvailableSpace::Definite(px(80.)),
                ),
                window,
                cx,
            );
            root.prepaint_at(point(px(0.), px(0.)), window, cx);
            let bounds = watch
                .normal(fixed, window)
                .expect("fixed descendant laid out during prepaint");
            assert_eq!(bounds.size, size(px(120.), px(80.)));
            gpui::div()
        },
    );
}
