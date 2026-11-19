//! Two distant edits must not invalidate the clean space between them.
use super::*;

fn corners(changed: bool) -> Scene {
    let mut scene = Scene::default();
    for (order, rect) in [
        [8., 8., 16., 16.],
        [W as f32 - 24., H as f32 - 24., 16., 16.],
    ]
    .into_iter()
    .enumerate()
    {
        let mut q = quad(rect, hsla(if changed { 0.7 } else { 0.2 }, 0.8, 0.5, 0.6));
        q.order = order as u32 + 1;
        scene.quads.push(q);
    }
    let mut middle = quad([88., 56., 80., 80.], hsla(0.4, 0.7, 0.4, 0.5));
    middle.order = 3;
    scene.quads.push(middle);
    scene
}

#[test]
fn distant_corner_edits_preserve_the_clean_middle_and_fresh_pixels() {
    let device = DirectXDevices::with_instrumentation(
        crate::GpuAdapterPreference::Software,
        gpui::Instrumentation::default(),
    )
    .unwrap();
    let mut before = corners(false);
    before.note_frame_damage(&Scene::default(), 1, (W, H));
    let mut after = corners(true);
    after.note_frame_damage(&before, 2, (W, H));
    let mut retained = offscreen(&device, W, H);
    retained
        .draw(&before, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let plan = retained.plan_frame(&after, WindowBackgroundAppearance::Opaque);
    assert!(
        matches!(plan, crate::directx_renderer::retained::Plan::Within(_)),
        "two corner tiles must not force a full-frame redraw"
    );
    if let crate::directx_renderer::retained::Plan::Within(regions) = &plan {
        assert_eq!(regions.len(), 2);
        assert_eq!(
            regions.iter().map(|r| r.width * r.height).sum::<u32>(),
            8192
        );
    }
    // Rebuild the comparison chain after inspecting its plan.
    retained.forget_frames();
    retained
        .draw(&before, WindowBackgroundAppearance::Opaque)
        .unwrap();
    retained
        .draw(&after, WindowBackgroundAppearance::Opaque)
        .unwrap();
    let mut fresh = offscreen(&device, W, H);
    fresh
        .draw(&after, WindowBackgroundAppearance::Opaque)
        .unwrap();
    assert_eq!(target_bytes(&mut retained), target_bytes(&mut fresh));
}
