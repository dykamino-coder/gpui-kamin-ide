// Per-tab scroll position memory for the JSONL chat viewer.
//
// Saved on every scroll event, restored on tab switch. `atBottom` is the
// authoritative pin state: when true, "restore" means snap to the new bottom
// (the tab may have grown while we were away), otherwise we set scrollTop to
// the saved offset.
//
// Module-level singleton — survives every renderer mount/unmount as long as
// the renderer process lives. Cleared implicitly on app reload.

export interface TabScroll { scrollTop: number; atBottom: boolean }

export const tabScrollMemory = new Map<string, TabScroll>()
