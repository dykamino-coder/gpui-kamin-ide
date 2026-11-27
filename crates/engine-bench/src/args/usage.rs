//! CLI usage text.
pub(crate) const USAGE: &str = "engine_bench <file.html> [--viewport WxH] [--scale-factor N] [--root-scroller] [--runs N] \
    [--gpu default|high-performance|minimum-power|software|NAME] [--mode cold|warm|both|relayout|scroll|edit] [--edit text|attr|class|insert|remove | --edit-markup FILE] [--json] [--snapshot first.png] [--frame-diff-dir DIR] [--fresh-check-dir DIR] [--why DIR] [--trace-frames] [--animation-step-ms N] [--scroll-back [--scroll-step PX]] [--memory-settle-ms N (2000..60000)] [--surface offscreen|window] [--plan PLAN.json]\n\
    engine_bench --list-gpus\n\
    engine_bench --why-compare A B   (what changed between two --why runs: dirs or why.json)\n\
    engine_bench --build-info   (features and profile of this exe, as in the JSON's `build`)\n\
    Default: 800x600, 30 runs per mode, both modes. Windows, release profile only.\n\
    Scale defaults to 1; viewport is CSS pixels, capture is rounded device pixels.\n\
    --root-scroller builds the standalone viewer's root scrollport in any mode.\n\
    --why also keeps progress.json once per second: running intervals and completed pass counts, even during a stalled frame.\n\
    Relayout alternates the CSS viewport width by one pixel on the same Document; capture stays fixed.\n\
Scroll dispatches one wheel step of 120 px down beside the root bar, in its visible content (viewport centre if unavailable).\n\
    Edit alternates the input and a local edit (--edit, default text) through Document::update,\n\
    then checks every captured edit frame against a fresh document; missing captures cannot pass.\n\
    insert copies and remove deletes the first leaf element after the middle; --edit-markup FILE is\n\
    the second markup itself, for any other mutation.\n\
    Animation elapsed is index * N ms (initial index 0, default step 16.667 ms).\n\
    Surface: offscreen (default) draws into a GPU texture of a message-only window, never seen\n\
    by the desktop compositor; window parks a presented desktop window off-screen (comparison only).\n\
    Plan: every phase of PLAN.json in this one process (see plan.rs), each on its own input, without the\n\
    memory steady window before each phase; frame series by duration; one JSON report with all phases.";
