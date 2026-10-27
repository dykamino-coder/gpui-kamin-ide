//! Неизменяемый рецепт копии фрагмента; построение запускает только нужный фрагмент.

use super::*;
use crate::layout::fragment::paginate::watch::Watched;
use std::rc::Rc;

pub(super) type Builder = Rc<dyn Fn(bool, usize, bool) -> gpui::AnyElement>;

#[allow(clippy::too_many_arguments)]
pub(super) fn new(
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
    kid_w: Option<f32>,
    frag_geom: Vec<(f32, f32)>,
    dec: Option<(f32, f32)>,
    merged: &BoxStyle,
    copy: &Element,
    h: f32,
    over: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: BoxStyle,
    whole: bool,
    watched: Rc<Watched>,
) -> Builder {
    let opts = opts.clone();
    // Under a native source of this multicol every continued absolute/fixed
    // root replays its own continuation record (paint::continuation).
    let slots = opts
        .cx
        .position
        .geometry
        .as_ref()
        .filter(|g| g.native_oof().is_some() && g.host == merged.self_node)
        .map(|g| g.continued_roots());
    let positioning = opts.cx.position.state.retain();
    let merged = merged.clone();
    let copy = copy.clone();
    std::rc::Rc::new(move |first: bool, part: usize, band: bool| {
        let mut opts = opts.in_stack_copy();
        opts.cx.position.continue_in_slots(slots.clone());
        // Копия строится после закрытия слоёв предков. Измерительные деревья
        // изолированы; только prepaint публикует OOF в исходный containing block.
        let (state, escaped) = positioning.isolate();
        opts.cx.position.state = state;
        // Копии фрагментов flex тоже наблюдаются: элемент рисует только
        // копия его фрагментов (`Watched::set_flex_owners`).
        opts.frag.watch = if first && !band {
            Some(watched.clone())
        } else if !band && watched.blocks().is_some_and(|b| b.has_flex()) {
            Some(watched.for_part(part))
        } else {
            None
        };
        escaped.wrap(build::build_copy(
            &opts,
            col_vert,
            col_rl,
            kid_w,
            &frag_geom,
            dec,
            &merged,
            &copy,
            h,
            over,
            nest_row,
            nest_phase_k,
            &inner,
            whole,
            first,
            part,
            band,
        ))
    })
}

#[cfg(test)]
mod tests {
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
    fn lazy_fragment_keeps_fixed_containing_block_after_layer_closes(
        cx: &mut gpui::TestAppContext,
    ) {
        let tree = crate::dom::parse(
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
            vec![],
            None,
            &merged,
            copy,
            20.,
            20.,
            None,
            0.,
            inner,
            false,
            watch.clone(),
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
                handles[0].measurement(window, cx).measure(
                    size(
                        gpui::AvailableSpace::Definite(px(50.)),
                        gpui::AvailableSpace::Definite(px(20.)),
                    ),
                    window,
                    cx,
                );
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
}

/// Построители разделяют DOM по COW, но у каждой копии собственные элементы и layout-id.
pub(super) fn fragments(
    build: Builder,
    copy: &Element,
    vertical: bool,
    count: usize,
) -> (
    gpui::AnyElement,
    Vec<gpui::AnyElement>,
    Vec<crate::layout::fragment::types::FragmentHandle>,
) {
    use crate::layout::fragment::types::FragmentHandle;
    let mut handles = Vec::with_capacity(count);
    let mut elements = Vec::with_capacity(count);
    for part in 0..count {
        let build = build.clone();
        let copy = copy.clone();
        let handle = FragmentHandle::new(move || {
            crate::layout::fragment::line_shape::side_margin_wrap(
                build(part == 0, part, false),
                &copy,
                vertical,
            )
        });
        elements.push(handle.element());
        handles.push(handle);
    }
    let el = elements.remove(0);
    (el, elements, handles)
}
