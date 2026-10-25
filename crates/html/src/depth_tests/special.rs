//! Изолированные пробы специальных построителей и фаз кадра на глубоком DOM.

use super::*;

fn source(n: usize, kind: &str) -> String {
    let (open, close, leaf, count) = match kind {
        "inline" => ("<span>", "</span>", "leaf", n),
        "float" => ("<div style='float:left'>", "</div>", "leaf", n),
        "multicol" => ("<div style='column-count:2'>", "</div>", "leaf", n),
        "grid" => ("<div style='display:grid'>", "</div>", "leaf", n),
        "math" => ("<mrow>", "</mrow>", "<mi>x</mi>", n),
        "table" => (
            "<table><tbody><tr><td>",
            "</td></tr></tbody></table>",
            "leaf",
            n.div_ceil(4),
        ),
        _ => panic!("unknown special path {kind}"),
    };
    let body = format!("{}{}{}", open.repeat(count), leaf, close.repeat(count));
    if kind == "math" {
        format!("<!doctype html><math>{body}</math>")
    } else {
        format!("<!doctype html>{body}")
    }
}

fn run(n: usize, kind: &str, stage: &str, cx: &mut gpui::TestAppContext) {
    eprintln!("special {kind} {stage} depth {n}");
    let document = Document::builder(&source(n, kind), "").build().unwrap();
    if n <= 1026 {
        assert!(document.traversal_limit().is_none());
    }
    eprintln!("finished cascade");
    let boxes = document.render(&options());
    eprintln!("finished build");
    if stage != "build" {
        cx.add_empty_window().draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(100.), gpui::px(100.)),
            |_, _| FrameProbe(gpui::div().children(boxes).into_any_element(), stage.into()),
        );
    }
    if n <= 1026 {
        assert!(document.traversal_limit().is_none());
    }
}

#[test]
#[ignore = "изолированный замер специальных путей, DEPTH_PROBE_KIND обязателен"]
#[allow(clippy::disallowed_methods)]
fn special_path_probe() {
    let mut cx = gpui::TestAppContext::single();
    let kind = std::env::var("DEPTH_PROBE_KIND").unwrap();
    run(depth(), &kind, &stage(), &mut cx);
}

#[test]
fn inline_grid_math_table_and_multicol_paint_at_browser_depth_on_eight_mib() {
    regression::on_eight_mib(|| {
        for kind in ["inline", "grid", "math", "table", "multicol"] {
            let mut cx = gpui::TestAppContext::single();
            run(1026, kind, "paint", &mut cx);
        }
    });
}

#[test]
fn float_paint_at_browser_depth_on_eight_mib() {
    regression::on_eight_mib(|| {
        let mut cx = gpui::TestAppContext::single();
        run(1026, "float", "paint", &mut cx);
    });
}
