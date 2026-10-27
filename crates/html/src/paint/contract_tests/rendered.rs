//! Пробы готового документа: однократная краска реального HTML и видимые сплошные квады
//! в порядке краски (граница ∩ маска, после переноса). Для unit-тестов потребителей.

use gpui::{
    AnyElement, AvailableSpace, Bounds, Context, Hsla, IntoElement, ParentElement, Pixels, Render,
    ScaledPixels, Scene, Styled, TestAppContext, Window, canvas, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

/// Visible quads with the paint group holding each (0: no group).
type Quads = Rc<RefCell<Vec<(Bounds<Pixels>, Hsla, usize)>>>;

/// The document's frame inside a real view (element state needs a current view);
/// every redraw builds the document element again.
struct Frame {
    build: Rc<dyn Fn() -> AnyElement>,
    sink: Quads,
    size: (f32, f32),
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (build, sink, (w, h)) = (self.build.clone(), self.sink.clone(), self.size);
        canvas(
            move |_, window, cx| {
                let mut root = build();
                root.layout_as_root(
                    size(
                        AvailableSpace::Definite(px(w)),
                        AvailableSpace::Definite(px(h)),
                    ),
                    window,
                    cx,
                );
                root.prepaint_at(point(px(0.), px(0.)), window, cx);
                root
            },
            move |_, mut root, window, cx| {
                let extent = Bounds::new(point(px(0.), px(0.)), size(px(w), px(h)));
                let (_, record) = window
                    .record_paint(extent, cx, |window, cx| root.paint(window, cx))
                    .expect("document paint");
                let mut sink = sink.borrow_mut();
                sink.clear();
                collect(record.scene(), window.scale_factor(), &mut sink);
                // The frame shows what was recorded.
                window
                    .paint_plan(
                        &gpui::PaintPlan::record(record),
                        gpui::PaintPlacement {
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                            clip: extent,
                        },
                    )
                    .expect("document replay");
            },
        )
        .size_full()
    }
}

/// Visible solid quads of `html` laid out in a `w × h` viewport, in paint order.
pub(crate) fn quads(html: &str, w: f32, h: f32) -> Vec<(Bounds<Pixels>, Hsla)> {
    plain(open(html, false, w, h, |_, sink| sink.take()))
}

/// Visible quads with their paint group (one per effect instance; 0: none).
pub(crate) fn grouped(html: &str, w: f32, h: f32) -> Vec<(Bounds<Pixels>, Hsla, usize)> {
    open(html, false, w, h, |_, sink| sink.take())
}

fn plain(found: Vec<(Bounds<Pixels>, Hsla, usize)>) -> Vec<(Bounds<Pixels>, Hsla)> {
    found.into_iter().map(|(b, c, _)| (b, c)).collect()
}

/// The same for a print document on the default WPT page box (5in × 3in sheet,
/// 0.5in margins): sheets laid out by the page stack in a `w × h` window.
pub(crate) fn printed(html: &str, w: f32, h: f32) -> Vec<(Bounds<Pixels>, Hsla)> {
    plain(open(html, true, w, h, |_, sink| sink.take()))
}

/// The cursor chosen by the frame's hit test at each window point.
pub(crate) fn cursors(
    html: &str,
    print: bool,
    w: f32,
    h: f32,
    points: &[(f32, f32)],
) -> Vec<gpui::CursorStyle> {
    open(html, print, w, h, |vcx, _| {
        points
            .iter()
            .map(|&(x, y)| {
                vcx.simulate_mouse_move(point(px(x), px(y)), None, Default::default());
                vcx.update(|window, _| window.hit_test_cursor_style())
            })
            .collect()
    })
}

fn open<R>(
    html: &str,
    print: bool,
    w: f32,
    h: f32,
    inspect: impl FnOnce(&mut gpui::VisualTestContext, &Quads) -> R,
) -> R {
    let document = Rc::new(
        crate::Document::builder(html, crate::BROWSER_CSS)
            .print(print)
            .build()
            .expect("test document"),
    );
    let (sheet, margin) = ((480., 288.), 48.);
    let area = (sheet.0 - 2. * margin, sheet.1 - 2. * margin);
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: if print { area } else { (w, h) },
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let build: Rc<dyn Fn() -> AnyElement> = Rc::new(move || {
        if print {
            let geom_for: crate::layout::page::page_stack::PageGeomFn =
                Rc::new(
                    move |_, _: &str| crate::layout::page::page_stack::PageGeom {
                        size: sheet,
                        margin: [margin; 4],
                        border: (0., gpui::black()),
                        outline: (0., 0., gpui::black()),
                        padding: [0.; 4],
                        bg: gpui::white(),
                        canvas: None,
                        area,
                        turn: 0,
                    },
                );
            let stack = crate::layout::page::paged::render_paged_select(
                document.nodes(),
                &document.frame_opts(&options),
                geom_for,
                None,
                None,
            );
            gpui::div()
                .w(px(w))
                .h(px(h))
                .child(document.scoped(stack))
                .into_any_element()
        } else {
            gpui::div()
                .children(document.render(&options))
                .into_any_element()
        }
    });
    let sink: Quads = Rc::default();
    let mut cx = TestAppContext::single();
    let (_, vcx) = cx.add_window_view(|_, _| Frame {
        build,
        sink: sink.clone(),
        size: (w, h),
    });
    vcx.update(|window, app| {
        window.refresh();
        window.draw(app).clear(app);
    });
    inspect(vcx, &sink)
}

/// Device bounds of `q` under `m` (four corners).
fn placed(q: Bounds<ScaledPixels>, m: gpui::TransformationMatrix) -> Bounds<ScaledPixels> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in [
        (q.origin.x.0, q.origin.y.0),
        (q.right().0, q.origin.y.0),
        (q.origin.x.0, q.bottom().0),
        (q.right().0, q.bottom().0),
    ] {
        let p = m.apply(point(px(x), px(y)));
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    Bounds::new(
        point(ScaledPixels(x0), ScaledPixels(y0)),
        size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
    )
}

fn collect(scene: &Scene, scale: f32, out: &mut Vec<(Bounds<Pixels>, Hsla, usize)>) {
    let mut pending = vec![(scene, 0)];
    let mut groups = 0;
    while let Some((scene, group)) = pending.pop() {
        for quad in &scene.quads {
            // Non-solid fills (gradients) report a negative hue.
            let color = match quad.background.as_solid() {
                Some(c) if c.a > 0. => c,
                Some(_) => continue,
                None => gpui::hsla(-1., 0., 0., 1.),
            };
            let b = placed(quad.bounds, quad.transformation);
            let visible = b.intersect(&quad.content_mask.bounds);
            if visible.size.width.0 <= 0. || visible.size.height.0 <= 0. {
                continue;
            }
            let logical = Bounds::new(
                point(
                    px(visible.origin.x.0 / scale),
                    px(visible.origin.y.0 / scale),
                ),
                size(
                    px(visible.size.width.0 / scale),
                    px(visible.size.height.0 / scale),
                ),
            );
            out.push((logical, color, group));
        }
        // Glyphs report as near-opaque black quads (text presence).
        for glyph in &scene.monochrome_sprites {
            let b = placed(glyph.bounds, glyph.transformation).intersect(&glyph.content_mask.bounds);
            if b.size.width.0 > 0. && b.size.height.0 > 0. {
                let logical = Bounds::new(
                    point(px(b.origin.x.0 / scale), px(b.origin.y.0 / scale)),
                    size(px(b.size.width.0 / scale), px(b.size.height.0 / scale)),
                );
                out.push((logical, gpui::hsla(0., 0., 0., 0.999), group));
            }
        }
        // Raster fills (images, rasterized gradients) report hue -2.
        for sprite in &scene.polychrome_sprites {
            let b = placed(sprite.bounds, sprite.transformation).intersect(&sprite.content_mask.bounds);
            if b.size.width.0 > 0. && b.size.height.0 > 0. {
                let logical = Bounds::new(
                    point(px(b.origin.x.0 / scale), px(b.origin.y.0 / scale)),
                    size(px(b.size.width.0 / scale), px(b.size.height.0 / scale)),
                );
                out.push((logical, gpui::hsla(-2., 0., 0., 1.), group));
            }
        }
        for inner in scene.groups.iter().rev() {
            groups += 1;
            pending.push((&inner.scene, groups));
        }
    }
}

/// Colour of the last painted quad covering `(x, y)`.
pub(crate) fn top(quads: &[(Bounds<Pixels>, Hsla)], x: f32, y: f32) -> Option<Hsla> {
    quads
        .iter()
        .rev()
        .find(|(b, _)| b.contains(&point(px(x), px(y))))
        .map(|(_, c)| *c)
}

mod continuations;

#[test]
fn p08dbg_probe() {
    let list = std::env::var("P08_HTML").unwrap_or_default();
    for path in list.split(';').filter(|p| !p.is_empty()) {
        let html = std::fs::read_to_string(path).unwrap_or_default();
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        let found = if name.contains("-print") {
            printed(&html, 800., 600.)
        } else {
            quads(&html, 800., 600.)
        };
        for (b, c) in found {
            eprintln!(
                "P08Q {name} {:?} {:?} {:?}",
                b.origin,
                b.size,
                (c.h, c.s, c.l)
            );
        }
    }
}
