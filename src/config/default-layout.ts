// The layout a FRESH install boots into.
//
// WHY this exists: `layout_get` returns `{}` until something is saved, and
// `applyLayoutSnapshot` validates key-by-key — so an empty store applied nothing
// and every signal kept its own constant default. That shipped a bare shell:
// file column hidden, right column hidden, no tool pinned anywhere, so the first
// thing a new user saw was an empty window they had to assemble by hand. The
// panel-size constants stay as they are — they're the fallback for a snapshot
// that omits ONE key, which is a different job from "what does the app look
// like out of the box".
//
// This is the FACTORY default and the weakest of the three: a stored layout
// wins over it (hydrateLayout), and a user's own default preset wins over that
// (applyDefaultLayoutOnBoot, later in the boot sequence).
//
// Ratios/pixels come from the maintainer's own working layout, rounded — they
// were captured from a live drag, so the extra decimals were sub-pixel noise.
// Ids must match real contributed view ids (`claudeBridge*` come from the
// Bridge VSIX): hydratePanelState drops non-strings but CANNOT know that an id
// names nothing, and an unknown id just yields an empty zone.
import type { LayoutSnapshot } from "../api/types.js"

export const DEFAULT_LAYOUT_SNAPSHOT: LayoutSnapshot = {
  sidebarVisible: true,
  sidebarWidthPx: 241,
  filePanelVisible: true,
  filePanelMode: "files",
  filePanelWidthRatio: 0.478,
  filePanelBottomVisible: true,
  filePanelBottomHeightRatio: 0.449,
  rightPanelVisible: true,
  rightPanelWidthPx: 415,
  rightPanelBottomVisible: true,
  mainVisible: true,
  mainBottomVisible: false,
  mainSplit: 0.66,
  // Left rail: projects/sessions.
  activitySidebar: { pinned: ["projects"], active: "projects" },
  // Right column: file tree on top, Bridge plan/todos/agents below.
  activityRightTop: { pinned: ["tree"], active: "tree" },
  activityRightBottom: {
    pinned: ["claudeBridgePlan", "claudeBridgeTodos", "claudeBridgeAgents"],
    active: "claudeBridgePlan",
  },
  // Centre column: editor on top, Bridge console in the drawer.
  activityCentralTop: { pinned: [], active: null },
  activityCentralBottom: { pinned: ["claudeBridgeConsole"], active: "claudeBridgeConsole" },
  // Main surface: the Bridge chat — the reason the app exists.
  activityMain: { pinned: ["claudeBridge"], active: "claudeBridge" },
  activityMainBottom: { pinned: [], active: null },
}
