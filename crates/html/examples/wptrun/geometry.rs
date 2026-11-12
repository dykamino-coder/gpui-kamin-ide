//! Детерминированный снимок CSSOM без отрисовки или захвата кадра.

use gpui::{AppContext, Bounds, Entity, WindowBounds, WindowOptions, point, px, size};
use kamin_html::{BROWSER_CSS, Document, RenderOpts};
use kamin_html::{ElementGeometry, LineHeight, Rect};
use serde_json::{Value, json};
use std::{path::PathBuf, rc::Rc};

pub(super) fn run(
    host: super::gpu::Host,
    args: &[String],
    mut config: gpui::InstrumentationConfig,
) {
    let parse = || -> Result<_, Box<dyn std::error::Error>> {
        if args.len() != 6 {
            return Err("usage: wptrun --geometry input output width height".into());
        }
        let input = std::fs::canonicalize(&args[2])?;
        let output = PathBuf::from(&args[3]);
        let width: u32 = args[4].parse()?;
        let height: u32 = args[5].parse()?;
        if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
            return Err("viewport must be in 1..=8192 CSS px".into());
        }
        let html = std::fs::read_to_string(&input)?;
        Ok((input, output, width, height, html))
    };
    let (input, output, width, height, html) = parse().unwrap_or_else(|e| super::audit::fatal(e));
    config.scale_factor = Some(1.);
    host.application()
        .with_instrumentation(config)
        .run(move |cx| {
            super::fonts::register(cx);
            let fonts = kamin_html::metrics::FontContext::new(cx.text_system().clone());
            let doc = Rc::new(
                Document::builder(&html, BROWSER_CSS)
                    .path(&input)
                    .fonts(&fonts)
                    .build()
                    .unwrap_or_else(|e| super::audit::fatal(format!("document: {e:?}"))),
            );
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(0.), px(0.)),
                            size(px(width as f32), px(height as f32)),
                        ))),
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    |window, cx| -> Entity<super::page::Page> {
                        let mut text = window.text_style();
                        text.font_family = "Times New Roman".into();
                        text.font_size = px(16.).into();
                        let opts = RenderOpts {
                            text,
                            viewport: (width as f32, height as f32),
                            normal_line_height: 1.31,
                            cx: Default::default(),
                            frag: Default::default(),
                        };
                        let elements = doc
                            .element_geometry(&opts, window, cx)
                            .unwrap_or_else(|e| super::audit::fatal(format!("geometry: {e:?}")));
                        let value = json!({ "schema_version": 1, "viewport": {
                "width": width, "height": height, "device_scale_factor": 1 },
                "elements": elements.iter().map(element).collect::<Vec<_>>() });
                        let mut bytes =
                            serde_json::to_vec_pretty(&value).expect("finite geometry JSON");
                        bytes.push(b'\n');
                        std::fs::write(&output, bytes).unwrap_or_else(|e| super::audit::fatal(e));
                        cx.new(|_| super::page::Page {
                            doc,
                            select: None,
                            #[cfg(feature = "wpt-harness")]
                            probes: Default::default(),
                        })
                    },
                )
                .unwrap_or_else(|e| super::audit::fatal(e));
            let _ = window;
            cx.quit();
        });
}

fn rect(r: Rect) -> Value {
    json!({"x": r.x, "y": r.y, "width": r.width, "height": r.height})
}

fn element(e: &ElementGeometry) -> Value {
    let line = match e.line_height {
        LineHeight::Normal => json!("normal"),
        LineHeight::Pixels(v) => json!(v),
    };
    json!({"path": e.path, "tag": e.tag, "id": e.id, "class": e.class,
        "border_box": rect(e.border_box), "client_rects": e.client_rects.iter().copied().map(rect).collect::<Vec<_>>(),
        "padding_boxes": e.padding_boxes.iter().copied().map(rect).collect::<Vec<_>>(),
        "content_boxes": e.content_boxes.iter().copied().map(rect).collect::<Vec<_>>(),
        "font_size": e.font_size, "line_height": line})
}
