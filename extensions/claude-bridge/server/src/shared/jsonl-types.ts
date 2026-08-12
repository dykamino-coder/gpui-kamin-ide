// ============================================================================
// JSONL Session Types — shared between server and client host
// Claude Code writes session data to ~/.claude/projects/<slug>/<session-id>.jsonl
// ============================================================================

// ---------------------------------------------------------------------------
// Content blocks inside assistant messages
// ---------------------------------------------------------------------------

export interface ThinkingBlock {
  type: 'thinking'
  thinking: string
}

export interface TextBlock {
  type: 'text'
  text: string
}

export interface ToolUseBlock {
  type: 'tool_use'
  id?: string
  name: string
  input: Record<string, unknown>
}

export interface ToolResultBlock {
  type: 'tool_result'
  tool_use_id: string
  content: string | ContentBlock[]
}

export type ContentBlock = ThinkingBlock | TextBlock | ToolUseBlock | ToolResultBlock

// ---------------------------------------------------------------------------
// JSONL entry (one line in the .jsonl file)
// ---------------------------------------------------------------------------

export interface JsonlEntry {
  // Core message types render in the viewer.
  // Metadata types (summary, agent-*, mode, etc.) are preserved in the stream
  // for consumers (tab title, agent colors, permission mode sync) but rendered
  // as null by the default JsonlEntry dispatcher.
  type:
    | 'user' | 'assistant' | 'system'
    | 'file-history-snapshot' | 'attribution-snapshot'
    | 'summary' | 'custom-title' | 'ai-title' | 'last-prompt' | 'task-summary' | 'tag'
    | 'agent-name' | 'agent-color' | 'agent-setting'
    | 'mode' | 'permission-mode' | 'pr-link' | 'queue-operation' | 'worktree-state' | 'content-replacement'
    | 'attachment' | 'progress' | 'speculation-accept'
  uuid: string
  parentUuid?: string | null
  isSidechain?: boolean
  timestamp?: string
  sessionId?: string
  cwd?: string
  version?: string
  permissionMode?: string

  message?: {
    role?: string
    content?: string | ContentBlock[]
    model?: string
    id?: string
    stop_reason?: string | null
    usage?: {
      input_tokens: number
      output_tokens: number
      cache_read_input_tokens?: number
      cache_creation_input_tokens?: number
    }
  }

  // Only on assistant entries
  requestId?: string

  // file-history-snapshot
  snapshot?: {
    trackedFileBackups: Record<string, unknown>
    timestamp: string
  }

  // Metadata entry fields (type-dependent; kept loose to avoid 20+ union types)
  leafUuid?: string
  summary?: string
  title?: string
  tag?: string
  agentType?: string
  agentName?: string
  agentColor?: string
  mode?: string
  prNumber?: number
  prUrl?: string
  prRepository?: string

  // Renderer-side runtime annotations — set by JsonlViewer to mark
  // entries as already-handled (compact summary expanded inline,
  // CLI-generated meta lines hidden, etc). Optional so wire-format
  // entries don't need to carry them.
  isCompactSummary?: boolean
  isMeta?: boolean
  // Field names match the CLI's actual compact_boundary payload (verified on
  // real 2.1.x transcripts) — NOT the `*TokenCount` guesses that never resolved.
  compactMetadata?: {
    trigger?: string
    preTokens?: number
    postTokens?: number
    durationMs?: number
    preCompactDiscoveredTools?: string[]
    preservedSegment?: { headUuid?: string; anchorUuid?: string; tailUuid?: string }
    preservedMessages?: number
    cumulativeDroppedTokens?: number
  }
  interruptedByUser?: boolean
  skippedByUser?: boolean
  attachment?: {
    type?: string
    prompt?: string
    filename?: string
    snippet?: string
    [k: string]: unknown
  }
  /** Tool-burst pseudo-entry: collapses N consecutive tool_use turns into
   *  one card. Renderer fills this when grouping. */
  _groupEntries?: JsonlEntry[]

  // Some legacy system entries put plain text in `content` instead of
  // `message.content` — keep the field typed at the entry level so the
  // viewer can read it without `as any`.
  content?: string
}

// ---------------------------------------------------------------------------
// JSONL watcher status
// ---------------------------------------------------------------------------

export interface JsonlStatus {
  status: 'searching' | 'watching' | 'error'
  filePath?: string
  error?: string
  /** True when watcher switched to a new JSONL file (compaction/new conversation).
   * Renderer should clear existing entries for this tab. */
  compacted?: boolean
  /** True when all historical JSONL entries have been sent (replay phase is done).
   * Renderer can now distinguish historical agents from live ones. */
  replayComplete?: boolean
  /** Fingerprint of the transcript file at replay-complete: the hash of its
   *  first KB and its size. A client that mirrors the transcript stores these
   *  with its copy and offers them back next launch, so the server can tell a
   *  still-valid prefix from a rewritten file. */
  fileHead?: string
  fileSize?: number
  /** Live replay progress, sent per batch while the historical transcript
   *  streams. `sent`/`total` are BYTES of the file (not record counts), so a
   *  client can show "loading history … N%". Absent once replay completes. */
  replayProgress?: { sent: number; total: number }
  /** The full compact-segment index, computed once from the authoritative
   *  transcript and sent alongside `replayComplete`. `boundaries` are the
   *  compact_boundary timestamps in file order; `counts` has one more entry —
   *  counts[0] the original pre-compaction segment, counts[k+1] the segment
   *  after boundaries[k]. Lets the strip show EVERY segment with visible-message
   *  counts, independent of the client's bounded local mirror. */
  segmentIndex?: { boundaries: { ts: string }[]; counts: number[] }
}
