export interface ContentBlock {
  type: 'text' | 'tool_use' | 'tool_result' | 'thinking'
  text?: string
  thinking?: string
  name?: string
  id?: string
  input?: any
  content?: string | ContentBlock[]
  is_error?: boolean
  tool_use_id?: string
  /** Set by trimHeavyToolResults when a fat tool_result body was truncated
   *  in-memory: the ORIGINAL char count, so the "N chars" label stays honest
   *  even though `content` now holds only a prefix. Absent = untrimmed. */
  _fullLen?: number
}

export interface MessageUsage {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

export interface JsonlMessage {
  /** Anthropic message id (msg_*) — used by streaming dedup. */
  id?: string
  role: string
  content: ContentBlock[] | string
  model?: string
  usage?: MessageUsage
  stop_reason?: string | null
}

export interface JsonlEntryData {
  type: string
  uuid?: string
  /** parentUuid→uuid chain — used by orderEntries to keep a child after its
   *  parent, and by compute-chain-uuids for the active-branch membership set. */
  parentUuid?: string
  /** Server reverse-replay file-position tag (monotonic per process); primary
   *  sort key in orderEntries. Absent on live streaming stubs (they sort last). */
  _ord?: number
  timestamp?: string
  message?: JsonlMessage
  /** For system entries */
  content?: string
  subtype?: string
  /** For assistant entries */
  model?: string
  usage?: MessageUsage
  isSidechain?: boolean
  /** Owning teammate/subagent for a sidechain STREAM stub — the CLI's
   *  `x-claude-code-agent-id` (`<name>-<n>@<team>`), stamped by the MITM. Lets the
   *  Agents panel / fullscreen route a live stream to that agent. Sidechain
   *  streaming stubs only. */
  subagentId?: string
  /** For result entries */
  result?: string
  cost_usd?: number
  duration_ms?: number

  // Renderer-side runtime annotations set by JsonlViewer / ToolGroup logic.
  // Optional so wire-format entries from the watcher don't need to carry
  // them. Keeping the interface flat (vs N discriminated unions) is a
  // pragmatic trade — see refactor plan stage B3.
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
   *  one card. JsonlViewer fills this when grouping. */
  _groupEntries?: JsonlEntryData[]

  /** True when this entry is being streamed live from MITM proxy SSE.
   *  Renderer shows a blinking cursor + skips tool-burst grouping while
   *  set. Replaced by the real JSONL entry once watcher delivers it. */
  __streaming?: boolean
  /** Index of the currently-active content block for cursor placement. */
  __streamingActiveBlock?: number
  /** Client-stamped time the streaming stub was first seen (ms epoch). Used by
   *  the TTL sweep to retire an orphaned stub whose canonical entry never came. */
  __streamStartedAt?: number
  /** Every source uuid FOLDED into this entry. The CLI splits one model turn into
   *  N JSONL rows sharing a message.id but each with its OWN uuid; appendJsonlEntries
   *  merges them into one entry (whose `.uuid` is only the LAST one) and adds every
   *  uuid to the dedup `seen` set. Trim must forget ALL of them, not just `.uuid`,
   *  or the non-surviving uuids stay in `seen` forever and a re-replay drops those
   *  content blocks as duplicates — the middle-of-chat data-loss gap. */
  __uuids?: string[]

  // Optional API-error subtype fields (rendered in SystemEntry.ApiErrorBlock).
  retryInMs?: number
  retryAttempt?: number
  maxRetries?: number
  cause?: { code?: string; path?: string; [k: string]: unknown }
  error?: { type?: string; cause?: { code?: string; path?: string; [k: string]: unknown } }
  // system/model_fallback: the CLI switched models mid-session (e.g. Opus→Sonnet
  // under load). Carries a human `content` line plus the two model ids.
  originalModel?: string
  fallbackModel?: string
  // The Agent/Task tool's structured result (a sibling of `message` on the user
  // entry that carries the tool_result). Field names verified on real 2.1.x
  // transcripts (cc81c65d); the agent tiles read the completion summary from it.
  // Loose bag — only the fields the tiles use are typed.
  toolUseResult?: {
    status?: string
    agentType?: string
    agentId?: string
    totalTokens?: number
    totalToolUseCount?: number
    totalDurationMs?: number
    [k: string]: unknown
  }
}

export interface TeammateMsg {
  id: string
  color?: string
  body: string
  parsed?: {
    type: string
    message?: string
    from?: string
    requestId?: string
    reason?: string
    idleReason?: string
    summary?: string
    [key: string]: unknown
  }
}
