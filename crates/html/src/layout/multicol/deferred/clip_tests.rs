//! Обрезанное OOF-содержимое не порождает продолжения за clip box.

use super::*;
use crate::layout::fragment::plan::Completion;
use gpui::{point, px, size};

fn measured_stack(
    content: &str,
    count: u16,
    height: f32,
    window: &mut gpui::Window,
    app: &mut gpui::App,
) -> ColumnStack {
    let tree = crate::dom::parse(
        &format!(
            "<section style='column-count:{count};column-gap:0;column-fill:auto;width:{}px;height:{height}px'>{content}</section>",
            count as f32 * 100.
        ),
        "",
    );
    let mut opts = RenderOpts {
        viewport: (400., 400.),
        normal_line_height: 1.2,
        text: Default::default(),
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
    let host = super::tests::section(&tree);
    let merged = crate::style::cascade::materialize::box_style(
        &opts.root_style(),
        &host.style,
        opts.cx.vcx(),
    );
    let measured = crate::layout::multicol::measure::stack_plan(
        host,
        &merged,
        &opts.root_style(),
        &opts,
        count,
        0.,
        false,
        false,
        Some(height),
        Some(height),
        None,
        None,
        None,
    );
    let kids = measured.stackable.unwrap();
    let source: Vec<_> = kids.iter().map(|(e, _)| e.clone()).collect();
    let geometry = crate::layout::positioned::containing_block::metrics::layout::collect(
        &source, &merged, 100., &opts, window, app,
    );
    opts.cx.position.geometry = Some(geometry.clone());
    let (parents, _, parallel, kids) = crate::layout::multicol::container::kids::column_kids(
        &merged,
        kids,
        count,
        0.,
        false,
        None,
        count as usize,
        Some(height),
        (Some(100.), Some(100.)),
        &opts,
    );
    let prepared = kids
        .iter()
        .enumerate()
        .map(|(i, (e, shape))| {
            crate::layout::multicol::stack_child::prepare(
                i,
                e,
                shape,
                parents[i].as_ref().unwrap_or(&merged),
                &opts,
                &parallel,
                false,
                false,
                Some(100.),
                Some(height),
                &measured.measured_kids,
            )
        })
        .collect();
    let children = crate::layout::multicol::container::kids::stack_children(
        host,
        &merged,
        &opts,
        count,
        false,
        false,
        (Some(100.), Some(100.)),
        None,
        measured.nested_auto,
        measured.nested_whole,
        measured.measured_kids,
        count as usize,
        Some(height),
        parents,
        parallel,
        kids,
        Vec::new(),
        prepared,
    );
    let mut stack = ColumnStack::new(
        children,
        count as usize,
        0.,
        Some(height),
        None,
        None,
        None,
        Default::default(),
    )
    .with_oof(Some(geometry), merged.wm());
    stack.col_w.set(100.);
    for i in 0..stack.children.len() {
        stack.lay_out_main(i, window, app);
    }
    stack
}

fn clipped_continuations(cx: &mut gpui::TestAppContext, position: &str, containing_block: &str) {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(400.), px(400.)), |window, app| {
        let mut stack = measured_stack(&format!("<div style='overflow:clip'><div style='{containing_block};height:300px'><div style='position:{position};width:100%;height:100000px'></div></div></div>"), 3, 100., window, app);
        assert_eq!(stack.children.len(), 1);
        assert_eq!((stack.children[0].h, stack.children[0].over), (300., 300.));
        stack.regrow_children(window, app);
        let (_, _, plan, _) = stack.balance(&stack.kid_geoms());
        assert_eq!(plan.failure, None);
        assert_eq!(plan.completion, Completion::Complete);
        assert_eq!(plan.iter().map(|f| (f.col, f.from, f.h)).collect::<Vec<_>>(), [(0, 0., 100.), (1, 100., 100.), (2, 200., 100.)]);
        gpui::div()
    });
}

#[gpui::test]
fn oof_107_absolute_overflow_stops_at_the_measured_clip_box(cx: &mut gpui::TestAppContext) {
    clipped_continuations(cx, "absolute", "position:relative");
}

#[gpui::test]
fn oof_110_fixed_overflow_stops_at_the_measured_clip_box(cx: &mut gpui::TestAppContext) {
    clipped_continuations(cx, "fixed", "will-change:transform");
}
