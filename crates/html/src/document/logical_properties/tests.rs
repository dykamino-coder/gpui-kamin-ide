//! Ориентация картинок масок сохраняется при подготовке документа.
use crate::document::Document;
use crate::dom::Node;

#[test]
fn absent_logical_properties_preserve_publication_and_variant_groups() {
    use crate::dom::{Element, Marks};
    use crate::style::cascade::materialize::materialize;
    let doc = crate::values::DocumentValues::detached(Default::default());
    let seed = doc.initial_style();
    let mut element = Element::synthetic("div", seed.clone(), Marks::default(), Vec::new());
    element.hover = Some(seed.clone().into());
    element.first_letter = Some(seed.clone().into());
    element.first_line = Some(seed.clone().into());
    let nodes = vec![Node::Element(element)];
    materialize(&nodes, &std::rc::Rc::new(seed.clone()), doc.values());
    let Node::Element(element) = &nodes[0] else {
        panic!("element")
    };
    let published = element.style.published().unwrap();
    let nodes = super::resolve_logical(nodes, doc.values());
    let Node::Element(element) = &nodes[0] else {
        panic!("element")
    };
    assert!(published.ptr_eq(&element.style.published().unwrap()));
    for style in [
        Some(&element.style),
        element.hover.as_ref(),
        element.first_letter.as_ref(),
        element.first_line.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(seed.reset_context.ptr_eq(&style.cascaded().reset_context));
        assert_eq!(seed, *style.cascaded());
    }
}

#[test]
fn mask_orientation_inherits_through_unwrapped_ancestors_and_resets() {
    let doc = Document::builder(
        r#"<style>
    html { image-orientation: none }
    .mask { mask-image: url(unused.jpg) }
    .reset { image-orientation: from-image }
    </style><body style="margin:0">
    <div class="mask"></div>
    <section class="reset"><div class="mask"></div></section>
    <section><div class="mask" style="image-orientation:from-image"></div></section>
    </body>"#,
        "",
    )
    .build()
    .unwrap();
    fn collect(nodes: &[Node], flags: &mut Vec<Option<bool>>) {
        for node in nodes {
            if let Node::Element(e) = node {
                if e.style.cascaded().reset_mask.mask_image.is_some() {
                    flags.push(e.style.cascaded().inh.ui.image_orient_none);
                }
                collect(&e.children, flags);
            }
        }
    }
    let mut flags = Vec::new();
    collect(doc.nodes(), &mut flags);
    assert_eq!(flags, [Some(true), Some(false), Some(false)]);
}

#[test]
fn logical_sides_map_by_the_inherited_mode_without_touching_own_styles() {
    use crate::style::values::value::Len;
    let doc = Document::builder(
        r#"<body><div style="writing-mode: sideways-lr; direction: rtl">
        <p id="a" style="margin-inline-start: 15px; padding-block-start: 5px"></p>
        </div></body>"#,
        "",
    )
    .build()
    .unwrap();
    let p = crate::dom::test_support::find(doc.nodes(), &|e| e.attr("id") == Some("a")).expect("p");
    // sideways-lr + rtl: inline-start сверху, block-start слева (§6.4).
    assert_eq!(
        p.style.cascaded().reset_box_layout.margin.top,
        Some(Len::Px(15.0))
    );
    assert_eq!(
        p.style.cascaded().reset_box_layout.padding.left,
        Some(Len::Px(5.0))
    );
    assert_eq!(
        (
            p.style.cascaded().inh.writing.writing_mode,
            p.style.cascaded().inh.writing.rtl
        ),
        (None, None)
    );
}

#[test]
fn deep_preparation_restores_writing_and_orientation_for_siblings() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            use crate::style::values::value::Len;
            let source = format!(
                "<div style='writing-mode:vertical-rl;image-orientation:none'>{}\
                 <p id='deep' style='margin-inline-start:7px;mask-image:url(unused.jpg)'>leaf</p>{}\
                 <p id='tail' style='margin-inline-start:7px;mask-image:url(unused.jpg)'></p></div>\
                 <p id='after' style='margin-inline-start:7px;mask-image:url(unused.jpg)'></p>",
                "<div>".repeat(20_000),
                "</div>".repeat(20_000)
            );
            let doc = Document::builder(&source, "").build().unwrap();
            for id in ["deep", "tail", "after"] {
                let element = crate::dom::traversal::read_nodes(doc.nodes())
                    .find_map(|node| match node {
                        Node::Element(e) if e.attr("id") == Some(id) => Some(e),
                        _ => None,
                    })
                    .unwrap();
                let style = element.style.cascaded();
                if id == "after" {
                    assert_eq!(style.reset_box_layout.margin.left, Some(Len::Px(7.)));
                    assert_ne!(style.inh.ui.image_orient_none, Some(true));
                } else {
                    assert_eq!(style.reset_box_layout.margin.top, Some(Len::Px(7.)));
                    assert_eq!(style.inh.ui.image_orient_none, Some(true));
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
