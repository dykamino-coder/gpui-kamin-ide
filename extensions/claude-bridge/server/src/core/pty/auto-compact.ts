// Bridge's auto-compaction policy: the single source of truth for both the
// CLI environment (session-settings.ts) and the context meter in the chat
// (`autoCompactAt` in `session:usage`, session-statusline.ts).

/** Window the CLI compacts against (`CLAUDE_CODE_AUTO_COMPACT_WINDOW`). */
export const AUTO_COMPACT_WINDOW_TOKENS = 530_000
/** Share of that window at which the CLI compacts (`CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`). */
export const AUTO_COMPACT_PCT = 92

/** CLI env that applies the policy. */
export function autoCompactEnv(): Record<string, string> {
  return {
    CLAUDE_CODE_AUTO_COMPACT_WINDOW: `${AUTO_COMPACT_WINDOW_TOKENS / 1000}k`,
    CLAUDE_AUTOCOMPACT_PCT_OVERRIDE: String(AUTO_COMPACT_PCT),
  }
}

/** Output tokens the CLI keeps free below the window before applying the percentage. */
export const CLI_OUTPUT_RESERVE_TOKENS = 20_000
/** Minimum distance the CLI keeps between the threshold and the reserved window. */
export const CLI_COMPACT_BUFFER_TOKENS = 13_000

/**
 * Token count at which the CLI auto-compacts, mirroring Claude Code 2.1.322:
 * the override window is clamped to the model's real window, up to 20k output
 * tokens are reserved, and the threshold is min(pct% of the remainder,
 * remainder - 13k). `modelWindow` is the status line's `context_window_size`;
 * unknown means the model window is not smaller than the override.
 */
export function autoCompactThreshold(modelWindow: number | null | undefined): number {
  const model =
    typeof modelWindow === 'number' && Number.isSafeInteger(modelWindow) && modelWindow > 0 ? modelWindow : Infinity
  const remainder = Math.min(model, AUTO_COMPACT_WINDOW_TOKENS) - CLI_OUTPUT_RESERVE_TOKENS
  return Math.max(0, Math.min(Math.floor((remainder * AUTO_COMPACT_PCT) / 100), remainder - CLI_COMPACT_BUFFER_TOKENS))
}
