//! Изолированные пробы глубины: запускать по одной, переполнение завершает процесс.
#![allow(clippy::print_stderr)]

use crate::dom::{Element, Node};
use crate::style::computed::{BoxStyle, Computed};
use crate::{Document, RenderOpts};
use gpui::{IntoElement, ParentElement};

mod regression;
mod special;

// Только изолированная тестовая проба; production engine не читает окружение.
#[allow(clippy::disallowed_methods)]
fn depth() -> usize {
    std::env::var("DEPTH_PROBE_N").unwrap().parse().unwrap()
}

#[allow(clippy::disallowed_methods)]
fn stage() -> String {
    std::env::var("DEPTH_PROBE_STAGE").unwrap()
}

fn markup(n: usize) -> String {
    format!(
        "<!doctype html>{}leaf{}",
        "<div>".repeat(n),
        "</div>".repeat(n)
    )
}

fn options() -> RenderOpts {
    RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800., 600.),
        normal_line_height: 1.31,
        cx: Default::default(),
        frag: Default::default(),
    }
}

fn tree(n: usize) -> Vec<Node> {
    let mut nodes = vec![Node::Text("leaf".into())];
    let style: crate::dom::SharedStyle = Computed::default().into();
    for _ in 0..n {
        let mut e = crate::layout::table::anon::anon_element("div", nodes);
        e.style = style.clone();
        nodes = vec![Node::Element(e)];
    }
    nodes
}

#[test]
#[ignore = "изолированный замер, DEPTH_PROBE_N и DEPTH_PROBE_STAGE обязательны"]
fn depth_probe() {
    let n = depth();
    let stage = stage();
    eprintln!(
        "sizes Computed={} BoxStyle={} Element={} Node={} RenderOpts={} LayoutCx={}",
        size_of::<Computed>(),
        size_of::<BoxStyle>(),
        size_of::<Element>(),
        size_of::<Node>(),
        size_of::<RenderOpts>(),
        size_of::<crate::layout::cx::LayoutCx>()
    );
    if stage == "parse" {
        let source = crate::dom::input::Admitted::parse(&markup(n), false).unwrap();
        eprintln!("finished parse {n}");
        drop(source);
    } else if stage == "compute" || stage == "build-direct" {
        let nodes = tree(n);
        let opts = options();
        if stage == "compute" {
            let root = opts.root_style();
            crate::style::cascade::materialize::materialize(&nodes, root.style_rc(), opts.cx.vcx());
            eprintln!("finished compute {n}");
        } else {
            let boxes = crate::render::render(&nodes, &opts);
            eprintln!("finished build {n}");
            drop(boxes);
        }
        drop(nodes);
    } else {
        let document = Document::builder(&markup(n), "").build().unwrap();
        eprintln!("finished cascade {n}");
        if stage != "cascade" {
            let boxes = document.render(&options());
            eprintln!("finished build {n}");
            drop(boxes);
        }
    }
}

#[gpui::test]
#[ignore = "изолированный замер настоящих фаз GPUI"]
fn depth_frame_probe(cx: &mut gpui::TestAppContext) {
    let n = depth();
    let stage = stage();
    eprintln!(
        "sizes Style={} DivLayout={} DivPrepaint={} DrawableDiv={}",
        size_of::<gpui::Style>(),
        size_of::<<gpui::Div as gpui::Element>::RequestLayoutState>(),
        size_of::<<gpui::Div as gpui::Element>::PrepaintState>(),
        size_of::<gpui::Drawable<gpui::Div>>(),
    );
    let document;
    let boxes = if stage.ends_with("-direct") {
        let mut inner = gpui::div().into_any_element();
        for _ in 0..n {
            inner = gpui::div().child(inner).into_any_element();
        }
        vec![inner]
    } else if stage.ends_with("-boxes") {
        let boxes = crate::render::render(&tree(n), &options());
        eprintln!("finished build {n}");
        boxes
    } else {
        document = Document::builder(&markup(n), "").build().unwrap();
        eprintln!("finished cascade {n}");
        let boxes = document.render(&options());
        eprintln!("finished build {n}");
        boxes
    };
    cx.add_empty_window().draw(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(100.), gpui::px(100.)),
        |_, _| {
            FrameProbe(
                gpui::div().children(boxes).into_any_element(),
                stage.replace("-direct", "").replace("-boxes", ""),
            )
        },
    );
    eprintln!("finished frame {n}");
}

struct FrameProbe(gpui::AnyElement, String);

impl IntoElement for FrameProbe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for FrameProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<gpui::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        let id = self.0.request_layout(window, cx);
        eprintln!("finished request_layout");
        let id = if self.1 == "request_layout" {
            window.request_layout(gpui::Style::default(), [], cx)
        } else {
            id
        };
        (id, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        eprintln!("finished layout");
        if !matches!(self.1.as_str(), "layout" | "request_layout") {
            self.0.prepaint(window, cx);
            eprintln!("finished prepaint");
        }
    }
    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        if self.1 == "paint" {
            self.0.paint(window, cx);
            eprintln!("finished paint");
        }
    }
}
