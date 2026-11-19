//! Real DirectWrite glyph pixels must agree after a retained record moves.

use crate::{
    BROWSER_CSS, Document, RenderOpts, host::capture::CaptureSession, metrics::FontContext,
};
use gpui::{
    AppContext, Application, AsyncApp, Bounds, Context, ElementPlace, IntoElement, Render,
    Subscription, TextRenderingMode, Window, WindowBounds, WindowHandle, WindowOptions, point, px,
    size,
};
use gpui_windows::{GpuAdapterPreference, WindowsPlatform};
use std::{borrow::Cow, cell::Cell, rc::Rc, time::Duration};

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

fn page(padding: f32) -> String {
    let rows: String = (0..160)
        .map(|i| {
            let padding = if i == 3 { padding } else { 0. };
            format!(
                "<div style='padding-top:{padding}px'>row {i} affine glyphs 135 <b>MW</b></div>"
            )
        })
        .collect();
    format!(
        "<!doctype html><style>html,body{{margin:0;background:#fff;color:#000;\
         font:17px/23px 'JetBrains Mono'}}div{{height:23px}}</style>{rows}"
    )
}

fn document(html: &str, fonts: &FontContext) -> Document {
    Document::builder(html, BROWSER_CSS)
        .fonts(fonts)
        .build()
        .expect("static real-font fixture")
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
) -> Result<()> {
    for padding in [0., 4., 0., 4., 0.5, 0., 1.5, 0.] {
        let html = page(padding);
        window
            .update(cx, |view, _, _| {
                let edits = view
                    .doc
                    .mutations(&html)
                    .expect("padding mutation preserves DOM shape");
                view.doc.apply(&edits, "").unwrap();
            })
            .map_err(|e| e.to_string())?;
        frame(cx, window, capture).await?;
    }
    for padding in [4., 0.5, 1.5, 0.] {
        let before_replays = window
            .read_with(cx, |view, _| view.doc.replayed_blocks())
            .map_err(|e| e.to_string())?;
        let html = page(padding);
        window
            .update(cx, |view, _, _| {
                let edits = view.doc.mutations(&html).unwrap();
                view.doc.apply(&edits, "").unwrap();
            })
            .map_err(|e| e.to_string())?;
        let incremental = frame(cx, window, capture).await?;
        let replayed = window
            .read_with(cx, |view, _| view.doc.replayed_blocks())
            .map_err(|e| e.to_string())?;
        if padding == 4. && replayed == before_replays {
            return Err("the fixture did not exercise retained flow records".into());
        }
        window
            .update(cx, |view, _, _| view.doc = document(&html, fonts))
            .map_err(|e| e.to_string())?;
        let fresh = frame(cx, window, capture).await?;
        if fresh.bgra != frame(cx, window, capture).await?.bgra {
            return Err("two fresh real-font frames differ".into());
        }
        if incremental.width != fresh.width || incremental.height != fresh.height {
            return Err("capture dimensions changed".into());
        }
        let differing = incremental
            .bgra
            .chunks_exact(4)
            .zip(fresh.bgra.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        if differing != 0 {
            return Err(format!(
                "padding {padding}: {differing} real-glyph pixels differ from fresh"
            ));
        }
        let ink = fresh
            .bgra
            .chunks_exact(4)
            .filter(|p| p[..3] != [255; 3])
            .count();
        if ink < 100 {
            return Err("the real font produced no visible text".into());
        }
        // Establish records again before the next mutation.
        for padding in [padding + 4., padding, padding + 4., padding] {
            let html = page(padding);
            window
                .update(cx, |view, _, _| {
                    let edits = view.doc.mutations(&html).unwrap();
                    view.doc.apply(&edits, "").unwrap();
                })
                .map_err(|e| e.to_string())?;
            frame(cx, window, capture).await?;
        }
    }
    Ok(())
}

fn real_font_pixels(scale: f32) {
    let result = Rc::new(Cell::new(None));
    let done = result.clone();
    let platform =
        Rc::new(WindowsPlatform::with_gpu_adapter(false, GpuAdapterPreference::Software).unwrap());
    platform.set_offscreen_windows(true);
    Application::with_platform(platform)
        .with_instrumentation(gpui::InstrumentationConfig {
            scale_factor: Some(scale),
            ..Default::default()
        })
        .run(move |cx| {
            cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(include_bytes!(
                    "../../../../shell/assets/fonts/JetBrainsMono-Variable.ttf"
                ))])
                .unwrap();
            let fonts = FontContext::new(cx.text_system().clone());
            let doc = document(&page(0.), &fonts);
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
                    futures::future::Either::Right(_) => {
                        Err("real-font differential timed out".into())
                    }
                };
                done.set(Some(checked));
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    let outcome = result.take().expect("the real-font check did not finish");
    assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
}

#[test]
fn moved_real_font_pixels_match_fresh_at_fractional_offsets() {
    real_font_pixels(1.);
    real_font_pixels(1.25);
}
