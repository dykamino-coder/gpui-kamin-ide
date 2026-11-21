//! Actual pixels of paint-only mutations must agree with a fresh document.

use crate::{
    BROWSER_CSS, Document, RenderOpts, host::capture::CaptureSession, metrics::FontContext,
};
use gpui::{
    AppContext, Application, AsyncApp, Bounds, Context, ElementPlace, IntoElement, Render,
    Subscription, Window, WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use gpui_windows::{GpuAdapterPreference, WindowsPlatform};
use std::{cell::Cell, rc::Rc, time::Duration};

type Result<T> = std::result::Result<T, String>;

struct View {
    doc: Document,
    step: Option<Subscription>,
}

fn opts() -> RenderOpts {
    let mut opts = RenderOpts {
        viewport: (400., 300.),
        text: gpui::TextStyle::default(),
        normal_line_height: 1.31,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.deferred.freeze_scrollbars = true;
    opts
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.doc.render_viewport(&opts())
    }
}

fn page(rule: &str, lit: bool) -> String {
    let class = if lit { "lit" } else { "" };
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff}}\
         body{{padding:20px}}.box{{width:80px;height:60px;border:2px solid #f00}}\
         {rule}</style></head><body class='{class}'><div class=box></div></body></html>"
    )
}

fn document(html: &str, fonts: &FontContext) -> Document {
    Document::builder(html, BROWSER_CSS)
        .fonts(fonts)
        .build()
        .expect("static border-paint fixture")
}

async fn frame(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    capture: &CaptureSession,
) -> Result<gpui::frame_capture::CapturedFrame> {
    let before = capture.sequence();
    window
        .update(cx, |_, window, _| {
            window.require_presentation();
            window.refresh();
        })
        .map_err(|e| e.to_string())?;
    capture
        .after(before)
        .await
        .map_err(|e| format!("capture closed: {e:?}"))?;
    capture
        .take_latest_after(before)
        .map(|f| f.into_pixels())
        .ok_or("missing captured pixels".into())
}

async fn compare(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    capture: &CaptureSession,
    fonts: &FontContext,
) -> Result<usize> {
    let mut compared = 0;
    for rule in [
        ".lit .box{border-color:#00f}",
        ".lit .box{border-color:#00f;background:#0f0}",
        ".lit .box{border-top-color:#0f0}",
        ".box{border-radius:6px}.lit .box{border-color:#00f;background:#0f0}",
    ] {
        window
            .update(cx, |view, _, _| {
                view.doc = document(&page(rule, false), fonts)
            })
            .map_err(|e| e.to_string())?;
        frame(cx, window, capture).await?;
        frame(cx, window, capture).await?;
        for lit in [true, false] {
            let html = page(rule, lit);
            let in_place = window
                .update(cx, |view, _, _| {
                    let values = view.doc.state.values.clone();
                    let edits = view.doc.mutations(&html).unwrap();
                    view.doc.apply(&edits, BROWSER_CSS).unwrap();
                    Rc::ptr_eq(&values, &view.doc.state.values)
                })
                .map_err(|e| e.to_string())?;
            if !in_place && !rule.contains("border-radius") && !rule.contains("border-top-color") {
                return Err(format!("fixture rebuilt the document: {rule}, lit={lit}"));
            }
            let retained = frame(cx, window, capture).await?;
            window
                .update(cx, |view, _, _| view.doc = document(&html, fonts))
                .map_err(|e| e.to_string())?;
            let fresh = frame(cx, window, capture).await?;
            if fresh.bgra != frame(cx, window, capture).await?.bgra {
                return Err("two fresh border frames differ".into());
            }
            if (retained.width, retained.height) != (fresh.width, fresh.height) {
                return Err("capture dimensions changed".into());
            }
            let ink = fresh
                .bgra
                .chunks_exact(4)
                .filter(|p| p[..3] != [255; 3])
                .count();
            if ink < 100 {
                return Err("fresh frame has no visible border".into());
            }
            let border = if !lit {
                [0, 0, 255]
            } else if rule.contains("border-top-color") {
                [0, 255, 0]
            } else {
                [255, 0, 0]
            };
            if !fresh.bgra.chunks_exact(4).any(|p| p[..3] == border) {
                return Err(format!(
                    "fresh frame lacks the expected border colour: {rule}, lit={lit}"
                ));
            }
            let different = retained
                .bgra
                .chunks_exact(4)
                .zip(fresh.bgra.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            if different != 0 {
                return Err(format!(
                    "{different} painted pixels differ from fresh: {rule}, lit={lit}"
                ));
            }
            compared += 1;
        }
    }
    Ok(compared)
}

#[test]
fn retained_border_and_background_pixels_match_fresh() {
    let result = Rc::new(Cell::new(None));
    let done = result.clone();
    let platform =
        Rc::new(WindowsPlatform::with_gpu_adapter(false, GpuAdapterPreference::Software).unwrap());
    platform.set_offscreen_windows(true);
    Application::with_platform(platform).run(move |cx| {
        let fonts = FontContext::new(cx.text_system().clone());
        let doc = document(&page("", false), &fonts);
        let (capture, sink) = CaptureSession::open();
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(400.), px(300.)),
                    })),
                    focus: false,
                    ..Default::default()
                },
                |window, cx| {
                    window.set_frame_capture(Some(sink)).unwrap();
                    let view = cx.new(|_| View { doc, step: None });
                    let parent = ElementPlace::of_view(view.entity_id());
                    let observed = view.downgrade();
                    let step = window.on_update_rendering(move |window, cx| {
                        observed
                            .update(cx, |view, cx| {
                                view.doc.set_viewport((400., 300.)).unwrap();
                                view.doc
                                    .update_rendering(&opts(), &parent, window, cx, |_| {});
                            })
                            .unwrap();
                    });
                    view.update(cx, |view, _| view.step = Some(step));
                    view
                },
            )
            .unwrap();
        cx.spawn(async move |cx| {
            let timeout = cx.background_executor().timer(Duration::from_secs(45));
            let checked = match futures::future::select(
                Box::pin(compare(cx, window, &capture, &fonts)),
                timeout,
            )
            .await
            {
                futures::future::Either::Left((checked, _)) => checked,
                futures::future::Either::Right(_) => Err("border differential timed out".into()),
            };
            done.set(Some(checked));
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    let outcome = result.take().expect("the border check did not finish");
    assert_eq!(outcome.expect("retained/fresh border pixels must match"), 8);
}
