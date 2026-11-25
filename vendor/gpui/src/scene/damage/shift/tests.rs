use crate::scene::{FrameShift, Scene, SceneDamage};
use crate::{
    Background, ContentMask, Hsla, Quad, ScaledPixels, blue, bounds, green, point, red, size,
};

const W: u32 = 256;
const H: u32 = 256;

fn quad(x: f32, y: f32, w: f32, h: f32, color: Hsla, mask_y: f32) -> Quad {
    Quad {
        bounds: bounds(
            point(ScaledPixels(x), ScaledPixels(y)),
            size(ScaledPixels(w), ScaledPixels(h)),
        ),
        content_mask: ContentMask {
            bounds: bounds(
                point(ScaledPixels(0.0), ScaledPixels(mask_y)),
                size(ScaledPixels(W as f32), ScaledPixels(H as f32)),
            ),
        },
        background: Background::from(color),
        ..Default::default()
    }
}

fn scene(quads: &[Quad]) -> Scene {
    let mut s = Scene::default();
    for (i, q) in quads.iter().enumerate() {
        let mut q = *q;
        q.order = i as u32 + 1;
        s.quads.push(q);
    }
    s
}

/// A page background that stays, text-like boxes that scroll by `top`, and
/// optionally a bar fixed at the top.
fn page(top: f32, bar: bool) -> Scene {
    let mut quads = vec![quad(0.0, 0.0, W as f32, H as f32, Hsla::white(), 0.0)];
    for k in 0..12 {
        let color = if k % 2 == 0 { red() } else { blue() };
        quads.push(quad(10.0, top + 40.0 * k as f32, 100.0, 20.0, color, 0.0));
    }
    if bar {
        quads.push(quad(0.0, 0.0, W as f32, 16.0, green(), 0.0));
    }
    scene(&quads)
}

fn step(old: Scene, new: Scene, d: (i32, i32)) -> (Option<FrameShift>, SceneDamage) {
    let mut old = old;
    old.note_frame_damage(&Scene::default(), 1, (W, H));
    let mut new = new;
    new.note_frame_damage_moved(&old, 2, (W, H), Some(Ok(d)));
    (new.frame_shift().cloned(), new.frame_damage().0)
}

#[test]
fn a_scroll_step_redraws_only_the_uncovered_strip() {
    let (shift, damage) = step(page(0.0, false), page(-20.0, false), (0, -20));
    let shift = shift.expect("the step is a move");
    assert_eq!(shift.copy, [0, 0, W, H - 20]);
    assert_eq!(shift.redraw, vec![[0, H - 20, W, 20]]);
    assert_eq!(
        damage,
        SceneDamage::Rect {
            x: 0,
            y: 0,
            width: W,
            height: H
        }
    );
}

#[test]
fn a_fixed_bar_damages_its_own_band_and_nothing_between() {
    let (shift, _) = step(page(0.0, true), page(-20.0, true), (0, -20));
    let shift = shift.expect("the step is a move");
    let rows: u64 = shift.redraw_area();
    // The strip and the bar (and the bar's moved copy) only, far from the window.
    assert!(rows <= (W * (20 + 64)) as u64, "{shift:?}");
    assert!(shift.redraw.iter().any(|r| r[1] == 0), "{shift:?}");
    assert!(shift.redraw.iter().any(|r| r[1] + r[3] == H), "{shift:?}");
}

#[test]
fn a_changed_box_in_the_moved_part_is_redrawn_too() {
    let mut new = page(-20.0, false);
    new.quads[3].background = Background::from(green());
    let (shift, _) = step(page(0.0, false), new, (0, -20));
    let shift = shift.expect("the step is a move");
    let hit = |r: &[u32; 4]| r[1] <= 60 && r[1] + r[3] >= 80;
    assert!(shift.redraw.iter().any(hit), "{shift:?}");
}

#[test]
fn a_frame_compared_in_place_redraws_each_change_apart() {
    let mut new = page(0.0, false);
    new.quads[1].background = Background::from(green());
    new.quads[6].background = Background::from(green());
    let (shift, damage) = step(page(0.0, false), new, (0, 0));
    let shift = shift.expect("two changes far apart cost less apart");
    assert!(shift.in_place(), "{shift:?}");
    assert_eq!(shift.redraw.len(), 2, "{shift:?}");
    assert!(shift.redraw_area() <= 2 * 128 * 64, "{shift:?}");
    // The damage is the redraws' bounds, not the window the copy would cover.
    let [x, y, width, height] = shift.bounds();
    assert_eq!(
        damage,
        SceneDamage::Rect {
            x,
            y,
            width,
            height
        }
    );
    assert!(height < H, "{shift:?}");
}

#[test]
fn a_move_that_explains_nothing_is_not_planned() {
    // Content moved by 20 but offered as 7: nearly every cell differs.
    let (shift, _) = step(page(0.0, false), page(-20.0, false), (0, -7));
    assert!(
        shift
            .as_ref()
            .is_none_or(|s| s.redraw_area() < (W * H) as u64),
        "{shift:?}"
    );
    let (none, damage) = step(page(0.0, false), page(0.0, false), (0, 0));
    assert_eq!((none, damage), (None, SceneDamage::Unchanged));
}

/// Boxes scrolling inside a scroller whose clip ends above a stationary bar
/// band (`y` 200..256), as a horizontal scroll bar leaves it.
fn clipped_page(top: f32) -> Scene {
    let mut quads = vec![quad(0.0, 200.0, W as f32, 56.0, green(), 0.0)];
    for k in 0..12 {
        let mut q = quad(8.0, top + 40.0 * k as f32, 240.0, 20.0, red(), 0.0);
        q.content_mask.bounds.size.height = ScaledPixels(200.0);
        q.corner_radii.top_left = ScaledPixels(4.0);
        quads.push(q);
    }
    scene(&quads)
}

#[test]
fn a_scroller_clip_that_parts_from_its_moved_copy_damages_only_its_edge() {
    let (shift, _) = step(clipped_page(0.0), clipped_page(-20.0), (0, -20));
    let shift = shift.expect("the step is a move");
    // The cells along the clip's edge and the band below it, not the boxes above.
    assert!(shift.redraw.iter().all(|r| r[1] >= 128), "{shift:?}");
}

#[test]
fn a_comparison_past_its_deadline_gives_up_and_the_same_one_in_time_plans() {
    let mut old = page(0.0, false);
    old.note_frame_damage(&Scene::default(), 1, (W, H));
    let mut new = page(-20.0, false);
    new.note_frame_damage(&old, 2, (W, H));
    let mut stats = Default::default();
    let now = std::time::Instant::now();
    let late = new.plan_shift_within(&old, (W, H), (0, -20), &mut stats, Some(now));
    assert_eq!(late.err(), Some("over_budget"));
    let later = now + std::time::Duration::from_secs(60);
    let in_time =
        new.plan_shift_within(&old, (W, H), (0, -20), &mut Default::default(), Some(later));
    assert_eq!(in_time.expect("a move").redraw, vec![[0, H - 20, W, 20]]);
}
