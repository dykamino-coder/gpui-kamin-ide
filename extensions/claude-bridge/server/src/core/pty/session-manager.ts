// ============================================================================
// Thin barrel — the original session-manager.ts has been split into:
//   - session-core.ts   (spawn/restart/destroy, MCP routing, helpers)
//   - session-io.ts     (input/output buffering, debounce, auto-responder)
//   - session-reaper.ts (idle / sub-agent / max-lifetime reaper)
// Re-exported here for backwards compatibility with existing importers.
// ============================================================================

export * from './session-core'
export * from './session-reaper'
export * from './session-io'
export * from './console-shrink'
