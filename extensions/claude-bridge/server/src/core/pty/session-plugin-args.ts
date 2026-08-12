// ============================================================================
// PTY Session — materialized plugin roots and Claude CLI launch arguments
// ============================================================================

import fs from "fs";
import path from "path";
import { buildClaudeArgs } from "./session-env";
import type { SessionConfig } from "./types";

/** Absolute proxy-plugin roots to pass as repeated `--plugin-dir` arguments. */
export function getSessionPluginDirs(settingsDir: string): string[] {
  const root = path.join(settingsDir, ".bridge-plugins");
  if (!fs.existsSync(root)) return [];
  try {
    return fs
      .readdirSync(root, { withFileTypes: true })
      .filter(
        (entry) =>
          entry.isDirectory() &&
          fs.existsSync(
            path.join(root, entry.name, ".claude-plugin", "plugin.json"),
          ),
      )
      .map((entry) => path.join(root, entry.name))
      .sort();
  } catch {
    return [];
  }
}

/** Where the CLI writes its own debug log when the knob below is on. Sits in
 * the session directory so it is thrown away with the session and never grows
 * unbounded across restarts. */
export function cliDebugLogPath(settingsDir: string): string {
  return path.join(settingsDir, "cli-debug.log");
}

/** Opt-in CLI debug log, off unless BRIDGE_CLI_DEBUG_LOG is set.
 *
 * The CLI keeps its own diagnostics for things the bridge cannot see, and the
 * team mailbox is one of them: a teammate's message to the lead can be dropped
 * with a warn-level `[InboxPoller] Dropping unrouted protocol frame from …`,
 * or held in memory while the lead is busy and flushed only at its next idle.
 * Neither line reaches the PTY without `--debug-file`, which is why "the
 * teammates answered but the lead says nobody did" left no trace to read.
 *
 * Off by default: the log is chatty and holds prompt text. */
function cliDebugArgs(settingsDir: string): string[] {
  if (!process.env.BRIDGE_CLI_DEBUG_LOG) return [];
  return ["--debug-file", cliDebugLogPath(settingsDir)];
}

/** Keep plugin discovery wired to the actual spawn path, not only to the
 * lower-level argument builder used by unit tests. */
export function buildSessionClaudeArgs(
  config: SessionConfig,
  settingsDir: string,
): string[] {
  return [
    ...buildClaudeArgs(config, getSessionPluginDirs(settingsDir)),
    ...cliDebugArgs(settingsDir),
  ];
}
