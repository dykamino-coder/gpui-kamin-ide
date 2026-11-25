use super::*;
use crate::scene::{PaintOperation, Primitive};
use crate::{Quad, ScrollHandle, TestAppContext, div, px, size};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

/// A quad painted as an element paints one: under the current content mask.
fn quad(window: &mut Window, y: f32) {
    let scale = window.scale_factor();
    let bounds = rect(0., y, 10., 10.).scale(scale);
    let content_mask = window.content_mask().scale(scale);
    window.insert_scene_primitive(Quad { bounds, content_mask, ..Default::default() });
}

/// Drawn quads (top in window pixels, drawn mask) and the number of dormant ones.
fn drawn(window: &Window) -> (Vec<(f32, Bounds<ScaledPixels>)>, usize) {
    let scale = window.scale_factor();
    let mut quads = Vec::new();
    let mut dormant = 0;
    for operation in &window.next_frame.scene.paint_operations {
        match operation {
            PaintOperation::Primitive(primitive @ Primitive::Quad(_)) => {
                // Where it lands: a moved copy moves its transformation.
                let placed = crate::scene::placed_bounds(primitive);
                quads.push((placed.origin.y.0 / scale, primitive.content_mask().bounds));
            }
            PaintOperation::Dormant(_) => dormant += 1,
            _ => {}
        }
    }
    (quads, dormant)
}

const CLIP: (f32, f32, f32, f32) = (0., 0., 100., 50.);

fn clip() -> Bounds<Pixels> {
    rect(CLIP.0, CLIP.1, CLIP.2, CLIP.3)
}

/// One record: a scroll node at `offset` holding a hitbox and a quad at
/// content rows 10 and 60 (the second below the 50 px clip).
fn record(window: &mut Window, handle: &ScrollHandle, offset: f32) -> (RetainedPrepaint, RetainedPaint, Vec<Hitbox>) {
    let mut hitboxes = Vec::new();
    let node = ScrollNodeId::of(handle);
    handle.set_max_offset(point(px(0.), px(10_000.)));
    handle.set_offset(point(px(0.), px(offset)));
    window.invalidator.set_phase(DrawPhase::Prepaint);
    let (_, prepaint) = window.record_retained_prepaint(|window| {
        window.with_scroll_node_prepaint(handle, clip(), |window| {
            for y in [10., 60.] {
                hitboxes.push(window.insert_hitbox(rect(0., y + offset, 10., 10.), HitboxBehavior::Normal));
            }
        });
    });
    window.invalidator.set_phase(DrawPhase::Paint);
    let (_, paint) = window.record_retained_paint(|window| {
        window.with_scroll_node_paint(node, |window| {
            for y in [10., 60.] {
                quad(window, y + offset);
            }
        });
    });
    (prepaint, paint, hitboxes)
}

fn next_frame(window: &mut Window) {
    std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
    window.next_frame.clear();
}

/// Replay of `prepaint`/`paint` from the rendered frame with the node at `to`.
fn scrolled(
    window: &mut Window,
    records: (&RetainedPrepaint, &RetainedPaint),
    handle: &ScrollHandle,
    to: f32,
) -> Option<(RetainedPrepaint, RetainedPaint)> {
    handle.set_offset(point(px(0.), px(to)));
    let plan = window.plan_scrolled_replay(records.0, records.1, None)?;
    window.invalidator.set_phase(DrawPhase::Prepaint);
    let prepaint = window.replay_retained_prepaint_scrolled(records.0, &plan);
    window.invalidator.set_phase(DrawPhase::Paint);
    let paint = window.replay_retained_paint_scrolled(records.1, &plan);
    Some((prepaint, paint))
}

/// A wheel step inside the interest rect replays the record moved: what
/// scrolls out stays in the node's reservoir, what scrolls in is drawn,
/// and the frame equals a fresh paint at the new offset; the step back wakes
/// the first row again. A step past the interest rect needs a build.
#[crate::test]
fn a_scroll_step_inside_the_interest_rect_replays_like_a_fresh_paint(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let s = window.scale_factor();
            let handle = ScrollHandle::new();
            let node = &handle;
            let (prepaint, paint, _) = record(window, node, 0.);
            let at_rest = drawn(window);
            assert_eq!(at_rest, (vec![(10., clip().scale(s))], 1));
            let at_rest = at_rest.0;
            next_frame(window);
            let (prepaint, paint) = scrolled(window, (&prepaint, &paint), node, -30.).expect("a step inside the interest rect");
            let stepped = drawn(window).0;
            next_frame(window);
            let (prepaint, paint) = scrolled(window, (&prepaint, &paint), node, 0.).expect("the step back");
            assert_eq!(drawn(window).0, at_rest, "the row that scrolled out comes back");
            next_frame(window);
            assert!(scrolled(window, (&prepaint, &paint), node, -200.).is_none());
            next_frame(window);
            record(window, node, -30.);
            assert_eq!(stepped, drawn(window).0, "a fresh paint at the stepped offset");
            assert_eq!(stepped, vec![(30., clip().scale(s))]);        })
        .unwrap();
}

/// Hitboxes under a node move with it and are hit-tested with own ∩ clip.
#[crate::test]
fn hitboxes_move_with_their_scroll_node(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let handle = ScrollHandle::new();
            let node = &handle;
            let (prepaint, paint, hitboxes) = record(window, node, 0.);
            let hit = |window: &Window, y: f32, hitbox: &Hitbox| {
                window.next_frame.hit_test(point(px(5.), px(y))).ids.contains(&hitbox.id)
            };
            assert!(hit(window, 15., &hitboxes[0]));
            assert!(!hit(window, 65., &hitboxes[1]), "outside the clip");
            next_frame(window);
            scrolled(window, (&prepaint, &paint), node, -30.).unwrap();
            assert!(hit(window, 35., &hitboxes[1]), "scrolled into the clip");
            assert!(!hit(window, 15., &hitboxes[0]), "moved away and clipped");
        })
        .unwrap();
}

/// A nested node's clip follows its parent's shift; an anchor (a fixed box)
/// stays where it is.
#[crate::test]
fn nested_nodes_and_anchors_follow_their_parent(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let s = window.scale_factor();
            let (outer, inner, fixed) = (ScrollHandle::new(), ScrollHandle::new(), ScrollHandle::new());
            let id = ScrollNodeId::of;
            outer.set_max_offset(point(px(0.), px(1000.)));
            let fixed_at = Rc::new(std::cell::Cell::new(point(px(0.), px(0.))));
            let fixed_source: Rc<dyn Fn() -> Point<Pixels>> = {
                let at = fixed_at.clone();
                Rc::new(move || at.get())
            };
            let tall = rect(0., 0., 100., 100.);
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                { outer.set_offset(point(px(0.), px(0.))); &mut *window }.with_scroll_node_prepaint(&outer, tall, |window| {
                    window.with_content_mask(Some(ContentMask { bounds: rect(0., 50., 50., 30.) }), |window| {
                        let chain = window.content_mask().bounds;
                        { inner.set_offset(point(px(0.), px(0.))); &mut *window }.with_scroll_node_prepaint(&inner, chain, |_| {});
                    });
                    window.with_scroll_anchor_prepaint(id(&fixed), fixed_source.clone(), |_| {});
                });
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, paint) = window.record_retained_paint(|window| {
                window.with_scroll_node_paint(id(&outer), |window| {
                    window.with_content_mask(Some(ContentMask { bounds: rect(0., 50., 50., 30.) }), |window| {
                        window.with_scroll_node_paint(id(&inner), |window| quad(window, 60.));
                    });
                    window.with_scroll_node_paint(id(&fixed), |window| quad(window, 0.));
                });
            });
            next_frame(window);
            outer.set_offset(point(px(0.), px(-20.)));
            fixed_at.set(point(px(0.), px(20.)));
            let plan = window.plan_scrolled_replay(&prepaint, &paint, None).expect("inside the interest rect");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            window.replay_retained_paint_scrolled(&paint, &plan);
            let clips: Vec<_> = window.next_frame.scroll_nodes.iter().map(|node| node.clip).collect();
            assert_eq!(clips, vec![tall, rect(0., 30., 50., 30.), tall]);
            let (quads, _) = drawn(window);
            assert_eq!(quads, vec![(40., rect(0., 30., 50., 30.).scale(s)), (0., tall.scale(s))]);
        })
        .unwrap();
}

/// A command under a node without its own mask (a copy that lost it) makes
/// the record build instead.
#[crate::test]
fn an_unmarked_command_under_a_node_needs_a_build(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let s = window.scale_factor();
            let handle = ScrollHandle::new();
            handle.set_max_offset(point(px(0.), px(1000.)));
            let node = ScrollNodeId::of(&handle);
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&handle, clip(), |_| {});
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, paint) = window.record_retained_paint(|window| {
                window.with_scroll_node_paint(node, |window| {
                    let bounds = rect(0., 10., 10., 10.).scale(s);
                    window.next_frame.scene.insert_primitive(Quad {
                        bounds,
                        content_mask: ContentMask { bounds },
                        ..Default::default()
                    });
                });
            });
            next_frame(window);
            handle.set_offset(point(px(0.), px(-10.)));
            assert!(window.plan_scrolled_replay(&prepaint, &paint, None).is_none());
        })
        .unwrap();
}

/// One text line as a line paints it: a layer over the line with two glyph
/// sprites in it, under the current content mask.
fn glyph_line(window: &mut Window, y: f32) {
    let scale = window.scale_factor();
    window.paint_layer(rect(0., y, 40., 10.), |window| {
        for x in [0., 20.] {
            let bounds = rect(x, y, 8., 10.).scale(scale);
            let content_mask = window.content_mask().scale(scale);
            window.insert_scene_primitive(crate::MonochromeSprite {
                order: 0,
                pad: 0,
                bounds,
                content_mask,
                color: Default::default(),
                tile: crate::AtlasTile {
                    texture_id: crate::AtlasTextureId { index: 0, kind: crate::AtlasTextureKind::Monochrome },
                    tile_id: crate::TileId(1),
                    padding: 0,
                    bounds: Default::default(),
                },
                transformation: crate::TransformationMatrix::unit(),
            });
        }
    });
}

/// Drawn glyphs (left and top in window pixels, drawn mask), each inside a layer.
fn glyphs(window: &Window) -> Vec<(f32, f32, Bounds<ScaledPixels>)> {
    let scale = window.scale_factor();
    let mut depth = 0usize;
    let mut out = Vec::new();
    for operation in &window.next_frame.scene.paint_operations {
        match operation {
            PaintOperation::StartLayer(..) => depth += 1,
            PaintOperation::EndLayer => depth -= 1,
            PaintOperation::Primitive(primitive @ Primitive::MonochromeSprite(_)) => {
                assert!(depth > 0, "a glyph outside its line's layer");
                let placed = crate::scene::placed_bounds(primitive);
                out.push((placed.origin.x.0 / scale, placed.origin.y.0 / scale, primitive.content_mask().bounds));
            }
            _ => {}
        }
    }
    out
}

/// Text lines are layers of glyphs (`paint_layer`), painted under the node
/// directly or copied there moved from a line record of the previous frame
/// (an inline line capture replayed, `replay_retained_paint_moved`): both
/// move with the node, and a step equals a fresh paint of the lines.
#[crate::test]
fn glyph_lines_and_their_moved_copies_scroll_like_a_fresh_paint(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let handle = ScrollHandle::new();
            handle.set_max_offset(point(px(0.), px(10_000.)));
            let node = ScrollNodeId::of(&handle);
            // The middle line: a record made at -60 (frame 0), copied moved
            // by 120 (frame 1), or painted where it is (a fresh paint).
            let paint_lines = |window: &mut Window, offset: f32, copy: Option<&RetainedPaint>, source: bool| {
                handle.set_offset(point(px(0.), px(offset)));
                window.invalidator.set_phase(DrawPhase::Prepaint);
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    window.with_scroll_node_prepaint(&handle, clip(), |_| {});
                });
                window.invalidator.set_phase(DrawPhase::Paint);
                let mut line = None;
                let (_, paint) = window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(node, |window| {
                        glyph_line(window, 10. + offset);
                        match copy {
                            Some(record) => {
                                window.replay_retained_paint_moved(record, point(px(0.), px(120.)));
                            }
                            None if source => {
                                line = Some(window.record_retained_paint(|window| glyph_line(window, -60. + offset)).1);
                            }
                            None => glyph_line(window, 60. + offset),
                        }
                        glyph_line(window, 110. + offset);
                    });
                });
                (prepaint, paint, line)
            };
            let (_, _, line) = paint_lines(window, 0., None, true);
            let line = line.unwrap();
            next_frame(window);
            let (prepaint, paint, _) = paint_lines(window, 0., Some(&line), false);
            assert_eq!(glyphs(window).len(), 2, "only the first line shows at rest");
            next_frame(window);
            let (prepaint, paint) = scrolled(window, (&prepaint, &paint), &handle, -40.).expect("a step inside the interest rect");
            let stepped = glyphs(window);
            next_frame(window);
            scrolled(window, (&prepaint, &paint), &handle, -80.).expect("a second step");
            let second = glyphs(window);
            next_frame(window);
            paint_lines(window, -40., None, false);
            assert_eq!(stepped, glyphs(window), "the first step is a fresh paint at -40");
            assert_eq!(stepped.iter().map(|glyph| glyph.1).collect::<Vec<_>>(), vec![20., 20.], "the copied line");
            next_frame(window);
            paint_lines(window, -80., None, false);
            assert_eq!(second, glyphs(window), "the second step is a fresh paint at -80");
            assert_eq!(second.iter().map(|glyph| glyph.1).collect::<Vec<_>>(), vec![30., 30.], "the third line");
        })
        .unwrap();
}

/// The cost of a scrolled replay against a plain one on a text-like record:
/// lines of glyph-sized quads every 20 px (80 and 480 a line) under an 800x600 node with
/// a 2000 px interest margin. Runs only with `KAMIN_SCROLL_COST_OUT` set (a
/// file the timings are appended to); a measurement, not a gate.
#[crate::test]
fn scrolled_replay_cost(cx: &mut TestAppContext) {
    let Ok(out) = std::env::var("KAMIN_SCROLL_COST_OUT") else { return };
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&out).unwrap();
            window.scroll_interest_margin = px(2000.);
            let handle = ScrollHandle::new();
            handle.set_max_offset(point(px(0.), px(100_000.)));
            let node = ScrollNodeId::of(&handle);
            let port = rect(0., 0., 800., 600.);
            for (rows, cols, step_x) in [(230usize, 80usize, 10.), (230, 480, 5.)] {
                window.next_frame.clear();
                handle.set_offset(point(px(0.), px(0.)));
                window.invalidator.set_phase(DrawPhase::Prepaint);
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    window.with_scroll_node_prepaint(&handle, port, |_| {});
                });
                window.invalidator.set_phase(DrawPhase::Paint);
                let (_, paint) = window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(node, |window| {
                        for row in 0..rows {
                            for col in 0..cols {
                                let scale = window.scale_factor();
                                let bounds = rect(col as f32 * step_x, row as f32 * 20. - 2000., 4., 16.).scale(scale);
                                let content_mask = window.content_mask().scale(scale);
                                window.insert_scene_primitive(Quad { bounds, content_mask, ..Default::default() });
                            }
                        }
                    });
                });
                let (mut plain, mut moved, mut planned) = (Vec::new(), Vec::new(), Vec::new());
                let (mut prepaint, mut paint) = (prepaint, paint);
                for step in 1..=9 {
                    next_frame(window);
                    let started = std::time::Instant::now();
                    window.invalidator.set_phase(DrawPhase::Prepaint);
                    let p = window.replay_retained_prepaint(&prepaint);
                    window.invalidator.set_phase(DrawPhase::Paint);
                    let q = window.replay_retained_paint(&paint);
                    plain.push(started.elapsed().as_secs_f64() * 1000.);
                    (prepaint, paint) = (p, q);
                    next_frame(window);
                    let started = std::time::Instant::now();
                    handle.set_offset(point(px(0.), px(-120. * step as f32)));
                    let plan = window.plan_scrolled_replay(&prepaint, &paint, None).expect("inside the interest");
                    planned.push(started.elapsed().as_secs_f64() * 1000.);
                    window.invalidator.set_phase(DrawPhase::Prepaint);
                    let p = window.replay_retained_prepaint_scrolled(&prepaint, &plan);
                    window.invalidator.set_phase(DrawPhase::Paint);
                    let q = window.replay_retained_paint_scrolled(&paint, &plan);
                    moved.push(started.elapsed().as_secs_f64() * 1000.);
                    (prepaint, paint) = (p, q);
                }
                let median = |v: &mut Vec<f64>| {
                    v.sort_by(f64::total_cmp);
                    v[v.len() / 2]
                };
                let (drawn, dormant) = drawn(window);
                writeln!(
                    file,
                    "commands {} drawn {} dormant {}: plain replay {:.3} ms, scrolled replay {:.3} ms (plan {:.3} ms)",
                    rows * cols, drawn.len(), dormant, median(&mut plain), median(&mut moved), median(&mut planned)
                )
                .unwrap();
            }
        })
        .unwrap();
}

/// An anchor whose children never painted moves apart from its scroller in
/// a step. Past the scroller's clip it shows nothing there either: the step
/// replays. Inside the clip it would show what was never recorded: a build.
#[crate::test]
fn an_unpainted_anchor_replays_only_while_its_clip_is_empty(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let tall = rect(0., 0., 100., 100.);
            for (mask, replays) in [(rect(0., 300., 50., 30.), true), (rect(0., 50., 50., 30.), false)] {
                let (outer, token) = (ScrollHandle::new(), Rc::new(()));
                outer.set_max_offset(point(px(0.), px(1000.)));
                let anchor = ScrollNodeId::of_token(&token);
                let counter: Rc<dyn Fn() -> Point<Pixels>> = {
                    let outer = outer.clone();
                    Rc::new(move || -outer.offset())
                };
                window.invalidator.set_phase(DrawPhase::Prepaint);
                let (_, prepaint) = window.record_retained_prepaint(|window| {
                    window.with_scroll_node_prepaint(&outer, tall, |window| {
                        window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
                            window.with_scroll_anchor_prepaint(anchor, counter.clone(), |_| {});
                        });
                    });
                });
                window.invalidator.set_phase(DrawPhase::Paint);
                let (_, paint) = window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(ScrollNodeId::of(&outer), |window| quad(window, 10.));
                });
                next_frame(window);
                outer.set_offset(point(px(0.), px(-20.)));
                let planned = window.plan_scrolled_replay(&prepaint, &paint, None).is_some();
                assert_eq!(planned, replays, "anchor under {mask:?}");
                window.next_frame.clear();
            }
        })
        .unwrap();
}
/// A scroll node painted inside a composition group (a stationary anchor of
/// a box with opacity) has its range in the group's scene. A step of the
/// scroller around it replays with the group, and the copied node lands in
/// the scene of the group's copy, so the next step replays again instead of
/// finding the node without paint.
#[crate::test]
fn a_node_painted_in_a_group_replays_step_after_step(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Empty);
    window
        .update(cx, |_, window, _| {
            window.scroll_interest_margin = px(100.);
            let tall = rect(0., 0., 100., 100.);
            let (outer, token) = (ScrollHandle::new(), Rc::new(()));
            outer.set_max_offset(point(px(0.), px(1000.)));
            let anchor = ScrollNodeId::of_token(&token);
            let counter: Rc<dyn Fn() -> Point<Pixels>> = {
                let outer = outer.clone();
                Rc::new(move || -outer.offset())
            };
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, mut prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&outer, tall, |window| {
                    window.with_scroll_anchor_prepaint(anchor, counter.clone(), |_| {});
                });
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, mut paint) = window.record_retained_paint(|window| {
                window.with_scroll_node_paint(ScrollNodeId::of(&outer), |window| {
                    quad(window, 10.);
                    let bounds = rect(0., 0., 50., 50.);
                    window.paint_group(bounds, Corners::default(), 0., 0.5, 0, &[], None, None, |window| {
                        window.with_scroll_node_paint(anchor, |window| quad(window, 20.));
                    });
                });
            });
            for (step, to) in [-20., -40.].into_iter().enumerate() {
                next_frame(window);
                (prepaint, paint) = scrolled(window, (&prepaint, &paint), &outer, to)
                    .unwrap_or_else(|| panic!("step {step} replays with the group"));
                let space = window.next_frame.scene.groups.last().map(|group| group.scene.scroll.space);
                let node = window.next_frame.scroll_nodes.iter().find(|node| node.id == anchor).unwrap();
                assert_eq!((node.paint.is_some(), node.painted_in), (true, space), "step {step}: the anchor lands in the group's copy");
            }
        })
        .unwrap();
}