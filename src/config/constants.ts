// All literal numeric values used by KaminIDE TS code live here. The rule:
// no `setTimeout(fn, 4000)` in source — there's a NAMED constant with a
// `// reason:` comment, and source imports it.
//
// The matching rule for CSS is in `src/renderer/theme/layout-tokens.css`.

// ── Window ───────────────────────────────────────────────────────────────
// Default window size on first launch. 1400×900 fits a 1366×768 laptop with
// a tiny letterbox and looks roomy on 1920×1080 — picked as the smallest
// "feels modern" desktop without forcing maximize on small monitors.
export const WINDOW_DEFAULT_WIDTH_PX = 1400
export const WINDOW_DEFAULT_HEIGHT_PX = 900

// Minimum window size. Below this, the activity bar + sidebar + editor
// stop fitting side-by-side without horizontal scroll. 800×600 = SVGA;
// any tablet-class display we'd ever target is at least this.
export const WINDOW_MIN_WIDTH_PX = 800
export const WINDOW_MIN_HEIGHT_PX = 600

// Background colour shown by the OS shell during the brief window between
// process spawn and the renderer painting. Matches `--bg-sidebar` so the
// transition is invisible — picking the renderer's actual final colour
// avoids a one-frame flash of "wrong-shade" grey.
export const WINDOW_BACKGROUND_COLOR = "#1d1d28"

// ── Notifications ────────────────────────────────────────────────────────
// How long a toast stays on screen. 4 s is the GitHub / VS Code default —
// long enough to read a 6-word message at average speed (~250 wpm), short
// enough that a stack of three doesn't block the bottom-right of the UI.
export const TOAST_VISIBLE_MS = 4_000

// ── Command palette ──────────────────────────────────────────────────────
// Hard cap on rendered list rows. Above this, virtualisation pays off; for
// Phase A we just truncate. Picked 50 because Preact's reconciler stays
// fast at this size on a Pi-class machine and 99 % of palette queries
// narrow below it on the second character. Phase B replaces this with
// a proper virtual list when corpus extensions push command counts past
// a few thousand.
export const PALETTE_MAX_ROWS = 50

// ── Panel resize bounds ──────────────────────────────────────────────────
// User explicit (2026-05-07): minimum 100 for every panel, NO maximum.
// Content inside the panels is extension-contributed; the host
// shouldn't artificially cap the user's preferred width.
export const SIDEBAR_MIN_WIDTH_PX = 100
export const FILE_PANEL_MIN_WIDTH_PX = 100
// The centre ("Left") column is the flex filler; without a floor it
// collapses to 0 when neighbours grow or the window shrinks. Same 100 px
// minimum as every other panel.
export const MAIN_MIN_WIDTH_PX = 100
export const RIGHT_PANEL_MIN_WIDTH_PX = 100
export const BOTTOM_PANE_MIN_HEIGHT_PX = 100

// ── Layout defaults ──────────────────────────────────────────────────────
// First-launch values for the persistent layout store + renderer signal
// initial values. Source of truth — both `host/layout-store.ts` and
// `renderer/signals/layout.ts` import from here so a one-place change
// updates both. Picked from Bridge convention.
export const SIDEBAR_DEFAULT_WIDTH_PX = 270
export const FILE_PANEL_DEFAULT_WIDTH_PX = 360
export const FILE_PANEL_DEFAULT_WIDTH_RATIO = 0.42
export const FILE_PANEL_BOTTOM_DEFAULT_HEIGHT_PX = 180
export const FILE_PANEL_BOTTOM_DEFAULT_HEIGHT_RATIO = 0.3
export const RIGHT_PANEL_DEFAULT_WIDTH_PX = 280
// Top card takes 55% of right panel by default. User-draggable.
export const RIGHT_PANEL_DEFAULT_SPLIT = 0.55

// Main column split — ratio of `main` height to `mainBottom` height
// inside the centre column. Mirrors `RIGHT_PANEL_DEFAULT_SPLIT`: when
// the bottom drawer is on, `main` takes this fraction of the column
// and `mainBottom` takes the rest. Ratio (not pixels) so both halves
// scale proportionally with the window — matching the right-panel
// resize behaviour the user expects.
export const MAIN_DEFAULT_SPLIT = 0.7
export const MAIN_SPLIT_LOWER = 0.2
export const MAIN_SPLIT_UPPER = 0.85

// ── Layout ratio bounds ──────────────────────────────────────────────────
// Prevent drag from collapsing/hyper-expanding ratio panels. Different
// regions get different bounds because their content tolerances differ.
//
// File-panel width as a fraction of the central area: keep at least
// 5% (so the user can see SOMETHING was there) and at most 60% (above
// that the editor becomes uselessly narrow).
export const FILE_PANEL_RATIO_LOWER = 0.05
export const FILE_PANEL_RATIO_UPPER = 0.6
// Right-panel vertical split: keep both cards at least 15% so the
// less-active card still has room for a label.
export const RIGHT_PANEL_SPLIT_LOWER = 0.15
export const RIGHT_PANEL_SPLIT_UPPER = 0.85
// Bottom pane height as a fraction of viewport: 10% minimum (a single
// terminal line + chrome), 80% maximum (above that the editor is
// effectively gone).
export const BOTTOM_PANE_RATIO_LOWER = 0.1
export const BOTTOM_PANE_RATIO_UPPER = 0.8

// ── Extension host ───────────────────────────────────────────────────────
// Tail size of the audit queue kept by the file-watcher hook (.claude/
// hooks/log-change.mjs). NOT consumed by host code — exported here only
// so reviewers see the value next to its peers. Mirror in the .mjs hook.
export const HOOK_REVIEW_QUEUE_MAX = 200
